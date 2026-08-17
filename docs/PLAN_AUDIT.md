# Personal Assistant Plan Audit

**Date:** 2026-08-14  
**Scope:** `docs/personal-assistant-plan.md` versus the current source, tests, runtime smoke tests, and distribution artifacts.

## Executive result

The core local database, Tauri command surface, sidebar workflows, model routing, search, meeting import/transcription, calendar/email domain logic, approvals, and export/deletion primitives are implemented and compile. They are **not yet a complete release implementation of every plan acceptance criterion**.

Verified on this machine:

- `cargo test`: **94 passed**
- `cargo check --examples`: **0 project warnings** (third-party `imap-proto` future-incompatibility notice remains)
- `npx tsc -b`: pass
- `npx vitest run`: **532 passed, 26 skipped** (live tests require the app)
- `npm run build`: pass
- sidebar self-test: **196 checks pass**; capture-device checks may be reported as clean environment skips when FFmpeg has no usable microphone
- live REST API: **26/26 pass** on successful runs; recording tests are device-sensitive

The normal frontend/Rust suites are not equivalent to the sidebar self-test: acceptance, plan-coverage, mock-pipeline, and parts of e2e use mocked `invoke` calls or inline reimplementations. They do not prove all native command/runtime paths.

Per-item completion criteria for the remaining release blockers are tracked in
`docs/RELEASE_BLOCKERS.md`.

## Fixes made during this audit

1. **Incremental indexing correctness:** `index_sources` now inspects URI-backed sources and compares content hashes before rebuilding. Previously indexed files were never re-read, so edits could be missed.
2. **Recording diagnostics:** recording stop now captures FFmpeg stderr and returns/logs a useful error when the process exits without producing a file or creates an empty file.
3. **Retention safety:** cleanup uses `COALESCE(started_at, created_at)`, meetings receive/migrate a `created_at` column, and scheduler cleanup removes files before nulling database paths. Automatic cleanup now defaults off, matching the plan's permanent-retention default.
4. **Permission capability consistency:** calendar mutation uses the plan capability `schedule_events`; `delete_data` is included in the declared capability set and approval checks.
5. **Diagnostics audit visibility:** Diagnostics now displays workspace audit events, including cloud-egress, permission, recording, sync, and deletion records.

## Plan status by area

### Implemented or substantially implemented

- Tauri + React + Rust + SQLite/FTS5 architecture.
- Workspaces, local database, data-directory migration, notes, memories, sources, ingestion, keyword/semantic/hybrid search, duplicate detection, citations for document/transcript context, deletion previews.
- Local recording, system/combined capture paths, batch import, drag/drop handler, Whisper job pipeline, transcript text/speaker editing, summaries, actions, voice profiles, queue visibility, retries.
- Calendar CRUD, recurrence expansion, OAuth/PKCE primitives, reminders, participants, invitations/ICS/RSVP, conflict detection, sync queue, meeting preparation.
- IMAP/SMTP primitives, OAuth sync primitives, email search/thread/labels/attachments/drafts/triage/action extraction.
- Model routes, LM Studio/OpenAI-compatible default routing, Ollama fallback support, cloud provider adapters, cost/token-limit storage, provider health/test commands, audit logging.
- Tasks/reminders, cron agents, permission/approval primitives, MCP/REST loopback API, CLI read commands, context export, JSON workspace export/import, retention and deletion primitives.
- Optional 3D component and conventional non-3D views.

### Partial or missing against the plan

#### Data/storage

- No distinct `projects` domain/table; projects are represented as workspaces.
- Sources do not preserve a general `modified_at`/parent-derived relationship model, and large imported files remain at original paths instead of being copied under the selected data root.
- Workspace export/restore is not a complete database backup: it omits several domains/relationships (for example full calendar/notes/attachments/permissions and all derived relationships). Differential backup is limited and timestamp semantics are incomplete.
- Source deletion removes indexed database data but does not remove the original source file.

#### Settings and policy

- UI settings are persisted as one JSON setting/localStorage object, while backend policy reads individual `app_settings` keys; several controls do not actually drive backend policy (`offlineOnly` vs UI `localOnlyMode`, `allowCloudText`, recording policy keys).
- Fallback chains are stored and health-marked, but provider selection/model selection is not consistently carried together through dispatch; this needs a dedicated provider+model routing test matrix.
- Token limits are stored/displayed but are not enforced in all chat, embedding, transcription, or cloud paths.
- Per-capability routing UI exposes only chat/transcription/embeddings/speech; plan capabilities reasoning, diarization, summarization, reranking, STT/TTS, and vision are not all selectable.

#### Meeting capture and transcription

