# Integration Configuration

## Calendar

The built-in calendar works without external accounts. Google Calendar and Outlook connections are stored per workspace and begin in `needs_auth` state.

OAuth configuration is optional and is read from the local process environment:

- `NEURAL_GOOGLE_CLIENT_ID`
- `NEURAL_GOOGLE_CLIENT_SECRET`
- `NEURAL_MICROSOFT_CLIENT_ID`
- `NEURAL_MICROSOFT_CLIENT_SECRET`

The application must not require the user to create a Google Cloud project. The distributed application will use its own pre-registered OAuth client when production credentials are available. Development builds can use environment-provided clients.

## Local models and provider runtimes

- `NEURAL_OLLAMA_URL` controls the Ollama generation endpoint.
- `NEURAL_OLLAMA_EMBEDDINGS_URL` controls the Ollama embeddings endpoint.
- `NEURAL_WHISPER_BIN` selects a Whisper-compatible executable.
- `NEURAL_OPENAI_COMPATIBLE_URL` points at any local OpenAI-compatible server
  (LM Studio, llama.cpp, vLLM) used for model routes.
- `NEURAL_OPENAI_API_KEY`, `NEURAL_ANTHROPIC_API_KEY`, `NEURAL_GOOGLE_API_KEY`,
  `NEURAL_OPENAI_COMPATIBLE_API_KEY` supply cloud/remote API keys (alternatively
  stored in the OS keychain via the Models → Cloud API keys panel).
- `NEURAL_FFMPEG_BIN` overrides the FFmpeg binary used for local recording.

Supported provider runtimes (plan §10): Ollama, LM Studio, llama.cpp, vLLM,
OpenCode Go plan, arbitrary OpenAI-compatible endpoints, OpenAI, Anthropic, and
Google. Local runtimes are never treated as cloud for cost tracking or egress
audits; cloud providers are gated by the data-sharing settings.

## Local MCP and REST API

When the desktop application is running, it exposes a loopback-only API at `127.0.0.1:8787`.

Health check:

```bash
curl http://127.0.0.1:8787/health
```

Workspace-scoped REST endpoints (read):

```text
GET /v1/workspaces
GET /v1/search?workspace_id=<id>&q=<query>
GET /v1/tasks?workspace_id=<id>
GET /v1/meetings?workspace_id=<id>
GET /v1/notes?workspace_id=<id>
GET /v1/transcripts?workspace_id=<id>&q=<query>
GET /v1/calendar/events?workspace_id=<id>
GET /v1/emails?workspace_id=<id>
GET /v1/approvals?workspace_id=<id>
GET /v1/permissions?agent_id=<agent>&workspace_id=<id>
GET /v1/memories?workspace_id=<id>
GET /v1/context/export?workspace_id=<id>
POST /v1/ask            (body: { "workspace_id": <id>, "prompt": "...", "model": "optional" })

```

Write endpoints (require an API key and explicit permission):

```text
POST /v1/notes/create      (capability: create_notes)
POST /v1/reminders/create  (capability: create_reminders)
POST /v1/tasks/create      (capability: modify_tasks)
```

API keys are created with `generate_api_key` and revoked with `revoke_api_key`.

MCP-compatible JSON-RPC endpoint:

```text
POST /mcp
```

Exposed MCP tools (read tools use `read_knowledge`; write tools require an API
key and their listed capability):

- `search_workspace` — keyword search over indexed knowledge
- `search_transcripts` — search meeting transcripts
- `list_tasks` / `list_meetings` / `list_notes` / `list_reminders`
- `list_calendar_events` / `list_emails` / `search_emails` / `get_sync_status`
- `list_memories` / `list_agent_runs`
- `create_note` (create_notes) / `create_reminder` (create_reminders)
- `create_task` (modify_tasks) / `create_memory` (create_notes)

The endpoint is loopback-only. External coding agents should connect only when
the user explicitly enables local access. Write tools are permission-gated and
approval-aware.

## CLI

`neural-cli` talks to the running desktop app over the loopback API:

```text
neural-cli health | workspaces | search <ws> <q> | tasks <ws> |
                 meetings <ws> | notes <ws> | transcripts <ws> <q> | ask <ws> <prompt>
```

## Context export for coding agents

`export_context` (and REST `GET /v1/context/export`) produce a markdown bundle
of a workspace — sources, notes, tasks, meetings, memories, and agents — that
can be handed to Claude Code, Codex, or OpenCode as grounding context.
