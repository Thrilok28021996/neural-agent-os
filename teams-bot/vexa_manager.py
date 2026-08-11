"""
Neural Agent OS — Vexa Meeting Bot Integration

Vexa (https://vexa.ai, Apache 2.0) handles Google Meet, Microsoft Teams,
and Zoom meeting bots. This module manages the Vexa deployment lifecycle
and data flow between Vexa and Neural Agent OS.

Architecture:
  Vexa Docker container (self-hosted) ←→ Neural Agent OS local API
  Vexa joins meetings → captures audio → transcribes → sends to Neural
"""

import asyncio
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import aiohttp

# ── Configuration ──────────────────────────────────────────────────────────

VEXA_IMAGE = os.environ.get("NEURAL_VEXA_IMAGE", "ghcr.io/vexa-ai/vexa:latest")
VEXA_PORT = int(os.environ.get("NEURAL_VEXA_PORT", "8445"))
VEXA_API_URL = f"http://127.0.0.1:{VEXA_PORT}"
NEURAL_API_URL = os.environ.get("NEURAL_API_URL", "http://127.0.0.1:8787")
VEXA_DATA_DIR = os.environ.get("NEURAL_VEXA_DATA_DIR", "/var/lib/neural/vexa")

# Provider-specific credentials (from OS keychain or env)
GOOGLE_CLIENT_ID = os.environ.get("NEURAL_GOOGLE_CLIENT_ID", "")
GOOGLE_CLIENT_SECRET = os.environ.get("NEURAL_GOOGLE_CLIENT_SECRET", "")
MICROSOFT_CLIENT_ID = os.environ.get("NEURAL_MICROSOFT_CLIENT_ID", "")
MICROSOFT_CLIENT_SECRET = os.environ.get("NEURAL_MICROSOFT_CLIENT_SECRET", "")


