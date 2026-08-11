# Neural Agent OS

Local-first desktop personal assistant. Records and processes meetings, creates multilingual transcripts with speaker diarization, summarizes conversations, manages tasks and reminders, and builds a cited knowledge base — all on your device.

**macOS · Windows · Linux** | **Offline-first** | **No account required**

> **Status (verified August 2026):** 189 Tauri commands · 80 Rust unit tests · 529 TypeScript tests passing (+18 live-API tests that run against a running app) · **99.07% real-code statement coverage / 100% lines** · macOS installer built and smoke-tested.

---

## Quick Start

### Prerequisites

| Tool | Purpose | Install |
|---|---|---|
| **Node.js 20+** | Frontend | [nodejs.org](https://nodejs.org) |
| **Rust** | Backend | [rustup.rs](https://rustup.rs) |
| **Ollama** (recommended) | Local AI chat, embeddings, summaries | `brew install ollama` or [ollama.com](https://ollama.com) |
| **FFmpeg** | Audio recording | `brew install ffmpeg` / `sudo apt install ffmpeg` |
| **Whisper** (optional) | Local transcription | `pip install openai-whisper` |
| **Playwright** (optional) | Meeting bot for Teams/Meet/Zoom | `pip install playwright && playwright install chromium` |

### Install & Run

```bash
# Clone
git clone https://github.com/neural-agent-os/neural-agent-os
cd neural-agent-os

# Install dependencies
npm install

# Pull AI models (one-time)
ollama pull qwen3:14b
ollama pull nomic-embed-text

# Start the desktop app (development)
npm run tauri dev

# Or build the release app + installer
npx tauri build
```

### First Launch

1. Neural Agent OS opens with three default workspaces: **Personal**, **Neural Agent OS**, **Research**
2. Choose a data directory for your SQLite database
3. No account creation required — everything stays local

---

## Features

### Workspaces
Isolate knowledge by context. Each workspace has its own:
- Ingested documents and folders
- Meeting transcripts and recordings
- Email accounts and calendar events
- Model preferences, token/cost limits, and fallback chains
- Autonomous agents and external-agent permissions
- Memories and workspace-level context exports

### Knowledge Base
- **Folder ingestion**: scan local folders for PDF, Markdown, TXT, DOCX, HTML, JSON, CSV
- **Web import**: fetch and index any public URL
- **Notes**: create, edit, search with full-text search
- **Memories**: durable facts that persist across conversations
- **Search**: keyword (FTS5), semantic (embeddings), **hybrid ranking** (merged + deduped), and reranking
- **Duplicate detection**: content-hash grouping of identical sources
- **Incremental indexing**: unchanged sources are skipped via content hashes
- **Citations**: every assistant answer cites its sources — including **meeting-timestamp citations** (`meeting_id` + `start_seconds`)

### Meetings
- **Import recordings**: MP3, WAV, M4A, MP4, MOV, WebM, OGG, FLAC (single or **batch**)
- **Local recording**: microphone or system audio via FFmpeg, with rule-engine gating and configurable notices; timestamp-named output (`~/Recordings/meeting-YYYY-MM-DD_HH-MM-SS.m4a`)
- **Auto-pipeline**: when a recording stops it is automatically imported as a meeting, and its **transcription and summarization run automatically** — back-to-back recordings are queued and processed **sequentially** (one per scheduler tick) via the durable background job queue
- **Transcription**: local Whisper or cloud (OpenAI/Google); **reprocessing** re-queues jobs
- **Transcript editing**: correct speaker labels *and* segment text
- **Diarization**: automatic speaker labeling from voice profiles **and calendar-participant names**
- **Summarization**: summaries, decisions, and action items (promotable to tasks)
- **Meeting bot**: Teams/Meet/Zoom via Playwright, with run history surfaced in-app

### Calendar
- **Built-in calendar**: create, edit, delete events with recurrence (simple + iCal RRULE)
- **Google / Outlook sync**: OAuth (PKCE), two-way, with conflict detection and resolution
- **Event reminders**: schedule reminders against calendar events
- **Invitations & RSVP**: iCal generation, sending, participant status
- **Time zones**: offset conversion
- **Recording rules**: per calendar/domain/participant/URL with priority ordering
- **Meeting preparation**: auto-gathers related emails, actions, and knowledge

### Email
- **Gmail / Outlook**: IMAP/SMTP or OAuth
- **Generic IMAP/SMTP**: self-hosted and custom providers
- **Thread reconstruction**: groups related messages (Re:/Fwd: chains)
- **Search**: full-text (FTS5) **and semantic** (embedding-ranked)
- **Triage**: keyword-classified inbox review
- **AI actions**: draft generation, summarization, action extraction, workspace auto-assignment
- **Labels, folders & attachments**: organize and index (PDF/DOCX/TXT/HTML/JSON/CSV)
- **Approval before send**: agent sends require explicit permission + approval

### AI & Models
- **Local runtimes**: Ollama, LM Studio, llama.cpp, vLLM, any OpenAI-compatible endpoint
- **Cloud providers**: OpenAI, Anthropic, Google, OpenCode Go plan
- **Per-capability routing**: chat, transcription, embeddings, speech, summarization — Ask Neural, agents, summaries, and email drafts all follow the route set in Models (provider + model + API key)
- **Provider fallback**: priority-ordered chains with health tracking and auto-skip
- **Cost tracking**: per-token pricing, cost limits, and comprehensive egress audit
- **Token limits**: per-capability ceilings on top of cost limits
- **Cloud data controls**: explicit toggles (`allowCloudAudio` / `allowCloudText`), offline-only mode, latency routing

### Automation
- **Tasks & reminders**: desktop notifications for due items (task- and event-linked)
- **Autonomous agents**: cron-scheduled, with retry (≤3), pause/resume, execution history, failure reporting
- **Approval workflow**: request → approve/deny/expire (24 h expiry), centralized `authorize_action`
- **Agent notifications**: desktop notifications on agent completion and failure
- **Permission modes**: approval-required, read-only, full
- **Daily briefing**: pre-built agent template; **research briefs** compile workspace context into summaries

### Voice & 3D
- **Voice input**: Web Speech API where available (Windows WebView2, browsers) and **push-to-talk mic capture → local Whisper** in the desktop runtime (macOS/Linux WebViews) — works in the packaged app
- **Text-to-speech**: local TTS (`say`/`espeak-ng`/SAPI) + browser fallback
- **3D assistant**: optional Three.js visualization (idle/listening/speaking/thinking)
- **Multilingual**: English, Hindi, Telugu

### Data Control & Privacy
- **Local-first**: SQLite + user-selected data directory; no account, no hosted backend
- **Backups**: full and differential, validated on creation; per-workspace retention policies
- **Deletion**: impact previews, selective derived-data deletion, cascading workspace delete
- **Retention cleanup**: recordings (30 d), sync queue (7 d), audit log (90 d), approvals (7 d)
- **Storage stats**: database/recording sizes with 5 GB/10 GB warnings
- **Secrets**: OS keychain for OAuth tokens and provider API keys

### External Agent Integration
- **MCP server**: 16 tools (search, transcripts, tasks, meetings, notes, reminders, calendar, email, memories, sync status, create note/reminder/task/memory) — loopback-only, permission-gated, approval-aware
- **REST API**: read + write endpoints with API-key auth and capability checks
- **CLI**: `neural-cli` for health, workspaces, search, tasks, meetings, notes, transcripts, ask
- **Context export**: markdown bundle of a workspace for Claude Code / Codex / OpenCode
- **Permission model**: the 10 plan capabilities (read_knowledge … execute_commands) + workspace allowlists

---

## Configuration

### Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `NEURAL_OLLAMA_URL` | Ollama generation endpoint | `http://127.0.0.1:11434/api/generate` |
| `NEURAL_OLLAMA_EMBEDDINGS_URL` | Ollama embeddings endpoint | `http://127.0.0.1:11434/api/embeddings` |
| `NEURAL_OPENAI_COMPATIBLE_URL` | Local OpenAI-compatible server (LM Studio, llama.cpp, vLLM) | *(unset)* |
| `NEURAL_WHISPER_BIN` | Whisper executable path | `whisper` |
| `NEURAL_FFMPEG_BIN` | FFmpeg executable path | `ffmpeg` |
| `NEURAL_AUDIO_DEVICE` | System audio capture device | Platform-specific |
| `NEURAL_OPENAI_API_KEY` | OpenAI API key (chat via Ask Neural/agents) | *(unset — or keychain)* |
| `NEURAL_ANTHROPIC_API_KEY` | Anthropic API key | *(unset — or keychain)* |
| `NEURAL_GOOGLE_API_KEY` | Google API key | *(unset — or keychain)* |
| `NEURAL_OPENAI_COMPATIBLE_API_KEY` | Optional bearer key for OpenAI-compatible runtimes | *(unset)* |
| `NEURAL_GOOGLE_CLIENT_ID` / `_SECRET` | Google OAuth (calendar/gmail) | *(unset)* |
| `NEURAL_MICROSOFT_CLIENT_ID` / `_SECRET` | Microsoft OAuth (outlook) | *(unset)* |
| `NEURAL_RECORDINGS_DIR` | Bot recording storage | `/tmp/neural-recordings` |

### OAuth Setup (Calendar & Email Sync)

**Google:**
1. [Google Cloud Console](https://console.cloud.google.com) → APIs & Services → Credentials
2. Create OAuth 2.0 Client ID → Desktop app
3. Redirect URI: `http://127.0.0.1:8787/oauth/callback`
4. Set `NEURAL_GOOGLE_CLIENT_ID` / `NEURAL_GOOGLE_CLIENT_SECRET`

**Microsoft:**
1. [Azure Portal](https://portal.azure.com) → App registrations
2. Redirect URI: `http://127.0.0.1:8787/oauth/callback`
3. Set `NEURAL_MICROSOFT_CLIENT_ID` / `NEURAL_MICROSOFT_CLIENT_SECRET`

---

## Development

```bash
npm install                 # dependencies
npm run dev                 # frontend only (Vite)
npm run tauri dev           # desktop app (Tauri)

npm run build               # production frontend (tsc -b && vite build)
npx tauri build             # release binary + installers

npm test                    # 529 TypeScript tests (+18 live-API tests)
npm run test:coverage       # coverage (99%+ statements)
npx tsc -b                  # type check
cargo test --manifest-path src-tauri/Cargo.toml    # 80 Rust unit tests
cargo check --manifest-path src-tauri/Cargo.toml   # Rust compile check
```

### Tests & Verification

| Suite | Count | Notes |
|---|---|---|
| TypeScript (vitest) | **529 passed + 18 skipped** | 10 files; the 18 API tests run only against a live app (`127.0.0.1:8787`) |
| Rust (cargo test) | **80 passed** | backend unit tests on in-memory SQLite with the real schema |
| Coverage | **99.07% stmts / 87.23% branch / 100% funcs / 100% lines** | data layer fully code-bound via generated contract tests |

Test layout:
- `tests/unit/data-layer-contract.test.ts` — generated contract tests binding every data-layer function to its exact backend command + arg keys (191 tests)
- `tests/unit/data-layer.test.ts` — data-layer behavior with mocked `invoke`
- `tests/validation/acceptance.test.ts` — all 20 plan §17 acceptance criteria, code-bound
- `tests/coverage/plan-coverage.test.ts` — all 18 plan §16 test areas, code-bound
- `tests/unit/{algorithms,infrastructure}.test.ts`, `tests/integration/{mock-pipeline,full-module,api}.test.ts`, `tests/pipeline/e2e.test.ts`
- Rust `#[cfg(test)]` modules in every backend file + `plan_extras.rs`

### Build & Distribution

Release artifacts (verified boot + REST/MCP/CLI smoke test):

```
src-tauri/target/release/neural-agent-os                       # raw binary (arm64, ~25 MB)
src-tauri/target/release/bundle/macos/Neural Agent OS.app      # macOS app bundle (ad-hoc signed)
src-tauri/target/release/bundle/dmg/Neural Agent OS_0.1.0_aarch64.dmg  # macOS installer
src-tauri/target/release/neural-cli                            # CLI binary
```

Remaining for production distribution: code signing/notarization, Windows/Linux bundles, and the auto-update server + signing key (the updater plugin is already wired).

### Project Structure

```
neural-agent-os/
├── src/                      # React + TypeScript frontend
│   ├── App.tsx               # All views (sidebar shell, 16 views)
│   ├── components/           # ThreeDAssistant (Three.js)
│   ├── data/                 # 66 data-layer modules (Tauri invoke bindings)
│   └── styles.css
├── src-tauri/                # Rust backend (22 modules)
│   └── src/
│       ├── lib.rs            # 189 Tauri commands, schema, scheduler, REST+MCP server
│       ├── plan_extras.rs    # Plan-completion features (token limits, memories, …)
│       ├── permissions.rs    # 10 capabilities + workspace allowlists (enforced)
│       ├── approvals.rs      # Approval workflow + 24h expiry
│       ├── calendar_complete.rs / calendar_extras.rs
│       ├── email_complete.rs # threads, labels, attachments, drafts
│       ├── diarization.rs    # speaker embeddings + participant fusion
│       ├── retention.rs      # backups, cleanup, audit log
│       ├── sync.rs           # two-way calendar sync + conflict resolution
│       ├── costs.rs / fallback.rs / health.rs / cloud.rs / oauth.rs
│       ├── job_queue.rs / monitoring.rs / meeting_prep.rs / deletion.rs
│       └── reranking.rs / cli.rs
├── teams-bot/                # Meeting bot (Playwright + Docker)
├── tests/                    # 529 TS tests (10 files) + Rust tests in src-tauri
└── docs/
    ├── USER_GUIDE.md
    ├── personal-assistant-plan.md
    ├── integrations.md       # REST/MCP/CLI/provider configuration
    └── LOCAL_ONLY.md
```

### Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2.5 (bundling: dmg / msi / deb) |
| Frontend | React 19, TypeScript, Three.js |
| Backend | Rust, SQLite (WAL), FTS5 |
| AI | Ollama, Whisper, OpenAI/Anthropic/Google APIs, OpenAI-compatible endpoints |
| Automation | Playwright, FFmpeg |
| Email | IMAP, SMTP, native-tls |
| OAuth | PKCE (Google, Microsoft) |

---

## API Reference

### REST API (`http://127.0.0.1:8787`, loopback-only)

**Read endpoints**

| Endpoint | Description |
|---|---|
| `GET /health` | Health check |
| `GET /diagnostics` | System diagnostics (DB size, workspaces, runtimes) |
| `GET /v1/workspaces` | List workspaces |
| `GET /v1/search?workspace_id=&q=` | Keyword search over knowledge |
| `GET /v1/tasks?workspace_id=` | List tasks |
| `GET /v1/meetings?workspace_id=` | List meetings |
| `GET /v1/notes?workspace_id=` | List notes |
| `GET /v1/transcripts?workspace_id=&q=` | Search transcripts |
| `GET /v1/calendar/events?workspace_id=` | List calendar events |
| `GET /v1/emails?workspace_id=` | List emails |
| `GET /v1/approvals?workspace_id=` | List pending approvals |
| `GET /v1/permissions?agent_id=&workspace_id=` | Permission summary for an agent |
| `GET /v1/memories?workspace_id=` | List memories |
| `GET /v1/context/export?workspace_id=` | Markdown context bundle (coding agents) |
| `POST /v1/ask` | Assistant chat (provider-routed) — body `{ workspace_id, prompt, model? }` |

**Write endpoints** (require an API key from `generate_api_key`, plus the listed capability)

| Endpoint | Capability |
|---|---|
| `POST /v1/notes/create` | `create_notes` |
| `POST /v1/reminders/create` | `create_reminders` |
| `POST /v1/tasks/create` | `modify_tasks` |

### MCP Server (`POST /mcp`, JSON-RPC 2.0)

16 tools, loopback-only, permission-gated (read tools use `read_knowledge`; write tools require an API key):

`search_workspace` · `search_transcripts` · `list_tasks` · `list_meetings` · `list_notes` · `list_reminders` · `list_calendar_events` · `list_emails` · `search_emails` · `get_sync_status` · `list_memories` · `list_agent_runs` · `create_note` · `create_reminder` · `create_task` · `create_memory`

### CLI

```bash
neural-cli health
neural-cli workspaces
neural-cli search personal "project roadmap"
neural-cli tasks personal
neural-cli meetings personal
neural-cli notes personal
neural-cli transcripts personal "action items"
neural-cli ask personal "Summarize my recent meetings"   # provider-routed assistant chat
```

The CLI is read-only by default and talks to the running desktop app over the loopback API.

---

## Meeting Bot (Browser Automation)

Automatically join Google Meet, Microsoft Teams, or Zoom meetings to record and transcribe them. No Azure registration, no Google Cloud project, no third-party APIs needed.

```
Calendar event → launch_meeting_bot() → Playwright opens Chromium
→ Joins meeting web UI → FFmpeg captures system audio
→ Audio file → Whisper transcription → Diarization → Summary
```

```bash
pip install playwright aiohttp && playwright install chromium
python teams-bot/meeting_bot.py google_meet "https://meet.google.com/abc-defg-hij" "Neural Bot" 60
python teams-bot/meeting_bot.py microsoft_teams "https://teams.microsoft.com/l/meetup-join/..." "Neural Bot" 45
python teams-bot/meeting_bot.py zoom "https://zoom.us/j/123456789" "Neural Bot" 30
```

Status reports from the bot are recorded in-app (`bot_runs`) and surfaced in the Meetings view. See `teams-bot/README.md` for platform-specific audio capture (BlackHole / VB-Cable / PulseAudio loopback).

---

## Verification History

- **August 11, 2026 — verification pass:** audited the original completion report; found real-code coverage was only ~16% (stub tests), a broken SQLite schema (app could not start), an unregistered `clean_subject` SQL function, and unenforced workspace allowlists. All fixed; the suite was rebuilt to be code-bound (see `COMPLETION_REPORT.md`).
- **August 11, 2026 — plan gap closure:** implemented every missing/partial plan item (memories, hybrid search, duplicate detection, semantic email search, token limits, provider runtimes, context export, batch import, transcript editing/reprocessing, triage, research briefs, event reminders, Teams-bot wiring, MCP/REST expansion, meeting-timestamp citations, incremental indexing, agent notifications).
- **August 11, 2026 — build pass:** produced and smoke-tested the macOS installer; fixed two runtime bugs found during smoke testing (REST server GET deadlock, SQLite `database is locked`).

---

## Roadmap & Known Limitations

- Code signing/notarization and Windows/Linux release builds (configs ready)
- Auto-update server + signing key (plugin wired; endpoints empty)
- True drag-and-drop recording import (batch path import works today)
- Live transcript display while recording (meeting visualization is minimal)
- Teams-bot join testing against a real tenant
- Additional languages beyond en/hi/te; performance tuning for very large datasets

---

## License

MIT
