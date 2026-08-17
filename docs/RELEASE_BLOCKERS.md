# Release Blockers — Personal Assistant Plan

Authoritative tracking for the plan items that must be completed before the
project can be called a production release. Status is updated one item at a
time (see `docs/PLAN_AUDIT.md` for the full audit).

## #3 — Automatic meeting detection and recording

**Status: COMPLETE (2026-08-14)** — scheduler-driven auto start/stop with
recording rules, plus rules management UI

- [x] Scheduler auto-detection — every scheduler tick (30 s) plans calendar
      events starting within ±2 minutes (`plan_auto_recordings`) and evaluates
      the recording rules per event
- [x] Auto-start — events matching an `automatic` or `notice_required` rule
      are recorded without prompting (rule engine enforced inside
      `start_local_recording`); a `recording_auto_started` marker on the event
      prevents double starts; started/failed outcomes are logged + audited
- [x] Auto-stop — auto-started recordings stop shortly after the event's
      scheduled end (graceful ffmpeg stop so the file finalizes)
- [x] Rule actions honoured — `confirm` events are never auto-recorded;
      `disabled` events are skipped
- [x] Rules management UI — Meetings view: add/list/delete rules
      (title/domain/participant × automatic/ask-first/never/notice, priority)
- [x] Observability — `auto_record_started` / `auto_record_stopped` /
      `auto_record_failed` audit entries + `log::info/error` with event_id
- [x] Tests — planner unit test (window filtering, rule actions, auto-started
      dedup), delete-rule test, 123 Rust + 40 live API + 532 frontend green

## #4 — Production Teams bot audio capture and secure transfer

**Status: IN PROGRESS — local hardening complete (2026-08-14); tenant-dependent
parts pending real Teams credentials**

- [ ] Replace simulated Teams audio capture (`teams-bot/bot.py` `capture_audio`
      stub) with a real Teams media-capture integration (Graph Communications
      API / bot media SDK) — requires tenant/Graph media access
- [x] Authenticate bot status and upload endpoints (shared-secret
      `X-Teams-Bot-Token`; `NAO_TEAMS_BOT_SECRET` on app and bot; 401 without
      a valid token)
- [x] Align upload format with the Rust API: raw binary body (nonce || AES-256-GCM
      ciphertext+tag) + `meeting_id`/`title` query params; server reads raw bytes
- [x] Encrypt recordings during transfer and validate/decrypt them locally
      (AES-256-GCM, key = SHA-256(secret); byte-for-byte decrypt verified in tests)
- [x] Preserve meeting state until transfer completes (transfer-before-leave;
      run-loop no longer skips active meetings)
- [x] Add retry, failure, and recovery handling for join/record/upload
      (bounded retries with state kept on failure; graceful shutdown hand-off)
- [x] Add integration tests with mocked Graph/media services (13 bot pytest
      tests; Rust crypto/auth unit tests; 8 live API tests incl. tamper rejection
      and decrypt roundtrip)
- [ ] Run a real Teams-tenant smoke test before marking complete
- [ ] Update COMPLETION_REPORT.md / TODO.md only after all of the above

## #5 — True multilingual/audio diarization

**Status: COMPLETE (2026-08-14)** — validated end-to-end with real speech, real
whisper, and a real audio diarization model

- [x] Explicit language selection (`en`/`hi`/`te`) for meeting transcription
      jobs — UI selector in Meetings view; `queue_transcription` /
      `reprocess_transcription` / job payload carry the language; whisper
      receives `--language`; auto-detect records whisper's detected language;
      Google cloud transcription maps to `en-US`/`hi-IN`/`te-IN` with
      alternatives
- [x] Confidence display — per-segment transcription confidence stored
      (`exp(avg_logprob)`, dampened by `no_speech_prob`) and speaker-assignment
      confidence in a separate `speaker_confidence` column; both shown as
      chips in the transcript UI
- [x] Audio-based speaker diarization backend — `scripts/diarize.py`
      (silero VAD + speechbrain ECAPA embeddings + silhouette/agglomerative
      clustering; optional pyannote path) resolved via `NEURAL_DIARIZE_BIN`/
      PATH; speaker windows mapped onto transcript segments by time overlap;
      text-embedding matcher kept as an explicitly-labelled fallback (audit +
      log record which backend ran)
- [x] Validate against a real audio diarization model — real speech fixtures
      (macOS `say` voices Lekha/hi-IN + Geeta/te-IN) diarized with the real
      backend: 2-speaker mixed hi/te conversation separated correctly
      (Lekha ↔ Geeta windows align with ground truth); single-speaker hi and
      te files each resolve to one speaker
- [x] Multilingual end-to-end validation — gated Rust test
      (`NAO_E2E_DIARIZE=1 cargo test --lib e2e_multilingual...`) runs real
      whisper (tiny) + the real diarizer through the full job pipeline on the
      hi/te/mixed fixtures: language stored (`hi`/`te`/auto→`hi`), per-segment
      confidence persisted, `diarization_audio` audit entries recorded, and
      ≥2 distinct speakers assigned to the mixed recording

