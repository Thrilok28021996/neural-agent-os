# Neural Agent OS — Verification TODO (updated)

**Last update:** August 12, 2026

(updated after sidebar verification pass)

## ✅ Completed in this pass

| # | Item | How it was completed / verified |
|---|---|---|
| 1 | Run live API tests automatically | `scripts/live-api-test.sh` starts the release app with an isolated temp data dir, waits for `/health`, runs `tests/integration/api.test.ts`, then shuts down. `npm run test:api`. Fixed the `runIf`-before-`beforeAll` bug that made all API tests always skip. **26/26 pass against the real app.** |
| 7 | Replace unreachable placeholder view | `SectionView` ("Foundation in progress") replaced with a deliberate `UnknownView` diagnostic. |
| 8 | Real integration tests for new REST/MCP surfaces | Added tests for calendar/events, emails, approvals, permissions, memories, context/export (echoes workspace id), tasks/create auth, recording start/stop loop, MCP write-tool auth. |
| 9 | Explicit job-queue integration tests | Refactored `handle_transcription_job` (connection-based) + `transcribe_meeting(&Connection)`; 4 new tests with a fake Whisper: FIFO transcription→segments→summarize-enqueue, missing meeting, missing meeting_id, unknown type fails. **89 Rust tests.** |
| 10 | Eliminate/wire Rust warnings | 23 → 1 (third-party `imap-proto`). Removed unused imports/fields; prefixed unused vars; wired `get_segment_embedding` into diarization and `record_bot_run` into the bot-status handler; annotated `resolve_provider` as a tested public API. |
| 11 | Queue visibility UI | Meetings view shows a live Processing queue panel (queued/running/failed counts, job list with retry counts, cancel, progress) polling every 8s. |
| 12 | Pipeline progress UI | Meeting rows show pipeline steps (queued / transcribed / failed / retry); queue panel shows transcription+summarization jobs. Live STT transcript while recording remains future (needs streaming STT). |
| 13 | True drag-and-drop import | Rust `WindowEvent::DragDrop(Drop)` handler imports dropped audio/video into the workspace, queues transcription, and emits `recording-dropped`; Meetings view listens and refreshes with a message. |
| 14 | Combined audio capture | macOS `source=combined` records mic + system audio mixed via `amix`; Capture UI gains "Microphone + System (combined)". |
| 15 | Diarization metadata fusion | Candidates now include calendar participants, previously corrected speaker labels on the meeting, and Teams bot identity from `bot_runs`. |
| 16 | Automatic updates / distribution | Generated signing keys (`.tauri-keys/`, gitignored), set `plugins.updater.pubkey` + endpoints, `bundle.createUpdaterArtifacts`, and produced `.tar.gz.sig` updater artifacts via `TAURI_SIGNING_PRIVATE_KEY`. |
| P2 extras | Duplicate cleanup UI | Knowledge view: "Delete all but first" per duplicate group. |
| P2 | Semantic email search UI | Mail view Semantic toggle (embedding-ranked search). |
| P2 | Calendar event reminders | Calendar view lists event-linked reminders with cancel/delete. |
| P2 | Provider connection test | `test_provider_connection` command + Test buttons in Models (OpenAI/Anthropic/Google/OpenAI-compatible/Ollama). |
| P2 | Backup verification | `verify_backup` command + Verify button in Data view. |
| P2 | Retention cleanup preview | Data view "Preview cleanup" (dry-run of 30-day policy). |
| P2 | Rate-limit/backoff | `send_with_retry` in chat.rs (retry once on 5xx/429). |
| P2 | DB migration versioning | `PRAGMA user_version` gating the ALTER migrations (schema v3). |
| P2 | Frontend code-split | Vite `manualChunks` (three/react); main bundle 810KB → 288KB. |

## ✅ Verified working with REAL tools

- Real Ollama chat: `POST /v1/ask` returned an actual LLM response (qwen3:0.6b) through provider-aware routing.
- Real Whisper transcription: macOS `say` speech → wav → app queue → Whisper (tiny) → transcript segments stored, meeting `transcribed`.
- Real summarization: same pipeline produced a markdown summary + 3 action items via Ollama.
- Live API suite (26 tests) passes against the packaged app.
- 131 Rust tests, 532 frontend tests, `tsc` clean, production build clean, updater `.sig` artifacts produced.

