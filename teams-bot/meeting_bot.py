"""
Neural Agent OS — Browser-Based Meeting Bot

Uses Playwright to automate joining Google Meet, Microsoft Teams (web),
and Zoom (web) meetings. Captures meeting audio and feeds it to the
local Whisper transcription pipeline.

No Azure bot registration required.
No Microsoft Graph Communications SDK required.
No Google Cloud project required (for Meet).

Approach:
1. Open Chromium via Playwright
2. Navigate to meeting URL
3. Handle the platform-specific join flow (dismiss dialogs, enter name, join)
4. Once in the meeting, record system audio via FFmpeg
5. After meeting ends, send audio to Neural's transcription pipeline
"""

import asyncio
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# ── Configuration ──────────────────────────────────────────────────────────

NEURAL_API_URL = os.environ.get("NEURAL_API_URL", "http://127.0.0.1:8787")
RECORDINGS_DIR = Path(os.environ.get("NEURAL_RECORDINGS_DIR", "/tmp/neural-recordings"))
RECORDINGS_DIR.mkdir(parents=True, exist_ok=True)

# ── Platform-specific join handlers ────────────────────────────────────────

PLATFORM_HANDLERS = {
    "google_meet": {
        "join_button": "button:has-text('Join now'), button:has-text('Ask to join'), button[aria-label*='Join']",
        "name_input": "input[aria-label*='name'], input[placeholder*='name'], input[placeholder*='Name']",
        "mute_button": "button[aria-label*='microphone'], button[aria-label*='Microphone'], div[aria-label*='microphone']",
        "camera_button": "button[aria-label*='camera'], button[aria-label*='Camera']",
        "leave_button": "button[aria-label*='leave'], button[aria-label*='Leave']",
        "wait_for_join": 30,
        "selector_timeout": 10000,
    },
    "microsoft_teams": {
        "join_button": "button:has-text('Join now'), button:has-text('Join'), button[aria-label*='Join']",
        "name_input": "input[placeholder*='name'], input[aria-label*='Name'], input#username",
        "mute_button": "button[aria-label*='Mute'], button#mute-button, button[title*='Mute']",
        "camera_button": "button[aria-label*='Camera'], button#video-button",
        "leave_button": "button[aria-label*='Leave'], button#hangup-button, button[title*='Leave']",
        "wait_for_join": 45,
        "selector_timeout": 15000,
    },
    "zoom": {
        "join_button": "button:has-text('Join'), button:has-text('Join Meeting'), a#joinBtn",
        "name_input": "input#inputname, input[placeholder*='name']",
        "mute_button": "button[aria-label*='Mute'], button. mute-button",
        "camera_button": "button[aria-label*='Video']",
        "leave_button": "button:has-text('Leave'), button:has-text('End')",
        "wait_for_join": 30,
        "selector_timeout": 15000,
    },
}

# ── Meeting Bot ────────────────────────────────────────────────────────────

