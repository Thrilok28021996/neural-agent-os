"""Tests for the Teams meeting bot (teams-bot/bot.py).

Covers the locally-hardenable parts of release blocker #4:
- WAV simulation helper
- AES-256-GCM payload format shared with the Rust app
- authenticated status/upload flow (X-Teams-Bot-Token)
- transfer-before-leave ordering and retry/failure state machine

Graph/tenant interactions are mocked; real-tenant smoke tests remain external.
"""

import asyncio
import struct
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

import bot as bot_module
from bot import (
    MAX_TRANSFER_ATTEMPTS,
    TeamsBot,
    derive_aes_key,
    encrypt_payload,
    make_silent_wav,
)


# ── Pure helpers ───────────────────────────────────────────────────────────

def parse_wav(data: bytes):
    """Return (sample_rate, channels, bits, data_bytes) from a WAV blob."""
    assert data[:4] == b"RIFF", "missing RIFF"
    assert data[8:12] == b"WAVE", "missing WAVE"
    fmt, size, audio_format, channels, rate, byte_rate, align, bits = struct.unpack_from("<4sIHHIIHH", data, 12)
    assert fmt == b"fmt ", "missing fmt chunk"
    assert audio_format == 1, "expected PCM"
    data_size = struct.unpack_from("<I", data, 40)[0]
    payload = data[44:44 + data_size]
    return rate, channels, bits, payload


def test_make_silent_wav_is_valid_pcm():
    wav = make_silent_wav(seconds=1.0, sample_rate=16000)
    rate, channels, bits, payload = parse_wav(wav)
    assert rate == 16000
    assert channels == 1
    assert bits == 16
    assert len(payload) == 16000 * 2  # 1 second of 16-bit mono
    assert all(b == 0 for b in payload)  # silence


def test_derive_aes_key_is_deterministic_32_bytes():
    a = derive_aes_key("s3cret")
    assert len(a) == 32
    assert a == derive_aes_key("s3cret")
    assert a != derive_aes_key("other")


def test_encrypt_payload_layout_matches_rust_contract():
    key = derive_aes_key("s3cret")
    plaintext = b"RIFF....wav bytes"
    payload = encrypt_payload(key, plaintext)
    # nonce(12) || ciphertext+tag
    assert len(payload) == 12 + len(plaintext) + 16
    nonce, ciphertext = payload[:12], payload[12:]
    decrypted = AESGCM(key).decrypt(nonce, ciphertext, None)
    assert decrypted == plaintext


# ── Fake HTTP transport ────────────────────────────────────────────────────

class FakeResponse:
    def __init__(self, status=200, text=""):
        self.status = status
        self._text = text

    async def text(self):
        return self._text

    async def json(self):
        return {}


class FakeRequestContext:
    """Mirrors aiohttp's `_RequestContextManager`: returned by session.post/get."""
    def __init__(self, response: FakeResponse):
        self._response = response

    async def __aenter__(self):
        return self._response

    async def __aexit__(self, *args):
        return False


class FakeSession:
    def __init__(self, response: FakeResponse):
        self._response = response
        self.requests = []

    async def __aenter__(self):
        return self

    async def __aexit__(self, *args):
        return False

    def post(self, *args, **kwargs):
        self.requests.append(("post", args, kwargs))
        return FakeRequestContext(self._response)

    def get(self, *args, **kwargs):
        self.requests.append(("get", args, kwargs))
        return FakeRequestContext(self._response)


@pytest.fixture
def fake_aiohttp(monkeypatch):
    """Replace aiohttp.ClientSession with an in-memory fake."""
    state = {"session": None}

    def make_session(**kwargs):
        state["session"] = FakeSession(state["response"])
        return state["session"]

    monkeypatch.setattr(bot_module.aiohttp, "ClientSession", make_session)
    return state


# ── Auth headers / status ──────────────────────────────────────────────────

def test_auth_headers_carry_shared_secret():
    bot = TeamsBot(shared_secret="top-secret")
    assert bot._auth_headers() == {"X-Teams-Bot-Token": "top-secret"}
    assert bot.aes_key == derive_aes_key("top-secret")


def test_report_status_sends_token_and_payload(fake_aiohttp):
    fake_aiohttp["response"] = FakeResponse(200)
    bot = TeamsBot(shared_secret="s")
    calls = []

    async def run():
        await bot.report_status("running", "hello", "m1")
        session = fake_aiohttp["session"]
        method, args, kwargs = session.requests[0]
        assert method == "post"
        assert args[0].endswith("/teams-bot/status")
        assert kwargs["headers"]["X-Teams-Bot-Token"] == "s"
        assert kwargs["json"]["status"] == "running"
        assert kwargs["json"]["meeting_id"] == "m1"
        calls.append(1)

    asyncio.run(run())
    assert calls


# ── Transfer state machine ─────────────────────────────────────────────────

