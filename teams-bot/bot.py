"""
Neural Agent OS — Microsoft Teams Meeting Bot

Joins scheduled Teams meetings, captures participant audio,
and transfers encrypted meeting data to the local Neural application.

Deployment: Docker container, runs locally or on a private server.
"""

import asyncio
import json
import os
import signal
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import aiohttp
import aiofiles
from cryptography.fernet import Fernet

# ── Configuration ──────────────────────────────────────────────────────────

DATA_DIR = Path(os.environ.get("NEURAL_TEAMS_DATA_DIR", "/data"))
RECORDINGS_DIR = DATA_DIR / "recordings"
METADATA_DIR = DATA_DIR / "metadata"
ENCRYPTION_KEY = os.environ.get("NEURAL_TEAMS_ENCRYPTION_KEY", Fernet.generate_key().decode())
NEURAL_API_URL = os.environ.get("NEURAL_API_URL", "http://host.docker.internal:8787")
TEAMS_CLIENT_ID = os.environ.get("NEURAL_TEAMS_CLIENT_ID", "")
TEAMS_CLIENT_SECRET = os.environ.get("NEURAL_TEAMS_CLIENT_SECRET", "")
TEAMS_TENANT_ID = os.environ.get("NEURAL_TEAMS_TENANT_ID", "common")

STATUS_INTERVAL = int(os.environ.get("NEURAL_TEAMS_STATUS_INTERVAL", "30"))
POLL_INTERVAL = int(os.environ.get("NEURAL_TEAMS_POLL_INTERVAL", "60"))

# ── State ──────────────────────────────────────────────────────────────────

class TeamsBot:
    def __init__(self):
        self.bot_id = str(uuid.uuid4())
        self.fernet = Fernet(ENCRYPTION_KEY.encode() if isinstance(ENCRYPTION_KEY, str) else ENCRYPTION_KEY)
        self.active_meetings: dict[str, dict] = {}
        self.running = True
        self.access_token: Optional[str] = None
        self.token_expiry: Optional[datetime] = None

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

        # In production: connect to Teams media stream via bot SDK
        # and capture audio frames here.
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
        """Transfer encrypted recording to local Neural Agent OS."""
        info = self.active_meetings.get(meeting_id)
        if not info:
            return False

        recording_path = Path(info["recording_path"])
        if not recording_path.exists():
            await self.report_status("transfer_failed", "Recording file not found", meeting_id)
            return False

        try:
            # Encrypt recording data
            async with aiofiles.open(recording_path, "rb") as f:
                plaintext = await f.read()
            encrypted = self.fernet.encrypt(plaintext)

            # Send to local API
            async with aiohttp.ClientSession() as session:
                form = aiohttp.FormData()
                form.add_field("meeting_id", meeting_id)
                form.add_field("title", info["meeting"].get("title", "Unknown"))
                form.add_field("encrypted", "true")
                form.add_field(
                    "file",
                    encrypted,
                    filename=f"{meeting_id}.enc",
                    content_type="application/octet-stream",
                )

                async with session.post(
                    f"{NEURAL_API_URL}/teams-bot/upload",
                    data=form,
                    timeout=aiohttp.ClientTimeout(total=120),
                ) as resp:
                    if resp.status == 200:
                        await self.report_status("transfer_complete", "Recording transferred", meeting_id)
                        return True
                    else:
                        body = await resp.text()
                        await self.report_status("transfer_failed", f"Upload error: {body[:200]}", meeting_id)
                        return False
        except Exception as e:
            await self.report_status("transfer_failed", str(e), meeting_id)
            return False

    async def run(self):
        """Main bot loop."""
        await self.report_status("starting", "Teams bot initializing")

        RECORDINGS_DIR.mkdir(parents=True, exist_ok=True)
        METADATA_DIR.mkdir(parents=True, exist_ok=True)

        # Initial auth
        if not await self.authenticate():
            print("Authentication failed. Bot will retry in next cycle.", file=sys.stderr)

        await self.report_status("running", "Teams bot is operational")

        while self.running:
            try:
                # Fetch meetings that need bot participation
                meetings = await self.fetch_upcoming_meetings()

                for meeting in meetings:
                    meeting_id = meeting.get("id")
                    if not meeting_id or meeting_id in self.active_meetings:
                        continue

                    # Check if meeting is happening now (±5 minutes)
                    starts_at = meeting.get("starts_at", "")
                    if starts_at:
                        try:
                            start_time = datetime.fromisoformat(starts_at.replace("Z", "+00:00"))
                            now = datetime.now(timezone.utc)
                            diff_seconds = (start_time - now).total_seconds()

                            # Join 2 minutes before start, leave 5 minutes after scheduled end
                            if -120 <= diff_seconds <= 300:
                                if await self.join_meeting(meeting):
                                    await self.capture_audio(meeting_id)
                            elif diff_seconds < -300 and meeting_id in self.active_meetings:
                                # Meeting is past its scheduled time
                                await self.leave_meeting(meeting_id)
                                # Transfer recording asynchronously
                                asyncio.create_task(self.transfer_recording(meeting_id))
                        except (ValueError, TypeError):
                            pass

                await asyncio.sleep(POLL_INTERVAL)
            except Exception as e:
                print(f"Bot loop error: {e}", file=sys.stderr)
                await asyncio.sleep(POLL_INTERVAL)

    def shutdown(self):
        """Graceful shutdown."""
        self.running = False
        for meeting_id in list(self.active_meetings.keys()):
            asyncio.ensure_future(self.leave_meeting(meeting_id))


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