## #7 — Full autonomous-agent tool execution

**Status: COMPLETE (2026-08-14)** — agent tool runtime implemented and tested;
real-model validation runs via the sidebar self-test when a local model is up

- [x] Tool execution runtime — `run_agent_now` is a tool-calling loop
      (`run_agent_loop`): the model may call workspace tools or answer in
      plain text, up to 6 steps, with tool results fed back
- [x] Tool catalogue — 20 tools across calendar, email, task, notes/memory,
      knowledge search, file, and command categories (`agent_tools.rs`)
- [x] Permissions — every tool is capability-gated via `check_permission`
      (read tools default-allowed; write tools require explicit grants);
      sensitive tools (`execute_command`, `modify_file`, `send_email`)
      enforce their own approval requests
- [x] Retries / recovery / execution state — tool failures are fed back to
      the model instead of aborting; step budget caps runaway loops; run
      records carry status, output, error, retry_count, and a tool-activity
      trace; scheduler retry logic unchanged
- [x] Observability — every tool call and run outcome is logged with
      run_id/agent_id/tool/args and recorded in the Diagnostics audit log
      (`agent_tool_executed`, `agent_run_completed`, `agent_run_failed`)
- [x] Tests — tool catalogue/parse/permission-gating unit tests + in-process
      loop tests (tool call → execute → feedback → final answer; recovery
      from tool failure; step-budget stop) — 9 tests, no network needed
- [ ] Agents UI shows the tool-activity trace prominently (run output already
      includes it; a dedicated step panel is a follow-up polish item)

## #8 — Complete export/restore integrity

**Status: COMPLETE (2026-08-14)** — v2 export format with integrity hashing
and full-domain coverage, verified by roundtrip + tamper tests

- [x] Full domain coverage — export/restore now includes notes, calendar
      events + participants, memories, source chunks, model routes,
      permissions (agent_permissions + workspace allowlists), web pages,
      email accounts, transcription jobs, bot runs, agent runs, speaker
      embeddings, plus the v1 domains (24 domains total)
- [x] Integrity — SHA-256 `content_hash` over the canonical payload +
      `schema_version` + per-domain `row_counts`; `verify_backup` recomputes
      the hash and rejects tampered/truncated files; `import_workspace`
      refuses to touch the DB on hash mismatch
- [x] Restore validation — import verifies version + hash, restores every
      domain in FK-safe order, and returns a per-domain restore summary that
      is logged and recorded in the audit log
- [x] Numeric fidelity — `table_export` now reads INTEGER/REAL columns via
      `ValueRef` (the old string reader silently nulled floats like
      confidence/start_seconds)
- [x] Regression tests — roundtrip across all domains (counts + row
      fidelity incl. floats/bools), tamper detection via hash + path-based
      `validate_backup`, empty-hash rejection, legacy v1 acceptance

## #9 — Full MCP/REST/CLI permission and tool restrictions

**Status: COMPLETE (2026-08-14)** — one gated tool surface across all three
channels, sharing the #7 runtime's permissions, approvals, logging, and audit

- [x] Audit every surface — MCP `/mcp`, REST `/v1/*`, and `neural-cli` were
      catalogued; MCP exposed only 16 hand-rolled tools, REST write endpoints
      were inconsistent, CLI was read-only
- [x] Unified tool catalogue — MCP `tools/list` and the new REST
      `/v1/tools/call` now enumerate the exact `agent_tools::AGENT_TOOLS`
      registry (20 tools incl. execute_command, modify_file, send_email,
      create_calendar_event, update_task_status) with workspace-scoped input
      schemas
- [x] Workspace-scoped + capability-scoped enforcement — every tool call goes
      through `agent_tools::execute_agent_tool` (workspace_id required except
      list_agent_runs; `check_permission` capability gating + workspace
      allowlists); unknown tools rejected before auth
- [x] Consistent authentication — all MCP `tools/call` and REST
      `/v1/tools/call` require an API key (read AND write tools); existing
      REST write endpoints already required keys
- [x] Approval enforcement parity — execute_command / modify_file / send_email
      create approval requests exactly as they do for the agent runtime (#7);
      approval-required errors map to HTTP 403 / JSON-RPC -32000
- [x] Observability — every call is logged with run_id/agent/tool/args and
      recorded in the Diagnostics audit log (`agent_tool_executed`)
- [x] CLI parity — `neural-cli tool <name> <workspace> <json>` calls the same
      gated endpoint with `NEURAL_API_KEY`; no special powers
- [x] Per-surface tests — Rust: catalogue/schema/capability consistency;
      live API: full tools/list (new tools + workspace schemas), read-tool
      401, unknown-tool 400, missing-workspace 400, /v1/tools/call 401 + 405
      (6 new live tests)
