# Neural Agent OS — User Guide

**Version:** 0.1.0  
**Last updated:** August 2026

Neural Agent OS is a local-first desktop personal assistant for macOS, Windows, and Linux. It records and processes meetings, creates multilingual transcripts with speaker diarization, summarizes conversations, manages tasks and reminders, and builds a cited knowledge base — all while keeping your data on your device.

---

## Table of Contents

1. [Installation](#installation)
2. [First Launch](#first-launch)
3. [Workspaces](#workspaces)
4. [Knowledge Base](#knowledge-base)
5. [Meetings](#meetings)
6. [Calendar](#calendar)
7. [Email](#email)
8. [Notes](#notes)
9. [The Assistant](#the-assistant)
10. [Autonomous Agents](#autonomous-agents)
11. [Model Configuration](#model-configuration)
12. [Voice & 3D Interface](#voice--3d-interface)
13. [External Coding Agents](#external-coding-agents)
14. [Microsoft Teams Bot](#microsoft-teams-bot)
15. [Privacy & Data Control](#privacy--data-control)
16. [Import & Export](#import--export)
17. [Troubleshooting](#troubleshooting)

---

## Installation

### macOS
1. Download `Neural Agent OS.dmg` from the releases page
2. Drag the app to your Applications folder
3. On first launch, right-click and select "Open" to bypass Gatekeeper
4. Grant microphone and accessibility permissions when prompted

### Windows
1. Download `Neural Agent OS.msi`
2. Run the installer and follow the prompts
3. Grant microphone permissions when the app starts

### Linux
1. Download the `.deb` or `.AppImage` package
2. Install with `sudo dpkg -i neural-agent-os.deb`
3. Ensure `libwebkit2gtk-4.1-0` and `libgtk-3-0` are installed

### Prerequisites
- **Ollama** (recommended): Install from [ollama.com](https://ollama.com) for local AI
- **FFmpeg**: Required for audio recording. Install via `brew install ffmpeg` (macOS), `choco install ffmpeg` (Windows), or `sudo apt install ffmpeg` (Linux)
- **Whisper** (optional): For local transcription. Install with `pip install openai-whisper`

---

## First Launch

1. Launch Neural Agent OS
2. Choose a data directory where your SQLite database and files will be stored
3. No account creation is required — everything stays local
4. Three default workspaces are pre-created: **Personal**, **Neural Agent OS**, and **Research**

---

## Workspaces

Workspaces isolate knowledge, meetings, emails, and agent contexts. Use them to keep work and personal data separate.

### Creating a Workspace
1. Click the `+ New workspace` button in the sidebar
2. Enter a name
3. The workspace appears immediately in the dropdown

### Workspace Settings
Each workspace can be configured with:
- **Designated folders**: Folders that are scanned for documents
- **Model preferences**: Which AI models handle chat, transcription, etc.
- **Provider fallback**: What to use when a provider is unavailable
- **Cost limits**: Maximum spend per capability
- **Autonomous agents**: Scheduled workflows with permission controls

### Deleting a Workspace
1. Go to the **Data** view
2. Click "Preview deletion" to see all records that would be removed
3. Confirm deletion — this also removes derived data (transcripts, summaries, etc.)
4. Recording files remain at their original paths

---

## Knowledge Base

Neural Agent OS builds a searchable, citable knowledge base from your documents, web pages, emails, meeting transcripts, and notes.

### Adding Sources

**Folder ingestion:**
1. Go to **Knowledge** view
2. Enter an absolute folder path (e.g., `/Users/you/Documents/research`)
3. Click "Scan folder"
4. Supported formats: PDF, Markdown, TXT, DOCX, HTML, JSON, CSV

**Web pages:**
1. Enter a URL and click "Import page"
2. The page is fetched, converted to text, and indexed

**Notes:**
1. Go to **Notes** view
2. Create, edit, and search notes with full-text search

**Emails & Meetings:**
- Automatically indexed when synced or transcribed

### Searching
- **Keyword search**: Uses SQLite FTS5 for instant full-text search
- **Semantic search**: Uses local embeddings via Ollama for meaning-based retrieval
- **Reranked search**: When available, Ollama re-ranks results by relevance to your query

### Citations
When the assistant answers with information from your knowledge base, it cites the source:
```
Sources
- Design brief / mobile assistant (/Users/you/Documents/brief.md)
- Weekly product sync (meeting transcript)
```

### Deletion
- Delete individual sources from the **Sources** view
- Deleting a source removes its searchable chunks and embeddings
- For complete workspace deletion, use the Data view with preview

---

## Meetings

### Recording

**Automatic detection:**
1. Neural Agent OS scans your calendar for meetings with URLs
2. Configure recording behavior in Settings:
   - **Record automatically**: Start recording for detected meetings
   - **Ask before recording**: Show a confirmation dialog
   - **Never record automatically**: Require manual start

**Manual recording:**
1. Go to **Capture** view
2. Select audio source (microphone or system audio)
3. Click "Start recording"
4. A pulsing indicator shows recording is active
5. Click "Stop recording" when done
6. The file is saved and ready for import

**Import existing recordings:**
1. Go to **Meetings** view
2. Enter the recording path and title
3. Supported formats: MP3, WAV, M4A, MP4, MOV, WebM, OGG, FLAC

### Transcription

1. After importing a recording, click "Queue transcription"
2. Transcription uses local Whisper by default
3. For cloud transcription:
   - Set up an API key in Provider settings
   - Select a cloud provider for the transcription capability
4. Speaker labels can be edited inline after transcription

### Voice Profiles
- When you label a speaker in a transcript, a voice profile is created
- The profile sample count grows with each correction
- Profiles help Neural learn who is who across meetings

### Summarization
1. Click "Summarize" on a transcribed meeting
2. The local model extracts:
   - Summary
   - Decisions
   - Action items
3. Action items can be promoted to tasks

---

## Calendar

### Built-in Calendar
Neural Agent OS includes a local calendar as the source of truth.

**Creating events:**
1. Go to **Calendar** view
2. Enter title, start time, and end time
3. Optionally add a meeting URL and recurrence rule
4. Click "Add event"

**Recurring events:**
- Supports standard recurrence (daily, weekly, monthly)

**Meeting detection:**
- Events with meeting URLs are flagged for potential recording

### External Calendar Sync

**Google Calendar:**
1. Click "Connect Google Calendar"
2. Set `NEURAL_GOOGLE_CLIENT_ID` and `NEURAL_GOOGLE_CLIENT_SECRET` environment variables
3. A browser window opens for authorization
4. Click "Sync" to pull events

**Outlook Calendar:**
1. Click "Connect Outlook"
2. Set `NEURAL_MICROSOFT_CLIENT_ID` and `NEURAL_MICROSOFT_CLIENT_SECRET`
3. Authorize in the browser
4. Sync to pull events

### Participants & Conflicts
- Add participants to events from the calendar detail view
- Neural automatically checks for scheduling conflicts
- Conflicts are displayed with overlapping event details

### Meeting Preparation
- Click "Prepare" on any upcoming meeting
- Neural gathers:
  - Recent related emails
  - Open action items
  - Relevant knowledge snippets
  - A suggested agenda

---

## Email

### Connecting Accounts

**Gmail / Outlook:**
1. Go to **Mail** view
2. Select the provider and enter your email address
3. Use OAuth (requires environment credentials) or app-specific passwords

**Generic IMAP/SMTP:**
1. Select "Generic IMAP / SMTP"
2. Enter IMAP and SMTP host addresses
3. Store credentials in the OS keychain

### Synchronization
- Click "Sync" to fetch the 20 most recent emails
- Emails are stored locally and indexed for search
- Thread reconstruction is preserved

### Sending
1. Click "Compose email"
2. Select the sending account
3. Enter recipient, subject, and body
4. Click "Send" — all sending requires explicit user action
5. Emails are sent via SMTP with TLS

### Action Extraction
- Click ⚡ on any email to extract action items
- Extracted tasks appear in the workspace task list

---

## Notes

- Create, edit, and delete rich text notes
- Notes are full-text searchable via FTS5
- Notes are available to the assistant for context-aware responses
- All notes stay within their workspace

---

## The Assistant

### Opening the Assistant
- Click "+ Ask Neural" in the top bar
- Or use the keyboard shortcut `Cmd+K` / `Ctrl+K`

### Capabilities
The assistant can:
- Answer questions from your knowledge base with citations
- Search meeting transcripts by speaker and topic
- Summarize recent activity
- Create tasks and reminders
- Draft email responses
- Plan meeting agendas

### Voice Interaction
- **Voice input**: Click the microphone button and speak
- **Spoken responses**: Click the speaker button to hear the response
- **Language selection**: Switch between English, Hindi, and Telugu

### 3D Assistant
- The 3D assistant character visualizes the assistant state:
  - **Idle**: Gentle rotation
  - **Listening**: Expanding core with pulsing particles
  - **Speaking**: Animated rings and glow effects
  - **Thinking**: Accelerated particle rotation
- The 3D interface is optional — disable it in Settings for accessibility

---

## Autonomous Agents

Agents are scheduled workflows that run at specified times.

### Creating an Agent
1. Go to **Agents** view
2. Enter a name, cron schedule, and prompt
3. Choose a permission mode:
   - **Approval required**: Agent actions need user approval
   - **Read only**: Agent can read but not modify
   - **Full access**: Agent operates without restriction
4. Click "Save paused" to create the agent

### Agent Lifecycle
- **Paused**: Agent is stored but not running
- **Active**: Agent runs on schedule
- **Archived**: Agent is retained but permanently inactive

### Agent Runs
- Each run records status, output, and errors
- Failed runs are retried up to 3 times with 20-second backoff
- Run history is viewable in the agent list

### Built-in Template
The "Daily briefing" template creates an agent that:
1. Lists upcoming meetings for the day
2. Shows open tasks and due dates
3. Highlights recent meeting outcomes
4. Flags unread emails needing attention
5. Suggests daily priorities

---

## Model Configuration

### Capability-Based Routing
Go to **Models** view to configure which model handles each capability:

| Capability | Default | Description |
|---|---|---|
| Chat | Ollama / qwen3:14b | Conversation and reasoning |
| Transcription | Local / whisper-large-v3 | Audio-to-text |
| Embeddings | Ollama / nomic-embed-text | Semantic search vectors |
| Speech | OpenAI / tts-1 | Text-to-speech |
| Summarization | Ollama / qwen3:14b | Meeting and document summaries |

### Provider Fallback
1. In Models, configure fallback chains per capability
2. Set provider priority order
3. If the primary provider fails, the next healthy provider is used
4. Unhealthy providers are automatically marked and can be manually restored

### Cost Limits
1. Set a maximum cost (in cents) per capability
2. When the limit is exceeded, the capability switches to local-only
3. View cost summaries broken down by provider and capability

### Supported Providers

**Local:**
- Ollama
- LM Studio
- llama.cpp (via Ollama-compatible API)
- OpenAI-compatible endpoints

**Cloud:**
- OpenAI (GPT-4o, GPT-4, GPT-3.5, Whisper, TTS)
- Anthropic (Claude 3 Opus, Sonnet, Haiku)
- Google (Gemini 1.5 Pro, Flash)

---

## Voice & 3D Interface

### Voice Input
- Uses the browser's Web Speech API
- Supported in Chrome, Edge, and Safari
- Language selection: English, Hindi, Telugu

### Text-to-Speech
- Uses the browser's Speech Synthesis API
- Language matches the selected assistant language

### 3D Assistant Character
The 3D visualization is built with Three.js and shows:
- **Orbital rings** representing different data contexts
- **Particle field** for knowledge connection density
- **Core sphere** that pulses with assistant activity
- State-aware colors and animation speeds

The 3D character can be:
- Toggled on/off in settings
- Clicked to activate voice input
- Resized to fit your layout

---

## External Coding Agents

Neural Agent OS interoperates with Claude Code, Codex, and OpenCode through:

### MCP Server
- Runs on `http://127.0.0.1:8787/mcp`
- Available tools:
  - `search_workspace` — Search knowledge base
  - `list_tasks` — List workspace tasks
  - `list_meetings` — List recent meetings
  - `list_notes` — List notes
  - `search_transcripts` — Search meeting transcripts
  - `list_agent_runs` — Inspect agent history

### REST API
- `GET /health` — Health check
- `GET /diagnostics` — System diagnostics
- `GET /v1/workspaces` — List workspaces
- `GET /v1/search?workspace_id=&q=` — Search knowledge
- `GET /v1/tasks?workspace_id=` — List tasks
- `GET /v1/meetings?workspace_id=` — List meetings
- `GET /v1/notes?workspace_id=` — List notes
- `GET /v1/transcripts?workspace_id=&q=` — Search transcripts

### CLI
```bash
neural-cli health
neural-cli workspaces
neural-cli search personal "project roadmap"
neural-cli tasks personal
neural-cli meetings personal
neural-cli notes personal
neural-cli transcripts personal "action items"
neural-cli ask personal "What are my open tasks?"
```

### Permissions
External agent access is workspace-scoped. By default, read-only. You can configure:
- Knowledge read access
- Task creation and modification
- Calendar event scheduling
- Email draft and send (requires approval)
- File and command execution

---

## Microsoft Teams Bot

The Teams bot is an optional Docker-deployed service that joins scheduled Teams meetings.

### Deployment
```bash
cd teams-bot
docker build -t neural-teams-bot .
docker run -d \
  --name neural-teams \
  -e NEURAL_TEAMS_CLIENT_ID=your_client_id \
  -e NEURAL_TEAMS_CLIENT_SECRET=your_secret \
  -e NEURAL_TEAMS_TENANT_ID=your_tenant \
  -e NEURAL_API_URL=http://host.docker.internal:8787 \
  -v neural-teams-data:/data \
  neural-teams-bot
```

### Requirements
- Azure AD application with `OnlineMeetings.Read.All` and `Calls.JoinGroupCall.All` permissions
- Tenant admin consent for application permissions
- Bot must be registered in Teams admin center

### Behavior
1. Bot polls the Neural API for upcoming Teams meetings
2. Joins meetings 2 minutes before scheduled start
3. Captures participant audio (requires bot media SDK)
4. Leaves 5 minutes after scheduled end
5. Transfers encrypted recording to the local Neural application

### Status
- The bot reports join, recording, upload, and processing status
- Failures are logged and reported to the Neural API
- Falls back gracefully when it cannot join or authenticate

---

## Privacy & Data Control

### Storage
- All data stored in your selected local directory
- SQLite database with FTS5 indexes
- Large files (recordings, documents) at their original paths
- Embeddings stored locally in SQLite

### Cloud Data Sharing
Control what leaves your device:
- **Cloud audio processing**: ON/OFF toggle
- **Cloud text processing**: ON/OFF toggle
- Per-workspace model selection (keep sensitive workspaces local)
- Explicit warnings when cloud providers are used for sensitive data

### Key Storage
- API keys stored in the operating system keychain
- macOS: Keychain
- Windows: Credential Manager
- Linux: Secret Service / keyring

### Deletion
- Individual source, meeting, note, and email deletion
- Deletion preview shows all derived data
- Workspace deletion removes all related records
- Recording files remain at original paths (manual cleanup)

### Import & Export
- Export any workspace to a portable JSON file
- Import workspace archives without replacing existing data
- Large binary files (recordings) are referenced by path, not embedded

---

## Import & Export

### Exporting
1. Go to **Data** view
2. Enter an output path (e.g., `exports/neural-workspace.json`)
3. Click "Export workspace"
4. The JSON file contains:
   - Workspace metadata
   - Sources and chunks
   - Meetings and transcript segments
   - Tasks and reminders
   - Emails and agents

### Importing
1. Enter the path to a previously exported JSON file
2. Click "Import workspace"
3. Records are inserted without overwriting existing data

---

## Troubleshooting

### Ollama is unavailable
- Ensure Ollama is running: `ollama serve`
- Pull the configured model: `ollama pull qwen3:14b`
- Set `NEURAL_OLLAMA_URL` if Ollama runs on a different host

### Whisper not found
- Install Whisper: `pip install openai-whisper`
- Or set `NEURAL_WHISPER_BIN` to a custom path
- For cloud transcription, configure an API key

### FFmpeg not found
- Install FFmpeg for your platform
- Or set `NEURAL_FFMPEG_BIN` to a custom path

### Microphone not working
- Check OS permissions for the app
- On macOS: System Preferences → Privacy → Microphone
- On Windows: Settings → Privacy → Microphone
- On Linux: Ensure PulseAudio is running

### Calendar sync fails
- Verify OAuth credentials are set as environment variables
- Check that the redirect URI `http://127.0.0.1:8787/oauth/callback` is registered
- Ensure `access_type=offline` and `prompt=consent` are in the auth URL

### Email sync fails
- For Gmail: Use an app-specific password or OAuth
- For Outlook: Ensure IMAP is enabled in account settings
- For IMAP: Verify host, port, and TLS settings

### Database issues
- The database is at `[data-directory]/neural-agent-os.sqlite`
- Backup this file before major operations
- Restart the app after changing the data directory

---

## Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `NEURAL_OLLAMA_URL` | Ollama API endpoint | `http://127.0.0.1:11434/api/generate` |
| `NEURAL_OLLAMA_EMBEDDINGS_URL` | Ollama embeddings endpoint | `http://127.0.0.1:11434/api/embeddings` |
| `NEURAL_WHISPER_BIN` | Whisper executable path | `whisper` |
| `NEURAL_FFMPEG_BIN` | FFmpeg executable path | `ffmpeg` |
| `NEURAL_GOOGLE_CLIENT_ID` | Google OAuth client ID | (required for Google sync) |
| `NEURAL_GOOGLE_CLIENT_SECRET` | Google OAuth client secret | (required for Google sync) |
| `NEURAL_MICROSOFT_CLIENT_ID` | Microsoft OAuth client ID | (required for Outlook sync) |
| `NEURAL_MICROSOFT_CLIENT_SECRET` | Microsoft OAuth client secret | (required for Outlook sync) |
| `NEURAL_TEAMS_CLIENT_ID` | Teams bot app ID | (required for Teams bot) |
| `NEURAL_TEAMS_CLIENT_SECRET` | Teams bot app secret | (required for Teams bot) |
| `NEURAL_TEAMS_TENANT_ID` | Teams tenant ID | `common` |
