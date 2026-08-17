"""
Neural Agent OS — Microsoft Teams Meeting Bot

Joins scheduled Teams meetings, captures participant audio,
and transfers encrypted meeting data to the local Neural application.

Deployment: Docker container, runs locally or on a private server.
"""

import asyncio
import hashlib
import json
import os
import signal
import struct
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional
from urllib.parse import quote

import aiohttp
import aiofiles
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

# ── Configuration ──────────────────────────────────────────────────────────

DATA_DIR = Path(os.environ.get("NEURAL_TEAMS_DATA_DIR", "/data"))
RECORDINGS_DIR = DATA_DIR / "recordings"
METADATA_DIR = DATA_DIR / "metadata"
# Shared secret between the bot and the local app. One value drives both the
# request auth (`X-Teams-Bot-Token` header) and the AES-256-GCM recording
# encryption (key = SHA-256(secret)). Configure identically on both sides.
SHARED_SECRET = os.environ.get("NAO_TEAMS_BOT_SECRET", "")
NEURAL_API_URL = os.environ.get("NEURAL_API_URL", "http://host.docker.internal:8787")
TEAMS_CLIENT_ID = os.environ.get("NEURAL_TEAMS_CLIENT_ID", "")
TEAMS_CLIENT_SECRET = os.environ.get("NEURAL_TEAMS_CLIENT_SECRET", "")
TEAMS_TENANT_ID = os.environ.get("NEURAL_TEAMS_TENANT_ID", "common")

STATUS_INTERVAL = int(os.environ.get("NEURAL_TEAMS_STATUS_INTERVAL", "30"))
POLL_INTERVAL = int(os.environ.get("NEURAL_TEAMS_POLL_INTERVAL", "60"))

# ── State ──────────────────────────────────────────────────────────────────

MAX_TRANSFER_ATTEMPTS = 5


def derive_aes_key(secret: str) -> bytes:
    """Derive the 32-byte AES-256 key from the shared secret (matches the Rust app)."""
    return hashlib.sha256(secret.encode("utf-8")).digest()


def encrypt_payload(aes_key: bytes, plaintext: bytes) -> bytes:
    """Encrypt a recording for transfer: nonce(12) || AES-256-GCM ciphertext+tag.

    Layout matches the Rust app (`teams_bot::decrypt_recording`).
    """
    nonce = os.urandom(12)
    aesgcm = AESGCM(aes_key)
    return nonce + aesgcm.encrypt(nonce, plaintext, None)


def make_silent_wav(seconds: float = 2.0, sample_rate: int = 16000) -> bytes:
    """Build a minimal valid 16-bit mono PCM WAV of silence.

    Used by the simulated capture so the local pipeline can be exercised
    end-to-end. In production the Graph Communications API bot media SDK
    writes real audio frames instead.
    """
    num_samples = int(seconds * sample_rate)
    data_size = num_samples * 2  # 16-bit mono
    header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF", 36 + data_size, b"WAVE",
        b"fmt ", 16, 1, 1, sample_rate, sample_rate * 2, 2, 16,
        b"data", data_size,
    )
    return header + b"\x00\x00" * num_samples


