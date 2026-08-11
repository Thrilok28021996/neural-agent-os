# Neural Agent OS — Local-Only Setup

Zero external APIs. Everything runs on your machine.

## Prerequisites

```bash
# 1. Ollama (local AI — chat, embeddings, summaries)
brew install ollama        # macOS
# or: curl -fsSL https://ollama.com/install.sh | sh  # Linux
# or: https://ollama.com/download/windows             # Windows

ollama serve &
ollama pull qwen3:14b           # Chat + summarization
ollama pull nomic-embed-text    # Semantic search embeddings

# 2. Whisper (local transcription)
pip install openai-whisper

# 3. FFmpeg (audio recording)
brew install ffmpeg             # macOS
# or: sudo apt install ffmpeg   # Linux
# or: choco install ffmpeg      # Windows

# 4. BlackHole (virtual audio for system capture — macOS)
brew install blackhole-2ch
# Windows: https://vb-audio.com/Cable/
# Linux: pactl load-module module-loopback

# 5. Playwright (meeting bot — joins Teams/Meet/Zoom via browser)
pip install playwright aiohttp
playwright install chromium

# 6. Node.js + Rust
# Node: https://nodejs.org (v20+)
# Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Install & Run

```bash
cd neural-agent-os
npm install
npm run tauri dev
```

## Enable Strict Local Mode

1. Open Neural Agent OS
2. Go to **Settings** → toggle **Strict local-only mode** ON
3. This blocks all cloud provider calls — only Ollama + Whisper + FFmpeg are used

## Email Setup (App Password — No OAuth Needed)

**Gmail:**
1. Enable 2FA: https://myaccount.google.com/security
2. Generate app password: https://myaccount.google.com/apppasswords → "Mail" → "Neural"
3. In Neural → **Mail** → Add account:
   - Provider: Generic IMAP/SMTP
   - Email: you@gmail.com
   - IMAP host: `imap.gmail.com`
   - SMTP host: `smtp.gmail.com`
   - Password: the 16-character app password

**Outlook:**
1. Enable 2FA
2. Generate app password at account.microsoft.com/security
3. IMAP: `outlook.office365.com`, SMTP: `smtp.office365.com`

## Record a Meeting

**System audio (with BlackHole):**
1. Set BlackHole as your system output (Sound settings → Output → BlackHole 2ch)
2. In Neural → **Capture** → Source: System audio → Start recording
3. This captures everything playing through your speakers (including browser meetings)

**Meeting bot (automatic join):**
```bash
python teams-bot/meeting_bot.py google_meet "https://meet.google.com/xxx" "Neural Bot" 60
python teams-bot/meeting_bot.py microsoft_teams "https://teams.microsoft.com/..." "Neural Bot" 45
python teams-bot/meeting_bot.py zoom "https://zoom.us/j/..." "Neural Bot" 30
```

Or from the app: **Calendar** → create event with meeting URL → click 🤖 Join.

## Transcribe

1. **Meetings** → import recording → click **Queue transcription**
2. Whisper runs locally, segments appear with timestamps
3. Label speakers → profiles are created automatically
4. Click **Summarize** to extract action items

## Search

1. **Knowledge** → scan a folder of documents
2. Click **Build searchable index**
3. Click **Create semantic embeddings** (requires Ollama)
4. Search by keyword or semantic meaning

## Verify Everything Works

```bash
# Check Ollama
curl http://localhost:11434/api/tags

# Check Whisper
whisper --help

# Check FFmpeg  
ffmpeg -version

# Check Playwright
python -c "from playwright.sync_api import sync_playwright; print('OK')"

# Run tests
npm test    # 301 tests
```

## Architecture (Local-Only)

```
┌──────────────────────────────────────────────────┐
│  Neural Agent OS (Tauri Desktop App)             │
│                                                  │
│  Frontend: React + TypeScript + Three.js         │
│  Backend:  Rust + SQLite + FTS5                  │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ Ollama   │  │ Whisper  │  │ FFmpeg   │       │
│  │ :11434   │  │ (binary) │  │ (binary) │       │
│  │ chat     │  │ audio→   │  │ mic/sys  │       │
│  │ embed    │  │ text     │  │ →file    │       │
│  └──────────┘  └──────────┘  └──────────┘       │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │Playwright│  │ IMAP/SMTP│  │ Calendar │       │
│  │browser   │  │ app pw   │  │ local    │       │
│  │meetings  │  │ email    │  │ events   │       │
│  └──────────┘  └──────────┘  └──────────┘       │
│                                                  │
│  Zero external APIs. All data stays local.       │
└──────────────────────────────────────────────────┘
```

## What Was Removed (Cloud Stubs)

- OpenAI/Anthropic/Google cloud provider options (local-only mode disables them)
- OAuth Google/Outlook sync (IMAP app passwords instead)
- Cloud transcription (local Whisper only)
- Browser speech APIs disabled in strict local mode (uses local Piper TTS instead)