def make_active_bot(tmp_path, monkeypatch, fake_aiohttp, statuses):
    """Bot with one active meeting whose recording exists on disk."""
    bot = TeamsBot(shared_secret="s")
    recording = Path(tmp_path) / "rec.wav"
    recording.write_bytes(make_silent_wav(seconds=0.5))
    meeting_id = "meet-1"
    bot.active_meetings[meeting_id] = {
        "meeting": {"id": meeting_id, "title": "Sync"},
        "joined_at": "now",
        "status": "recording",
        "recording_path": str(recording),
    }
    reported = []
    async def report(status, message="", meeting_id=None):
        reported.append((status, message))
    monkeypatch.setattr(bot, "report_status", report)
    fake_aiohttp["response"] = FakeResponse(200)
    return bot, meeting_id, reported


def test_transfer_success_pops_state(fake_aiohttp, tmp_path, monkeypatch):
    bot, meeting_id, reported = make_active_bot(tmp_path, monkeypatch, fake_aiohttp, [200])
    fake_aiohttp["response"] = FakeResponse(200)

    async def run():
        ok = await bot.transfer_recording(meeting_id)
        assert ok is True
        assert meeting_id not in bot.active_meetings
        assert ("transfer_complete", "Recording transferred") in reported
        # payload is the encrypted binary body, not multipart
        session = fake_aiohttp["session"]
        method, args, kwargs = session.requests[0]
        assert method == "post"
        assert "meeting_id=meet-1" in args[0]
        assert kwargs["headers"]["Content-Type"] == "application/octet-stream"
        body = kwargs["data"]
        nonce, ciphertext = body[:12], body[12:]
        plaintext = AESGCM(bot.aes_key).decrypt(nonce, ciphertext, None)
        assert plaintext[:4] == b"RIFF"

    asyncio.run(run())


def test_transfer_failure_keeps_state_and_retries(fake_aiohttp, tmp_path, monkeypatch):
    bot, meeting_id, reported = make_active_bot(tmp_path, monkeypatch, fake_aiohttp, [500])
    fake_aiohttp["response"] = FakeResponse(500, text="boom")

    async def run():
        ok = await bot.transfer_recording(meeting_id)
        assert ok is False
        assert meeting_id in bot.active_meetings  # state preserved for retry
        assert bot.active_meetings[meeting_id]["transfer_attempts"] == 1
        assert ("transfer_failed", "Upload error: boom") in reported
        # second attempt increments
        ok2 = await bot.transfer_recording(meeting_id)
        assert ok2 is False
        assert bot.active_meetings[meeting_id]["transfer_attempts"] == 2

    asyncio.run(run())


def test_transfer_gives_up_after_max_attempts(fake_aiohttp, tmp_path, monkeypatch):
    bot, meeting_id, reported = make_active_bot(tmp_path, monkeypatch, fake_aiohttp, [500])
    fake_aiohttp["response"] = FakeResponse(500, text="boom")

    async def run():
        bot.active_meetings[meeting_id]["transfer_attempts"] = MAX_TRANSFER_ATTEMPTS
        ok = await bot.transfer_recording(meeting_id)
        assert ok is False
        assert meeting_id not in bot.active_meetings
        assert ("transfer_failed", "Max transfer attempts reached") in reported

    asyncio.run(run())


def test_transfer_without_secret_fails_closed(fake_aiohttp, tmp_path, monkeypatch):
    bot = TeamsBot(shared_secret="")
    recording = Path(tmp_path) / "rec.wav"
    recording.write_bytes(b"x")
    bot.active_meetings["m"] = {"meeting": {"title": "t"}, "recording_path": str(recording), "status": "recording"}
    reported = []
    async def report(status, message="", meeting_id=None):
        reported.append((status, message))
    monkeypatch.setattr(bot, "report_status", report)

    async def run():
        assert await bot.transfer_recording("m") is False
        assert "m" not in bot.active_meetings
        assert ("transfer_failed", "NAO_TEAMS_BOT_SECRET not configured") in reported

    asyncio.run(run())


# ── Run-loop scheduling (transfer before leave) ────────────────────────────

def past_meeting(meeting_id, minutes_ago=10):
    start = datetime.now(timezone.utc) - timedelta(minutes=minutes_ago)
    return {"id": meeting_id, "title": "Past", "starts_at": start.isoformat()}


def upcoming_meeting(meeting_id, delta_minutes=0):
    start = datetime.now(timezone.utc) + timedelta(minutes=delta_minutes)
    return {"id": meeting_id, "title": "Now", "starts_at": start.isoformat()}


def run_one_poll(bot, meetings):
    """Run a single poll iteration (startup noise excluded)."""
    asyncio.run(bot._poll_once(meetings))