class MeetingBot:
    """Joins web meetings via Playwright and captures audio."""

    def __init__(self, platform: str, meeting_url: str, display_name: str = "Neural Assistant"):
        self.platform = platform
        self.meeting_url = meeting_url
        self.display_name = display_name
        self.handlers = PLATFORM_HANDLERS.get(platform, PLATFORM_HANDLERS["google_meet"])
        self.browser = None
        self.page = None
        self.recording_process: Optional[subprocess.Popen] = None
        self.recording_path: Optional[Path] = None
        self.joined = False
        self.running = True

    async def start(self, headless: bool = False, duration_minutes: int = 60):
        """Launch browser and join the meeting."""
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            print("Install playwright: pip install playwright && playwright install chromium", file=sys.stderr)
            return False

        playwright = await async_playwright().start()
        self.browser = await playwright.chromium.launch(
            headless=headless,
            args=[
                "--use-fake-ui-for-media-stream",      # Auto-grant mic/camera
                "--use-fake-device-for-media-stream",   # Use fake video/audio
                "--disable-blink-features=AutomationControlled",
                "--no-sandbox",
                "--disable-setuid-sandbox",
                f"--window-size=1280,800",
            ],
        )

        context = await self.browser.new_context(
            permissions=["microphone", "camera"],
            geolocation=None,
        )
        self.page = await context.new_page()

        try:
            await self._join_meeting()
        except Exception as e:
            print(f"[Bot] Join failed: {e}", file=sys.stderr)
            await self._cleanup()
            return False

        # Start audio capture via FFmpeg (system audio while browser is in meeting)
        self._start_audio_capture()

        # Stay in meeting for configured duration
        print(f"[Bot] Joined. Recording for {duration_minutes} minutes...", file=sys.stderr)
        await self._wait_in_meeting(duration_minutes)

        # Leave and finalize
        await self._leave_meeting()
        await self._cleanup()

        # Send to Neural transcription
        await self._send_to_neural()
        return True

    async def _join_meeting(self):
        """Handle the platform-specific join flow."""
        print(f"[Bot] Navigating to {self.meeting_url}", file=sys.stderr)
        await self.page.goto(self.meeting_url, wait_until="domcontentloaded", timeout=30000)

        # Wait for page to load
        await asyncio.sleep(3)

        # Enter display name if prompted
        name_input = self.handlers.get("name_input")
        if name_input:
            try:
                input_el = await self.page.wait_for_selector(name_input, timeout=self.handlers["selector_timeout"])
                if input_el:
                    await input_el.click()
                    await input_el.fill("")
                    await input_el.type(self.display_name, delay=50)
                    print(f"[Bot] Entered name: {self.display_name}", file=sys.stderr)
            except Exception:
                pass  # Name input not always present

        # Mute mic and turn off camera (bot doesn't need to send)
        for button_type in ["mute_button", "camera_button"]:
            try:
                selector = self.handlers.get(button_type)
                if selector:
                    btn = await self.page.wait_for_selector(selector, timeout=3000)
                    if btn:
                        await btn.click()
                        await asyncio.sleep(0.5)
            except Exception:
                pass

        # Click join
        join_selector = self.handlers.get("join_button")
        if join_selector:
            try:
                join_btn = await self.page.wait_for_selector(join_selector, timeout=self.handlers["selector_timeout"])
                if join_btn:
                    await join_btn.click()
                    print("[Bot] Clicked join button", file=sys.stderr)
            except Exception as e:
                print(f"[Bot] Could not find join button: {e}", file=sys.stderr)
                # Try pressing Enter as fallback
                await self.page.keyboard.press("Enter")

        # Wait to be admitted
        await asyncio.sleep(self.handlers.get("wait_for_join", 30))

        # Check if we're actually in the meeting
        leave_selector = self.handlers.get("leave_button")
        if leave_selector:
            try:
                await self.page.wait_for_selector(leave_selector, timeout=10000)
                self.joined = True
                print("[Bot] Successfully joined meeting", file=sys.stderr)
            except Exception:
                print("[Bot] May not have joined successfully", file=sys.stderr)

    def _start_audio_capture(self):
        """Start FFmpeg to capture system audio."""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        self.recording_path = RECORDINGS_DIR / f"{self.platform}_{timestamp}.wav"

        ffmpeg_bin = os.environ.get("NEURAL_FFMPEG_BIN", "ffmpeg")

        # Platform-specific audio capture
        if sys.platform == "darwin":
            # macOS: capture system audio via BlackHole or Soundflower
            # Falls back to default microphone if virtual device not set up
            device = os.environ.get("NEURAL_AUDIO_DEVICE", ":0")
            cmd = [ffmpeg_bin, "-y", "-f", "avfoundation", "-i", device,
                   "-ac", "1", "-ar", "16000", str(self.recording_path)]
        elif sys.platform == "win32":
            device = os.environ.get("NEURAL_AUDIO_DEVICE", "audio=virtual-audio-capturer")
            cmd = [ffmpeg_bin, "-y", "-f", "dshow", "-i", device,
                   "-ac", "1", "-ar", "16000", str(self.recording_path)]
        else:  # Linux
            device = os.environ.get("NEURAL_AUDIO_DEVICE", "default")
            cmd = [ffmpeg_bin, "-y", "-f", "pulse", "-i", device,
                   "-ac", "1", "-ar", "16000", str(self.recording_path)]

        try:
            self.recording_process = subprocess.Popen(
                cmd, stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            print(f"[Bot] Recording started: {self.recording_path}", file=sys.stderr)
        except Exception as e:
            print(f"[Bot] FFmpeg recording failed: {e}", file=sys.stderr)

    async def _wait_in_meeting(self, duration_minutes: int):
        """Wait in the meeting, periodically checking if we're still connected."""
        end_time = time.time() + duration_minutes * 60
        while self.running and time.time() < end_time:
            await asyncio.sleep(10)
            # Check if meeting has ended (leave button disappeared)
            try:
                if self.page:
                    leave_btn = await self.page.query_selector(
                        self.handlers.get("leave_button", "")
                    )
                    if not leave_btn and self.joined:
                        print("[Bot] Meeting appears to have ended", file=sys.stderr)
                        break
            except Exception:
                pass

    async def _leave_meeting(self):
        """Click leave/end button to exit the meeting."""
        if not self.page:
            return
        leave_selector = self.handlers.get("leave_button")
        if leave_selector:
            try:
                leave_btn = await self.page.wait_for_selector(leave_selector, timeout=5000)
                if leave_btn:
                    await leave_btn.click()
                    print("[Bot] Left meeting", file=sys.stderr)
                    await asyncio.sleep(2)
            except Exception:
                pass

    async def _cleanup(self):
        """Stop recording and close browser."""
        # Stop FFmpeg
        if self.recording_process:
            self.recording_process.terminate()
            try:
                self.recording_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.recording_process.kill()
            self.recording_process = None
            print("[Bot] Recording stopped", file=sys.stderr)

        # Close browser
        if self.page:
            try:
                await self.page.close()
            except Exception:
                pass
        if self.browser:
            try:
                await self.browser.close()
            except Exception:
                pass

    async def _send_to_neural(self):
        """Send recording to Neural Agent OS for transcription."""
        if not self.recording_path or not self.recording_path.exists():
            print("[Bot] No recording to send", file=sys.stderr)
            return

        try:
            import aiohttp

            meeting_id = f"meeting-{self.platform}-{int(time.time())}"
            title = f"{self.platform.replace('_', ' ').title()} Meeting"

            async with aiohttp.ClientSession() as session:
                # Register meeting
                async with session.post(
                    f"{NEURAL_API_URL}/vexa/transcript",
                    json={
                        "bot_id": meeting_id,
                        "transcript": {
                            "segments": [],  # Will be filled by transcription
                            "recording_path": str(self.recording_path),
                            "platform": self.platform,
                        },
                    },
                    timeout=aiohttp.ClientTimeout(total=10),
                ) as resp:
                    if resp.status == 200:
                        print(f"[Bot] Recording ready for transcription: {self.recording_path}", file=sys.stderr)
                    else:
                        print(f"[Bot] Failed to notify Neural: HTTP {resp.status}", file=sys.stderr)

        except ImportError:
            print("[Bot] aiohttp not available. Install: pip install aiohttp", file=sys.stderr)
        except Exception as e:
            print(f"[Bot] Error sending to Neural: {e}", file=sys.stderr)


# ── CLI ────────────────────────────────────────────────────────────────────

async def main():
    if len(sys.argv) < 3:
        print(json.dumps({
            "usage": "python meeting_bot.py <platform> <meeting_url> [display_name] [duration_minutes]",
            "platforms": ["google_meet", "microsoft_teams", "zoom"],
            "example": "python meeting_bot.py google_meet https://meet.google.com/abc-defg-hij 'Neural Bot' 60",
        }))
        return

    platform = sys.argv[1]
    meeting_url = sys.argv[2]
    display_name = sys.argv[3] if len(sys.argv) > 3 else "Neural Assistant"
    duration = int(sys.argv[4]) if len(sys.argv) > 4 else 60

    bot = MeetingBot(platform, meeting_url, display_name)
    success = await bot.start(duration_minutes=duration)

    result = {
        "platform": platform,
        "joined": bot.joined,
        "recording_path": str(bot.recording_path) if bot.recording_path else None,
        "success": success,
    }
    print(json.dumps(result))


if __name__ == "__main__":
    asyncio.run(main())