class TeamsBot:
    def __init__(self, shared_secret: Optional[str] = None):
        self.bot_id = str(uuid.uuid4())
        self.shared_secret = shared_secret if shared_secret is not None else SHARED_SECRET
        self.aes_key = derive_aes_key(self.shared_secret) if self.shared_secret else None
        self.active_meetings: dict[str, dict] = {}
        self.running = True
        self.access_token: Optional[str] = None
        self.token_expiry: Optional[datetime] = None

    def _auth_headers(self) -> dict[str, str]:
        """Headers used for every request to the local app."""
        return {"X-Teams-Bot-Token": self.shared_secret}

    async def report_status(self, status: str, message: str = "", meeting_id: Optional[str] = None):
        """Send status update to the local Neural Agent OS API."""
        payload = {
            "bot_id": self.bot_id,
            "status": status,
            "message": message,
            "meeting_id": meeting_id,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{NEURAL_API_URL}/teams-bot/status",
                    json=payload,
                    headers=self._auth_headers(),
                    timeout=aiohttp.ClientTimeout(total=10),
                ) as resp:
                    if resp.status != 200:
                        print(f"Status report failed: HTTP {resp.status}", file=sys.stderr)
        except Exception as e:
            print(f"Status report error: {e}", file=sys.stderr)

    async def authenticate(self) -> bool:
        """Acquire Microsoft Graph API token using client credentials."""
        if not TEAMS_CLIENT_ID or not TEAMS_CLIENT_SECRET:
            await self.report_status("auth_failed", "Missing Teams client credentials")
            return False

        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"https://login.microsoftonline.com/{TEAMS_TENANT_ID}/oauth2/v2.0/token",
                    data={
                        "client_id": TEAMS_CLIENT_ID,
                        "client_secret": TEAMS_CLIENT_SECRET,
                        "scope": "https://graph.microsoft.com/.default",
                        "grant_type": "client_credentials",
                    },
                    timeout=aiohttp.ClientTimeout(total=15),
                ) as resp:
                    if resp.status != 200:
                        body = await resp.text()
                        await self.report_status("auth_failed", f"Token request failed: {body}")
                        return False
                    data = await resp.json()
                    self.access_token = data["access_token"]
                    expires_in = data.get("expires_in", 3600)
                    self.token_expiry = datetime.now(timezone.utc).timestamp() + expires_in
                    await self.report_status("authenticated", "Teams Graph API token acquired")
                    return True
        except Exception as e:
            await self.report_status("auth_failed", str(e))
            return False

    async def ensure_authenticated(self):
        """Refresh token if expired."""
        if not self.access_token or (self.token_expiry and datetime.now(timezone.utc).timestamp() > self.token_expiry - 300):
            await self.authenticate()

    async def fetch_upcoming_meetings(self):
        """Fetch upcoming Teams meetings from the local Neural calendar."""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(
                    f"{NEURAL_API_URL}/teams-bot/meetings",
                    headers=self._auth_headers(),
                    timeout=aiohttp.ClientTimeout(total=10),
                ) as resp:
                    if resp.status == 200:
                        return await resp.json()
        except Exception as e:
            print(f"Failed to fetch meetings: {e}", file=sys.stderr)
        return []

    async def join_meeting(self, meeting: dict) -> bool:
        """Attempt to join a Teams meeting via Graph API."""
        await self.ensure_authenticated()
        if not self.access_token:
            return False

        meeting_id = meeting.get("id", str(uuid.uuid4()))
        online_meeting_id = meeting.get("online_meeting_id")

        if not online_meeting_id:
            await self.report_status(
                "join_failed",
                f"No online meeting ID for {meeting.get('title', 'Unknown')}",
                meeting_id,
            )
            return False

        try:
            async with aiohttp.ClientSession() as session:
                headers = {
                    "Authorization": f"Bearer {self.access_token}",
                    "Content-Type": "application/json",
                }
                # Register bot as a participant in the online meeting
                body = {
                    "participants": {
                        "attendees": [
                            {
                                "identity": {
                                    "application": {
                                        "id": TEAMS_CLIENT_ID,
                                        "displayName": "Neural Assistant (Recording Bot)",
                                    }
                                }
                            }
                        ]
                    }
                }
                async with session.post(
                    f"https://graph.microsoft.com/v1.0/communications/onlineMeetings/{online_meeting_id}/participants",
                    headers=headers,
                    json=body,
                    timeout=aiohttp.ClientTimeout(total=15),
                ) as resp:
                    if resp.status in (200, 201, 202):
                        self.active_meetings[meeting_id] = {
                            "meeting": meeting,
                            "joined_at": datetime.now(timezone.utc).isoformat(),
                            "status": "joined",
                            "recording_path": str(
                                RECORDINGS_DIR / f"{meeting_id}_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}.wav"
                            ),
                        }
                        await self.report_status("joined", f"Joined meeting: {meeting.get('title')}", meeting_id)
                        return True
                    else:
                        body_text = await resp.text()
                        await self.report_status(
                            "join_failed",
                            f"Graph API error: HTTP {resp.status} - {body_text[:200]}",
                            meeting_id,
                        )
                        return False
        except Exception as e:
            await self.report_status("join_failed", str(e), meeting_id)
            return False

    async def capture_audio(self, meeting_id: str):
        """Simulate audio capture from a joined Teams meeting.

        In production, this would use the Microsoft Graph Communications API
        bot media SDK to capture real-time audio streams. This stub
        demonstrates the data flow and can be replaced with the official
        Microsoft.Graph.Communications.Calls.Media library.
        """
        info = self.active_meetings.get(meeting_id)
        if not info:
            return

        recording_path = Path(info["recording_path"])
        recording_path.parent.mkdir(parents=True, exist_ok=True)

        # In production: connect to the Teams media stream via the Graph
        # Communications API / bot media SDK and write real audio frames here.
        # The stub below writes a short silent WAV so the join -> capture ->
        # encrypt -> upload -> decrypt pipeline can run end-to-end locally.
        simulated = make_silent_wav(seconds=2.0, sample_rate=16000)
        async with aiofiles.open(recording_path, "wb") as f:
            await f.write(simulated)

        await self.report_status(
            "recording",
            f"Audio capture active for {info['meeting'].get('title')}",
            meeting_id,
        )

        # Write metadata
        metadata = {
            "meeting_id": meeting_id,
            "title": info["meeting"].get("title"),
            "joined_at": info["joined_at"],
            "bot_id": self.bot_id,
            "encrypted": True,
        }
        meta_path = METADATA_DIR / f"{meeting_id}.json"
        meta_path.parent.mkdir(parents=True, exist_ok=True)
        async with aiofiles.open(meta_path, "w") as f:
            await f.write(json.dumps(metadata, indent=2))

        info["status"] = "recording"
        self.active_meetings[meeting_id] = info

    async def leave_meeting(self, meeting_id: str):
        """Leave a meeting and finalize recording."""
        info = self.active_meetings.pop(meeting_id, None)
        if not info:
            return

        await self.report_status("left", f"Left meeting: {info['meeting'].get('title')}", meeting_id)

    async def transfer_recording(self, meeting_id: str) -> bool:
        """Transfer the encrypted recording to the local Neural Agent OS app.

        The meeting stays in `active_meetings` until the transfer succeeds, so
        a failed upload can be retried on the next poll. After
        MAX_TRANSFER_ATTEMPTS the state is dropped with a final failure report.
        """
        info = self.active_meetings.get(meeting_id)
        if not info:
            return False
        if info.get("transfer_complete"):
            return True
        if info.get("transfer_attempts", 0) >= MAX_TRANSFER_ATTEMPTS:
            self.active_meetings.pop(meeting_id, None)
            await self.report_status("transfer_failed", "Max transfer attempts reached", meeting_id)
            return False
        if not self.aes_key:
            self.active_meetings.pop(meeting_id, None)
            await self.report_status("transfer_failed", "NAO_TEAMS_BOT_SECRET not configured", meeting_id)
            return False

        recording_path = Path(info["recording_path"])
        if not recording_path.exists():
            self.active_meetings.pop(meeting_id, None)
            await self.report_status("transfer_failed", "Recording file not found", meeting_id)
            return False

        try:
            # Encrypt recording data (nonce || AES-256-GCM ciphertext+tag)
            async with aiofiles.open(recording_path, "rb") as f:
                plaintext = await f.read()
            encrypted = encrypt_payload(self.aes_key, plaintext)

            title = info["meeting"].get("title", "Unknown")
            url = (
                f"{NEURAL_API_URL}/teams-bot/upload"
                f"?meeting_id={quote(meeting_id)}&title={quote(title)}"
            )
            # Send to local API as a raw binary body (matches the Rust handler)
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    url,
                    data=encrypted,
                    headers={
                        **self._auth_headers(),
                        "Content-Type": "application/octet-stream",
                    },
                    timeout=aiohttp.ClientTimeout(total=120),
                ) as resp:
                    if resp.status == 200:
                        info["transfer_complete"] = True
                        info["status"] = "transferred"
                        self.active_meetings[meeting_id] = info
                        await self.report_status("transfer_complete", "Recording transferred", meeting_id)
                        self.active_meetings.pop(meeting_id, None)
                        return True
                    else:
                        body = await resp.text()
                        info["transfer_attempts"] = info.get("transfer_attempts", 0) + 1
                        self.active_meetings[meeting_id] = info
                        await self.report_status("transfer_failed", f"Upload error: {body[:200]}", meeting_id)
                        return False
        except Exception as e:
            info["transfer_attempts"] = info.get("transfer_attempts", 0) + 1
            self.active_meetings[meeting_id] = info
            await self.report_status("transfer_failed", str(e), meeting_id)
            return False

    async def _poll_once(self, meetings: list):
        """Process one poll of scheduled meetings (testable without the loop).

        - Meetings already joined: once the window has passed (>5 min after
          start), transfer the recording FIRST while the state still exists,
          then leave. Failed transfers keep the state and retry on the next
          poll (up to MAX_TRANSFER_ATTEMPTS).
        - New meetings: join from 2 minutes before start until 5 minutes after.
        """
        for meeting in meetings:
            meeting_id = meeting.get("id")
            if not meeting_id:
                continue
            starts_at = meeting.get("starts_at", "")
            if not starts_at:
                continue
            try:
                start_time = datetime.fromisoformat(starts_at.replace("Z", "+00:00"))
                now = datetime.now(timezone.utc)
                diff_seconds = (start_time - now).total_seconds()
            except (ValueError, TypeError):
                continue

            if meeting_id in self.active_meetings:
                if diff_seconds < -300:
                    if await self.transfer_recording(meeting_id):
                        await self.leave_meeting(meeting_id)
                    else:
                        await self.report_status(
                            "transfer_pending",
                            "Transfer failed; will retry on next poll",
                            meeting_id,
                        )
                continue

            if -120 <= diff_seconds <= 300:
                if await self.join_meeting(meeting):
                    await self.capture_audio(meeting_id)

    async def run(self):
        """Main bot loop."""
        await self.report_status("starting", "Teams bot initializing")

        for data_dir in (RECORDINGS_DIR, METADATA_DIR):
            try:
                data_dir.mkdir(parents=True, exist_ok=True)
            except OSError as e:
                print(f"Warning: cannot create {data_dir}: {e}", file=sys.stderr)

        # Initial auth
        if not await self.authenticate():
            print("Authentication failed. Bot will retry in next cycle.", file=sys.stderr)

        await self.report_status("running", "Teams bot is operational")

        while self.running:
            try:
                # Fetch meetings that need bot participation
                meetings = await self.fetch_upcoming_meetings()
                await self._poll_once(meetings)
                await asyncio.sleep(POLL_INTERVAL)
            except Exception as e:
                print(f"Bot loop error: {e}", file=sys.stderr)
                await asyncio.sleep(POLL_INTERVAL)

    def shutdown(self):
        """Graceful shutdown: hand off any active recordings, then leave."""
        self.running = False
        for meeting_id in list(self.active_meetings.keys()):
            asyncio.ensure_future(self._shutdown_meeting(meeting_id))

    async def _shutdown_meeting(self, meeting_id: str):
        """Best-effort final transfer before leaving; never blocks shutdown forever."""
        try:
            await asyncio.wait_for(self.transfer_recording(meeting_id), timeout=30)
        except Exception:
            pass
        await self.leave_meeting(meeting_id)


# ── Entrypoint ─────────────────────────────────────────────────────────────

def handle_signal(bot: TeamsBot):
    """Handle OS signals for graceful shutdown."""
    def _handler(signum, frame):
        print(f"\nReceived signal {signum}, shutting down...")
        bot.shutdown()
    return _handler


async def main():
    bot = TeamsBot()

    loop = asyncio.get_event_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        try:
            loop.add_signal_handler(sig, handle_signal(bot))
        except NotImplementedError:
            # Windows doesn't support add_signal_handler
            pass

    try:
        await bot.run()
    except asyncio.CancelledError:
        bot.shutdown()
    finally:
        print("Teams bot stopped.")


if __name__ == "__main__":
    asyncio.run(main())