## 🔧 Real bugs found & fixed during this pass

- `send_invitation` built an email body but **never sent anything** (claimed `ics_attached: true`). Now actually sends via SMTP with the `.ics` attached, and only marks the participant invited on a successful send.
- Live API tests could never run: `it.runIf(apiAvailable)` evaluated before `beforeAll` set the flag (always skipped). Fixed with top-level `await`.
- `/v1/context/export` returned an empty `workspace_id` (fixed).
- Unknown background job types were reported as "completed" (now fail visibly).

## ⏳ Remaining — requires external credentials/infrastructure (cannot be completed from this environment)

- **P0-2 (partial):** install `openai-whisper` for end users (pipeline verified with a real install here).
- **P0-3 (partial):** manual GUI click-through of macOS permission prompts and native dialogs on a clean account (code + Info.plist verified).
- **P0-4:** cloud provider routing with real OpenAI/Anthropic/Google keys (routing verified with Ollama + a mock OpenAI-compatible server; Test buttons now let you verify live).
- **P0-5:** Google/Microsoft OAuth end-to-end with real registered clients.
- **P0-6:** Teams bot join against a real tenant (scripts verified to compile; Docker/Playwright setup in place).
- **P1-16 tail:** host the update server at the configured endpoint, publish signed artifacts, code-sign/notarize macOS, build Windows/Linux installers.
- **P2:** live-STT transcript while recording (streaming STT), provider model catalogs, accessibility review, splitting `lib.rs`.

## Build artifacts

- macOS app + dmg: `src-tauri/target/release/bundle/macos|dmg`
- Updater signature: `src-tauri/target/release/bundle/macos/Neural Agent OS.app.tar.gz.sig`
- Signing keys: `.tauri-keys/` (keep private; set `TAURI_SIGNING_PRIVATE_KEY` when building signed artifacts)



## ✅ Sidebar verification pass (this session)

| # | Item | How it was completed / verified |
|---|---|---|
| 20 | Verify every left-sidebar view end-to-end | New `run_sidebar_selftest` (`cargo run --example sidebar_selftest`, `npm run test:sidebar`): builds a real Wry app against an isolated temp data dir and exercises **all 196 Tauri commands** backing the 17 sidebar views — real SQLite schema, real whisper transcription, real Ollama chat/embeddings, real ffmpeg mic capture. **196/196 checks pass.** |
| 21 | **Fixed: Notes view could never save** | `create_note` computed `now` but never passed it — `INSERT INTO notes ... VALUES (?1..?5,?5)` received 4 params → every "Save note" click failed with "Wrong number of parameters". Extracted `insert_note()` helper + regression test `create_note_insert_matches_schema` (cargo test). |
| 22 | Warning cleanup | `cargo check --examples` warnings 21 → **0** (unused imports/vars, dead code in `calendar_extras`/`sync`/`health`/`email_complete`/`oauth`/`calendar_complete`/`reranking`/`meeting_prep`; annotated tested public APIs in `fallback`/`diarization`). |
| 23 | Command-surface audit | Static audit: all 188 frontend-invoked commands exist in the Rust `generate_handler` (0 missing); data-layer contract test covers 183 functions with correct command names + arg keys. |

**Verified numbers now:** 532 TS tests + 131 Rust tests + 26 live API tests + 196 sidebar command checks — all passing.

## 🔎 Full personal-plan and distribution audit (2026-08-14)

A source/runtime audit against `docs/personal-assistant-plan.md` is recorded in
`docs/PLAN_AUDIT.md`. Core tests pass, but the plan is not fully complete. The
remaining release blockers include policy/settings wiring, automatic meeting
recording, true audio diarization/live transcript, Teams transfer/authentication,
complete Gmail/Outlook OAuth and email credentials/UI, complete agent tools/MCP,
backup integrity/restore coverage, and production distribution (signed updater,
notarization, Windows/Linux artifacts, CI, and clean-machine install/update
smoke tests). Per-item completion criteria for the five production gaps
(#4 Teams bot, #5 audio diarization, #7 agent tools, #8 export/restore,
#9 MCP/REST/CLI permissions) are tracked in `docs/RELEASE_BLOCKERS.md`.
