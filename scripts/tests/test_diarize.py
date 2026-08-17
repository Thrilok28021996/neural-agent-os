"""Tests for scripts/diarize.py (audio diarization backend wrapper).

The heavy path (pyannote.audio) is not exercised here; we test the stdlib-only
contract pieces: stub windows, duration detection, JSON output shape, and the
clear failure mode when pyannote is not installed.
"""

import json
import struct
import subprocess
import sys
import wave
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import diarize  # noqa: E402


def make_wav(path: Path, seconds: float = 3.0, rate: int = 16000) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(b"\x00\x00" * int(seconds * rate))


def test_stub_windows_cover_the_duration():
    windows = diarize.stub_windows(10.0)
    assert len(windows) == 2
    assert windows[0]["speaker"] == "SPEAKER_00"
    assert windows[1]["speaker"] == "SPEAKER_01"
    assert windows[0]["start"] == 0.0
    assert windows[1]["end"] == 10.0
    # windows are ordered and non-overlapping
    assert windows[0]["end"] <= windows[1]["start"] + 1e-9
    assert windows[0]["start"] < windows[0]["end"]
    assert windows[1]["start"] < windows[1]["end"]


def test_duration_seconds_from_wav(tmp_path):
    path = tmp_path / "a.wav"
    make_wav(path, seconds=2.5)
    assert diarize.duration_seconds(str(path)) == pytest.approx(2.5, abs=0.1)


def test_stub_json_contract_via_cli(tmp_path):
    audio = tmp_path / "in.wav"
    make_wav(audio, seconds=4.0)
    proc = subprocess.run(
        [sys.executable, str(Path(__file__).resolve().parent.parent / "diarize.py"), "--json", "--stub", str(audio)],
        capture_output=True, text=True, timeout=30,
    )
    assert proc.returncode == 0, proc.stderr
    payload = json.loads(proc.stdout)
    assert "segments" in payload
    assert len(payload["segments"]) == 2
    for window in payload["segments"]:
        assert set(window) == {"start", "end", "speaker"}
        assert isinstance(window["start"], (int, float))
        assert isinstance(window["end"], (int, float))
        assert window["speaker"].startswith("SPEAKER_")


def test_real_backend_fails_cleanly_without_deps(tmp_path):
    # System python3 (no sklearn/simple-diarizer in its site-packages): the
    # backend must fail with a clear message, not a traceback.
    audio = tmp_path / "in.wav"
    make_wav(audio)
    proc = subprocess.run(
        [sys.executable, str(Path(__file__).resolve().parent.parent / "diarize.py"), "--json", str(audio)],
        capture_output=True, text=True, timeout=30,
    )
    assert proc.returncode != 0
    assert "diarizer environment" in proc.stderr.lower() or "not installed" in proc.stderr.lower()