- Calendar detection is a manual “Scan calendar” action; scheduler does not automatically detect events and start recording according to rules.
  **Update (2026-08-14):** automatic detection + recording is now implemented —
  the scheduler plans in-window events, evaluates recording rules, auto-starts
  `automatic`/`notice_required` meetings, auto-stops at the event end, and the
  Meetings view manages rules (see `docs/RELEASE_BLOCKERS.md` #3).
- Recording rules and monitored/designated folders have backend commands but no complete UI configuration flow.
- Separate mic/system tracks are not implemented; combined capture mixes tracks.
- Imported/batch recordings register rows but do not consistently enqueue the full pipeline automatically; manual Queue/reprocess remains necessary in some paths.
- Live/streaming transcript while recording is missing.
- Language selection is not exposed for meeting jobs; cloud transcription language handling is incomplete. Cloud-transcription-with-local-diarization is not wired end-to-end.
- Current diarization is transcript-text embedding matching, not audio speaker diarization. Confidence is stored but not fully displayed in the transcript UI.
  **Update (2026-08-14):** audio-based diarization is now available via
  `scripts/diarize.py` (silero VAD + speechbrain ECAPA + agglomerative
  clustering, optional pyannote), language selection (en/hi/te) is exposed and
  plumbed through the whole pipeline, per-segment transcription confidence and
  speaker-assignment confidence are stored and displayed, and the whole path
  was validated end-to-end with real hi/te speech (see `docs/RELEASE_BLOCKERS.md` #5).

#### Teams bot

- The bot is an external scaffold and is not verified against a real Teams tenant. `capture_audio` is a production stub in the browser bot. Bot upload/status routes are unauthenticated and do not implement the planned encrypted/private transfer contract; multipart upload and server expectations do not line up reliably. No real join/media/recovery test exists.

#### Calendar and email

- Calendar event create/update paths do not consistently enforce the plan's approval/permission model; sync conflict resolution has backend support but no complete UI controls.
- Time-zone selection and recurring-instance management are not complete in the UI.
- Google/Microsoft OAuth still requires user-supplied client configuration; this contradicts the plan's “preconfigured/no project” acceptance criterion.
- Mail UI does not expose the complete OAuth initiation, IMAP credential entry, thread-label management, or attachment-download/indexing flow. Gmail OAuth sync is metadata/snippet oriented; OAuth provider-API sending is not implemented (send uses SMTP credentials).

#### Assistant, agents, integrations, voice

- `run_agent_now` sends an agent prompt to a chat model and records text; it is not a complete tool/workflow execution runtime for calendar/email/file/command actions.
- MCP/REST/CLI surfaces are narrower than the plan's complete permission-aware tool matrix; repository/file context and mutation tools are not fully exposed.
- Agent activity visualization is not wired into the Agents UI.
- 3D view is primarily visual; it does not provide live transcript, real meeting-state visualization, or full voice interaction.
- Vision, reasoning-specific routing, true provider capability catalogs, and complete voice model routing are absent.
- Assistant citations do not consistently cite email/memory/conversation facts; conversation search is SQL `LIKE`, not a dedicated indexed transcript/conversation search.

## Distribution and release audit

### What exists

- `tauri.conf.json` targets `all` platforms and enables bundling/updater artifacts.
- A macOS arm64 `.app`, `.dmg`, and updater `.app.tar.gz` can be built locally.
- `Info.plist` includes microphone usage text.
- Updater public-key/endpoints scaffolding exists.

### Missing/blocking distribution work

1. **Updater signing is not currently reproducible:** `npx tauri build` creates the bundles but fails signing because `.tauri-keys/neural-agent-os.key` is encrypted and its password is unavailable. No `.sig` artifact was produced in the audit build.
2. **Updater infrastructure is a placeholder:** `https://updates.neural-agent.os/...` is not a hosted update server, and there is no release metadata publishing workflow.
3. **macOS production trust is incomplete:** generated output is ad-hoc/linker-signed; notarization and Developer ID signing are not configured. `codesign --verify --deep --strict`/`spctl` do not provide a production-trust result.
4. **Windows artifacts are missing:** no `.msi`/`.exe` was built or tested.
5. **Linux artifacts are missing:** no `.deb`/`.AppImage`/`.rpm` was built or tested.
6. **No CI release matrix:** no GitHub Actions/CI workflow builds, signs, tests, and publishes macOS/Windows/Linux artifacts.
7. **Runtime dependencies are not bundled:** FFmpeg, Whisper/model files, LM Studio/Ollama, and Playwright/Chromium are external prerequisites. Whisper and FFmpeg are marked optional in README even though core local capture/transcription hard-fails without them.
8. **Platform permissions are incomplete:** microphone declaration exists for macOS, but system-audio entitlement/permissions and Windows/Linux capture permissions have not been verified.
9. **No installer/update smoke matrix:** only local macOS arm64 runtime/API checks exist; clean-machine install, upgrade, rollback, and cross-platform smoke tests remain outstanding.

## Recommended release order

1. Fix policy/settings synchronization and enforce token/cost/cloud controls consistently.
2. Complete Mail OAuth/credentials/UI and calendar conflict-resolution UI.
3. Harden/authenticate Teams bot transfer and run real-tenant tests.
4. Complete true audio diarization, language selection, live transcript, and automatic meeting recording.
5. Define export/restore schema coverage and integrity hashes.
6. Configure a real updater server and protected signing secret; publish signed macOS artifacts.
7. Add CI jobs for macOS arm64/x64, Windows x64, Linux x64, installer smoke tests, and updater tests.
8. Only then change the release status from “audit-ready core” to “production release”.