def test_loop_transfers_before_leaving(fake_aiohttp, tmp_path, monkeypatch):
    """An active meeting past its window is transferred FIRST, then left."""
    bot = TeamsBot(shared_secret="s")
    meeting_id = "meet-past"
    recording = Path(tmp_path) / "rec.wav"
    recording.write_bytes(make_silent_wav(seconds=0.5))
    bot.active_meetings[meeting_id] = {
        "meeting": {"id": meeting_id, "title": "Past"},
        "joined_at": "now",
        "status": "recording",
        "recording_path": str(recording),
    }
    order = []
    async def report(status, message="", meeting_id=None):
        order.append(("report", status))
    async def transfer(mid):
        order.append(("transfer", mid))
        return True
    async def leave(mid):
        order.append(("leave", mid))
    async def join(m):
        order.append(("join", m["id"]))
        return True
    async def capture(mid):
        order.append(("capture", mid))
    monkeypatch.setattr(bot, "report_status", report)
    monkeypatch.setattr(bot, "transfer_recording", transfer)
    monkeypatch.setattr(bot, "leave_meeting", leave)
    monkeypatch.setattr(bot, "join_meeting", join)
    monkeypatch.setattr(bot, "capture_audio", capture)
    fake_aiohttp["response"] = FakeResponse(200)

    run_one_poll(bot, [past_meeting(meeting_id)])

    assert order[0] == ("transfer", meeting_id)
    assert order[1] == ("leave", meeting_id)
    assert ("report", "transfer_pending") not in order


def test_loop_does_not_rejoin_active_meeting(fake_aiohttp, tmp_path, monkeypatch):
    """An active meeting still inside its window is left alone."""
    bot = TeamsBot(shared_secret="s")
    meeting_id = "meet-active"
    recording = Path(tmp_path) / "rec.wav"
    recording.write_bytes(b"x")
    bot.active_meetings[meeting_id] = {
        "meeting": {"id": meeting_id, "title": "Active"},
        "joined_at": "now",
        "status": "recording",
        "recording_path": str(recording),
    }
    calls = []
    async def report(status, message="", meeting_id=None):
        calls.append(("report", status))
    async def transfer(mid):
        calls.append(("transfer", mid))
        return True
    async def leave(mid):
        calls.append(("leave", mid))
    async def join(m):
        calls.append(("join", m["id"]))
        return True
    async def capture(mid):
        calls.append(("capture", mid))
    monkeypatch.setattr(bot, "report_status", report)
    monkeypatch.setattr(bot, "transfer_recording", transfer)
    monkeypatch.setattr(bot, "leave_meeting", leave)
    monkeypatch.setattr(bot, "join_meeting", join)
    monkeypatch.setattr(bot, "capture_audio", capture)

    run_one_poll(bot, [upcoming_meeting(meeting_id, delta_minutes=0)])

    assert calls == []  # active + in-window: nothing to do


def test_loop_joins_and_captures_new_meeting(fake_aiohttp, monkeypatch):
    """A new meeting inside the join window is joined and captured."""
    bot = TeamsBot(shared_secret="s")
    meeting_id = "meet-new"
    order = []
    async def report(status, message="", meeting_id=None):
        order.append(("report", status))
    async def join(m):
        order.append(("join", m["id"]))
        # Real join_meeting registers the meeting in active_meetings.
        bot.active_meetings[m["id"]] = {"meeting": m, "status": "joined", "recording_path": "/tmp/x.wav"}
        return True
    async def capture(mid):
        order.append(("capture", mid))
    async def transfer(mid):
        order.append(("transfer", mid))
        return True
    async def leave(mid):
        order.append(("leave", mid))
    monkeypatch.setattr(bot, "report_status", report)
    monkeypatch.setattr(bot, "join_meeting", join)
    monkeypatch.setattr(bot, "capture_audio", capture)
    monkeypatch.setattr(bot, "transfer_recording", transfer)
    monkeypatch.setattr(bot, "leave_meeting", leave)
    fake_aiohttp["response"] = FakeResponse(200)

    run_one_poll(bot, [upcoming_meeting(meeting_id, delta_minutes=0)])

    assert order == [("join", meeting_id), ("capture", meeting_id)]
    assert meeting_id in bot.active_meetings


def test_shutdown_transfers_before_leaving(fake_aiohttp, tmp_path, monkeypatch):
    """Graceful shutdown hands off active recordings before leaving."""
    bot = TeamsBot(shared_secret="s")
    meeting_id = "meet-shutdown"
    recording = Path(tmp_path) / "rec.wav"
    recording.write_bytes(make_silent_wav(seconds=0.5))
    bot.active_meetings[meeting_id] = {
        "meeting": {"id": meeting_id, "title": "Shutdown"},
        "joined_at": "now",
        "status": "recording",
        "recording_path": str(recording),
    }
    order = []
    async def transfer(mid):
        order.append(("transfer", mid))
        return True
    async def leave(mid):
        order.append(("leave", mid))
    monkeypatch.setattr(bot, "transfer_recording", transfer)
    monkeypatch.setattr(bot, "leave_meeting", leave)
    fake_aiohttp["response"] = FakeResponse(200)

    async def run():
        bot.shutdown()
        await asyncio.gather(*asyncio.all_tasks() - {asyncio.current_task()})
    asyncio.run(run())

    assert order == [("transfer", meeting_id), ("leave", meeting_id)]
