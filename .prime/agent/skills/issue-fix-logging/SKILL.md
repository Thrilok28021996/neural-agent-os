---
name: issue-fix-logging
description: >-
  Use whenever fixing a bug, error, or runtime issue in the neural-agent-os
  codebase (Rust backend in src-tauri/src or the React frontend in src/). After
  applying the fix, add structured logging so the error stays trackable: Rust
  log::error!/warn! with command + entity ids, retention::record_audit entries
  for the Diagnostics audit log, frontend logEvent() calls in catch blocks, and
  a regression test where practical. Applies to both backend (Tauri commands,
  SQL, job queue) and frontend (UI handlers, data layer) fixes.
---

# Issue-fix logging (neural-agent-os)

Rule: **every fixed issue gets (1) the fix, (2) structured logging in the error
path, (3) a regression test where practical.** This keeps every error
trackable through the app's own diagnostics instead of only in the fixer's
memory.

## Where the logs go

- Rust `log::*` macros → rotating file at `<data_root>/logs/neural-agent-os.log`
  (initialized by `logging::init` in setup). Viewable in the app under
  **Diagnostics → Logs** (`read_app_log` command) or on disk.
- `retention::record_audit(...)` → `audit_log` table → **Diagnostics → Audit
  log** (user-visible history).
- Frontend `logEvent(level, source, message)` → forwarded to the same on-disk
  log via the `log_event` Tauri command (the webview cannot write files).

## Backend (Rust) — do this after the fix

1. In every error path you touched, log **before** returning `Err`, with the
   function/command name and the entity ids involved:

   ```rust
   Err(error) => {
       log::error!("create_note: insert failed (workspace={workspace_id}, note={id}): {error}");
       Err(error)
   }
   ```

   Follow existing conventions:
   `log::error!("process_transcription_job: job {job_id} failed: {error}")`,
   `log::warn!("download_model: empty model name")`,
   `log::info!("import_meeting_recording: imported {path} -> {meeting_id}")`.

2. For user-meaningful actions (deletes, sends, config changes, permission
   decisions), also write an audit entry:

   ```rust
   retention::record_audit(
       &connection,
       "recording_deleted",          // snake_case action name
       None,                         // provider (Some("openai") for cloud)
       Some("audio"),                // data kind
       None,                         // workspace_id
       &format!("Deleted recording file for meeting {meeting_id}"),
   )?;
   ```

   Audit actions already in use: `recording_deleted`, `command_executed`,
   `calendar_invitation_sent`, `cloud_chat`, `cloud_embeddings`.

3. If a command's failure is only visible through its return value (e.g. REST
   or MCP handlers), log inside the command body before the error is returned.

## Frontend (TS) — do this after the fix

In the view/component handler you touched, log in `catch` blocks and key
lifecycle points:

```ts
try {
  const meeting = await importMeetingRecording(workspace.id, path.trim(), title)
  logEvent('info', 'meetings', `imported meeting ${meeting.id}`)
} catch (error) {
  logEvent('error', 'meetings', `import failed: ${errMsg(error)}`)
  setMessage(errMsg(error))
}
```

- Levels: `'info'` (lifecycle), `'warn'` (degraded), `'error'` (failures).
- Source: the view name (`'meetings'`, `'notes'`, `'knowledge'`, `'window'` for
  uncaught errors — App.tsx already forwards window errors automatically).
- `logEvent` comes from `./data/logging`; `errMsg` is defined in `App.tsx`.

## Regression test

Where practical, add a test that fails without the fix:

- Rust: unit test against the real schema via `crate::tests::test_connection()`
  (seed workspaces first — `open_database` seeds them, `init_schema` does not).
  Example: `create_note_insert_matches_schema` guards the notes INSERT param
  count. If the SQL lives inside a command body, extract the DB part into a
  `fn *_db(connection, ...)` helper (e.g. `insert_note`) so the test can bind
  to it.
- TS: extend `tests/unit/data-layer-contract.test.ts` (command name + arg keys)
  or add a unit test for the fixed helper.

## Verification (always after logging edits)

```bash
cargo check --examples          # must stay at 0 warnings
cargo test                      # Rust suite incl. new regression test
npx tsc -b && npx vitest run    # frontend type-check + tests
npm run test:api                # live REST/MCP tests (requires release build)
npm run test:sidebar            # 197-command sidebar self-test (real app)
```

Re-run the full suites after logging/warning cleanup — a cleanup that compiles
is not proof the behavior still works.

## Real example (this repo)

`create_note` computed `now` but never passed it to the INSERT (4 params for
5 slots) — every Notes save failed at runtime while all mocked tests passed.
Fix: `insert_note(&connection, &id, &workspace_id, title, content, &now)` helper
+ `log::error!` on failure + regression test `create_note_insert_matches_schema`
bound to the real schema.