class VexaManager:
    """Manages Vexa Docker deployment and data synchronization."""

    def __init__(self):
        self.container_name = "neural-vexa-bot"
        self.running = False
        self.session: Optional[aiohttp.ClientSession] = None

    async def ensure_running(self) -> bool:
        """Start Vexa container if not running."""
        if await self._is_healthy():
            self.running = True
            return True

        print("[Vexa] Starting container...", file=sys.stderr)
        env = {
            "GOOGLE_CLIENT_ID": GOOGLE_CLIENT_ID,
            "GOOGLE_CLIENT_SECRET": GOOGLE_CLIENT_SECRET,
            "MICROSOFT_CLIENT_ID": MICROSOFT_CLIENT_ID,
            "MICROSOFT_CLIENT_SECRET": MICROSOFT_CLIENT_SECRET,
            "DATABASE_URL": f"postgresql://neural:neural@localhost:5432/vexa",
            "REDIS_URL": "redis://localhost:6379",
            "VEXA_API_PORT": str(VEXA_PORT),
        }

        try:
            # Pull latest image
            subprocess.run(
                ["docker", "pull", VEXA_IMAGE],
                check=False, capture_output=True, timeout=120,
            )
            # Remove old container
            subprocess.run(
                ["docker", "rm", "-f", self.container_name],
                check=False, capture_output=True,
            )
            # Start new container
            cmd = [
                "docker", "run", "-d",
                "--name", self.container_name,
                "--restart", "unless-stopped",
                "-p", f"{VEXA_PORT}:{VEXA_PORT}",
                "-v", f"{VEXA_DATA_DIR}:/data",
            ]
            for key, val in env.items():
                if val:
                    cmd.extend(["-e", f"{key}={val}"])
            cmd.append(VEXA_IMAGE)

            result = subprocess.run(cmd, check=True, capture_output=True, text=True, timeout=30)
            print(f"[Vexa] Container started: {result.stdout.strip()}", file=sys.stderr)

            # Wait for health
            for _ in range(30):
                if await self._is_healthy():
                    self.running = True
                    return True
                await asyncio.sleep(2)

            print("[Vexa] Container started but health check failed", file=sys.stderr)
            return False
        except Exception as e:
            print(f"[Vexa] Failed to start: {e}", file=sys.stderr)
            return False

    async def _is_healthy(self) -> bool:
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(
                    f"{VEXA_API_URL}/health",
                    timeout=aiohttp.ClientTimeout(total=5),
                ) as resp:
                    return resp.status == 200
        except Exception:
            return False

    async def get_session(self) -> aiohttp.ClientSession:
        if self.session is None or self.session.closed:
            self.session = aiohttp.ClientSession()
        return self.session

    async def schedule_bot(
        self,
        meeting_url: str,
        platform: str,  # "google_meet", "microsoft_teams", "zoom"
        meeting_title: str = "Meeting",
        start_time: Optional[str] = None,
        duration_minutes: int = 60,
    ) -> dict:
        """Schedule a bot to join a meeting via Vexa API."""
        if not self.running:
            if not await self.ensure_running():
                return {"error": "Vexa container is not running"}

        session = await self.get_session()
        payload = {
            "meeting_url": meeting_url,
            "platform": platform,
            "title": meeting_title,
            "start_time": start_time or datetime.now(timezone.utc).isoformat(),
            "duration_minutes": duration_minutes,
            "webhook_url": f"{NEURAL_API_URL}/vexa/webhook",
        }

        try:
            async with session.post(
                f"{VEXA_API_URL}/api/bots",
                json=payload,
                timeout=aiohttp.ClientTimeout(total=15),
            ) as resp:
                if resp.status in (200, 201):
                    data = await resp.json()
                    # Notify Neural about the scheduled bot
                    await self._notify_neural("bot_scheduled", {
                        "bot_id": data.get("id"),
                        "meeting_url": meeting_url,
                        "platform": platform,
                        "title": meeting_title,
                    })
                    return data
                else:
                    text = await resp.text()
                    return {"error": f"Vexa API error: {resp.status} - {text[:200]}"}
        except Exception as e:
            return {"error": str(e)}

    async def get_bot_status(self, bot_id: str) -> dict:
        """Get status of a scheduled/running bot."""
        session = await self.get_session()
        try:
            async with session.get(
                f"{VEXA_API_URL}/api/bots/{bot_id}",
                timeout=aiohttp.ClientTimeout(total=10),
            ) as resp:
                return await resp.json() if resp.status == 200 else {"error": f"HTTP {resp.status}"}
        except Exception as e:
            return {"error": str(e)}

    async def get_transcript(self, bot_id: str) -> dict:
        """Get transcript from a completed bot session."""
        session = await self.get_session()
        try:
            async with session.get(
                f"{VEXA_API_URL}/api/bots/{bot_id}/transcript",
                timeout=aiohttp.ClientTimeout(total=10),
            ) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    # Forward transcript to Neural for local storage
                    await self._store_transcript_locally(bot_id, data)
                    return data
                return {"error": f"HTTP {resp.status}"}
        except Exception as e:
            return {"error": str(e)}

    async def _store_transcript_locally(self, bot_id: str, transcript: dict):
        """Send transcript data to Neural Agent OS for local persistence."""
        session = await self.get_session()
        try:
            async with session.post(
                f"{NEURAL_API_URL}/vexa/transcript",
                json={
                    "bot_id": bot_id,
                    "transcript": transcript,
                    "received_at": datetime.now(timezone.utc).isoformat(),
                },
                timeout=aiohttp.ClientTimeout(total=30),
            ) as resp:
                if resp.status != 200:
                    print(f"[Vexa] Failed to store transcript: HTTP {resp.status}", file=sys.stderr)
        except Exception as e:
            print(f"[Vexa] Transcript storage error: {e}", file=sys.stderr)

    async def _notify_neural(self, event: str, data: dict):
        """Send event notification to Neural Agent OS."""
        session = await self.get_session()
        try:
            async with session.post(
                f"{NEURAL_API_URL}/vexa/event",
                json={"event": event, "data": data, "timestamp": datetime.now(timezone.utc).isoformat()},
                timeout=aiohttp.ClientTimeout(total=10),
            ) as resp:
                pass  # Best effort
        except Exception:
            pass

    async def list_bots(self, status: Optional[str] = None) -> list:
        """List all bots."""
        session = await self.get_session()
        params = {}
        if status:
            params["status"] = status
        try:
            async with session.get(
                f"{VEXA_API_URL}/api/bots",
                params=params,
                timeout=aiohttp.ClientTimeout(total=10),
            ) as resp:
                return await resp.json() if resp.status == 200 else []
        except Exception:
            return []

    async def stop(self):
        """Stop Vexa container gracefully."""
        if self.session and not self.session.closed:
            await self.session.close()
        subprocess.run(["docker", "stop", self.container_name], check=False, capture_output=True)
        self.running = False


# ── CLI ────────────────────────────────────────────────────────────────────

async def main():
    manager = VexaManager()
    command = sys.argv[1] if len(sys.argv) > 1 else "start"

    if command == "start":
        ok = await manager.ensure_running()
        print(json.dumps({"status": "running" if ok else "failed"}))
    elif command == "schedule":
        if len(sys.argv) < 4:
            print(json.dumps({"error": "Usage: vexa schedule <platform> <meeting_url> [title]"}))
            return
        platform = sys.argv[2]
        url = sys.argv[3]
        title = sys.argv[4] if len(sys.argv) > 4 else "Meeting"
        result = await manager.schedule_bot(url, platform, title)
        print(json.dumps(result))
    elif command == "status":
        bot_id = sys.argv[2] if len(sys.argv) > 2 else ""
        result = await manager.get_bot_status(bot_id) if bot_id else await manager.list_bots()
        print(json.dumps(result))
    elif command == "transcript":
        bot_id = sys.argv[2] if len(sys.argv) > 2 else ""
        if bot_id:
            result = await manager.get_transcript(bot_id)
            print(json.dumps(result))
    elif command == "stop":
        await manager.stop()
        print(json.dumps({"status": "stopped"}))
    else:
        print(json.dumps({"error": f"Unknown command: {command}"}))


if __name__ == "__main__":
    asyncio.run(main())
