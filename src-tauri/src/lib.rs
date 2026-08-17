mod cloud;
pub mod logging;
mod fallback;
mod costs;
mod reranking;
mod calendar_extras;
mod monitoring;
mod meeting_prep;
mod diarization;
mod job_queue;
mod permissions;
mod retention;
mod sync;
mod approvals;
mod oauth;
mod deletion;
mod calendar_complete;
mod email_complete;
mod health;
mod plan_extras;
mod chat;
mod teams_bot;
mod agent_tools;
mod sidebar_selftest;
pub use sidebar_selftest::run_sidebar_selftest;

use tauri::{Manager, Emitter};
use rusqlite::{params, Connection};
use walkdir::WalkDir;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read as _;
use mailparse::MailHeaderMap;
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use tauri_plugin_notification::NotificationExt;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

// ── Unified data layout ────────────────────────────────────────────────────
// Everything Neural Agent OS creates or stores lives under ONE root folder:
//   <root>/database/           SQLite database (+ WAL/SHM)
//   <root>/logs/               application log (+ rotated .log.1)
//   <root>/backups/            workspace backups
//   <root>/recordings/         captured audio (default output path)
//   <root>/transcripts/        whisper JSON/TXT outputs
//   <root>/models/             Hugging Face model cache
//   <root>/tmp/                transient voice-capture / intermediates
//   <root>/teams-recordings/   Teams bot uploads
// The root defaults to ~/Neural Agent OS and can be overridden with
// NEURAL_DATA_DIR or the legacy data-directory.txt marker.
static DATA_ROOT: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

pub(crate) fn set_data_root(root: std::path::PathBuf) {
    if let Ok(mut slot) = DATA_ROOT.lock() { *slot = Some(root); }
}

pub(crate) fn data_root() -> std::path::PathBuf {
    if let Ok(slot) = DATA_ROOT.lock() {
        if let Some(root) = slot.as_ref() { return root.clone(); }
    }
    std::env::var("NEURAL_DATA_DIR")
        .map(std::path::PathBuf::from)
        .ok()
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Neural Agent OS"))
}

const DATA_SUBDIRS: &[&str] = &["database", "logs", "backups", "recordings", "transcripts", "models", "tmp", "teams-recordings"];

/// Absolute path of a named subfolder under the data root (created on demand).
pub(crate) fn data_subdir(name: &str) -> std::path::PathBuf {
    let dir = data_root().join(name);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Create the full folder tree under the data root.
pub(crate) fn ensure_data_layout() {
    let root = data_root();
    let _ = std::fs::create_dir_all(&root);
    for sub in DATA_SUBDIRS {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
}

/// Move files written by older versions (flat app-data directory) into the
/// unified layout. Runs once at startup, before the database is opened.
fn migrate_legacy_layout(app: &tauri::AppHandle, root: &std::path::Path) {
    let Ok(old) = app.path().app_data_dir() else { return };
    if old == root { return; }
    let move_file = |src: std::path::PathBuf, dst: std::path::PathBuf| {
        if src.is_file() && !dst.exists() {
            if std::fs::rename(&src, &dst).is_err() {
                let _ = std::fs::copy(&src, &dst);
                let _ = std::fs::remove_file(&src);
            }
            log::info!("data migration: {} -> {}", src.display(), dst.display());
        }
    };
    move_file(old.join("neural-agent-os.sqlite"), root.join("database/neural-agent-os.sqlite"));
    move_file(old.join("neural-agent-os.sqlite-wal"), root.join("database/neural-agent-os.sqlite-wal"));
    move_file(old.join("neural-agent-os.sqlite-shm"), root.join("database/neural-agent-os.sqlite-shm"));
    move_file(old.join("neural-agent-os.log"), root.join("logs/neural-agent-os.log"));
    move_file(old.join("neural-agent-os.log.1"), root.join("logs/neural-agent-os.log.1"));
    if let Ok(entries) = std::fs::read_dir(old.join("backups")) {
        for entry in entries.flatten() {
            move_file(entry.path(), root.join("backups").join(entry.file_name()));
        }
    }
    if let Ok(entries) = std::fs::read_dir(old.join("teams-recordings")) {
        for entry in entries.flatten() {
            move_file(entry.path(), root.join("teams-recordings").join(entry.file_name()));
        }
    }
}

fn database_path(_app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(data_subdir("database").join("neural-agent-os.sqlite"))
}

fn open_database(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    log::debug!("open_database: {}", path.display());
    let connection = Connection::open(&path).map_err(|error| error.to_string())?;
    init_schema(&connection)?;
    // Backfill the creation timestamp for databases created before this
    // column existed; retention uses it when imported meetings lack started_at.
    let _ = connection.execute("ALTER TABLE meetings ADD COLUMN created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP", []);
    let _ = connection.execute("ALTER TABLE meetings ADD COLUMN language TEXT NOT NULL DEFAULT 'en'", []);
    let _ = connection.execute("ALTER TABLE transcription_jobs ADD COLUMN language TEXT", []);
    let _ = connection.execute("ALTER TABLE transcript_segments ADD COLUMN speaker_confidence REAL", []);
    let _ = connection.execute("UPDATE meetings SET created_at = COALESCE(created_at, started_at, CURRENT_TIMESTAMP) WHERE created_at IS NULL", []);
connection.execute(
        "INSERT OR IGNORE INTO workspaces (id, name, source_count) VALUES (?1, ?2, ?3), (?4, ?5, ?6), (?7, ?8, ?9)",
        params!["personal", "Personal", 8, "neural-os", "Neural Agent OS", 12, "research", "Research", 5],
    ).map_err(|error| error.to_string())?;
    // Default model routing: LM Studio is the default provider for chat,
    // summarization, and embeddings (override per workspace in the Models
    // view; existing explicit routes are left untouched). The model ids are
    // configurable via NEURAL_OPENAI_COMPATIBLE_MODEL and
    // NEURAL_OPENAI_COMPATIBLE_EMBEDDING_MODEL.
    let chat_model = default_lmstudio_model();
    let embed_model = default_lmstudio_embedding_model();
    for (capability, model) in [("chat", &chat_model), ("summarization", &chat_model), ("embeddings", &embed_model)] {
        connection.execute(
            "INSERT OR IGNORE INTO model_routes (workspace_id, capability, provider, model)
             SELECT id, ?1, 'lmstudio', ?2 FROM workspaces",
            params![capability, model],
        ).map_err(|error| error.to_string())?;
    }
    Ok(connection)
}

/// Resolve a user-supplied output path to an absolute, writable location.
/// Desktop apps launched from Finder/Dock have a working directory of `/`,
/// which is not writable — so relative paths (`recordings/x.m4a`,
/// `exports/x.json`) are resolved against the app data directory, and `~/`
/// is expanded to the user's home.
fn resolve_output_path_inner(home: &str, data_dir: &std::path::Path, path: &str) -> Result<std::path::PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Path cannot be empty".into());
    }
    let expanded = if let Some(rest) = trimmed.strip_prefix("~/") {
        std::path::PathBuf::from(home).join(rest)
    } else {
        std::path::PathBuf::from(trimmed)
    };
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Ok(data_dir.join(expanded))
    }
}

fn resolve_output_path(_app: &tauri::AppHandle, path: &str) -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    // Relative/~ paths resolve inside the unified data root so everything the
    // app writes stays in one folder.
    resolve_output_path_inner(&home, &data_root(), path)
}

/// Locate an external binary. macOS GUI apps launched from Finder have a
/// minimal PATH (/usr/bin:/bin:...), so Homebrew-installed tools like FFmpeg
/// or Whisper would otherwise be "not found". Checks the explicit env var,
/// well-known install locations, then PATH.
pub(crate) fn resolve_binary(name: &str, env_var: &str) -> Result<String, String> {
    if let Ok(path) = std::env::var(env_var) {
        if !path.trim().is_empty() {
            return Ok(path.trim().to_string());
        }
    }
    let candidates = [
        format!("/opt/homebrew/bin/{name}"),
        format!("/usr/local/bin/{name}"),
        format!("/opt/local/bin/{name}"),
        format!("/usr/bin/{name}"),
        format!("/bin/{name}"),
    ];
    for candidate in candidates {
        if std::path::Path::new(&candidate).is_file() {
            return Ok(candidate);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let candidate = format!("{dir}/{name}");
            if std::path::Path::new(&candidate).is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "{name} not found. Install it (e.g. `brew install ffmpeg` / `pip install openai-whisper`) or set {env_var}."
    ))
}

pub(crate) fn init_schema(connection: &Connection) -> Result<(), String> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 10000;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS app_settings (
           key TEXT PRIMARY KEY NOT NULL,
           value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS conversations (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           role TEXT NOT NULL,
           content TEXT NOT NULL,
           model TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS agents (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           name TEXT NOT NULL,
           prompt TEXT NOT NULL,
           schedule TEXT NOT NULL,
           permission_mode TEXT NOT NULL DEFAULT 'approval_required',
           status TEXT NOT NULL DEFAULT 'paused',
           last_run_at TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS model_routes (
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           provider TEXT NOT NULL,
           model TEXT NOT NULL,
           PRIMARY KEY(workspace_id, capability)
         );
          CREATE TABLE IF NOT EXISTS agent_runs (
           id TEXT PRIMARY KEY NOT NULL,
           agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
           status TEXT NOT NULL DEFAULT 'running',
           output TEXT,
            error TEXT,
            retry_count INTEGER NOT NULL DEFAULT 0,
           started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           completed_at TEXT
          );
         CREATE TABLE IF NOT EXISTS workspaces (
           id TEXT PRIMARY KEY NOT NULL,
           name TEXT NOT NULL,
           source_count INTEGER NOT NULL DEFAULT 0,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS sources (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           kind TEXT NOT NULL,
           title TEXT NOT NULL,
           uri TEXT,
           status TEXT NOT NULL DEFAULT 'pending',
           content_hash TEXT,
           indexed_at TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS source_chunks (
           id TEXT PRIMARY KEY NOT NULL,
           source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
           chunk_index INTEGER NOT NULL,
           content TEXT NOT NULL,
           UNIQUE(source_id, chunk_index)
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS source_search USING fts5(
           content,
           source_id UNINDEXED,
           chunk_id UNINDEXED
         );
         CREATE TABLE IF NOT EXISTS meetings (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           title TEXT NOT NULL,
           started_at TEXT,
           duration_seconds INTEGER,
           recording_path TEXT,
           status TEXT NOT NULL DEFAULT 'created',
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           language TEXT NOT NULL DEFAULT 'en'
         );
         CREATE TABLE IF NOT EXISTS transcription_jobs (
           id TEXT PRIMARY KEY NOT NULL,
           meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
           provider TEXT NOT NULL DEFAULT 'unconfigured',
           status TEXT NOT NULL DEFAULT 'queued',
           language TEXT,
           error TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS meeting_summaries (
           meeting_id TEXT PRIMARY KEY NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
           model TEXT NOT NULL,
           summary TEXT NOT NULL,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS meeting_actions (
           id TEXT PRIMARY KEY NOT NULL,
           meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
           title TEXT NOT NULL,
           owner TEXT,
           due_at TEXT,
           status TEXT NOT NULL DEFAULT 'open'
         );
         CREATE TABLE IF NOT EXISTS transcript_segments (
           id TEXT PRIMARY KEY NOT NULL,
           meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
           speaker TEXT,
           start_seconds REAL NOT NULL,
           end_seconds REAL NOT NULL,
           text TEXT NOT NULL,
           confidence REAL,
           speaker_confidence REAL
         );
         CREATE TABLE IF NOT EXISTS tasks (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           title TEXT NOT NULL,
           due_at TEXT,
           status TEXT NOT NULL DEFAULT 'open'
         );"
    ).map_err(|error| error.to_string())?;
        connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS reminders (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
           calendar_event_id TEXT REFERENCES calendar_events(id) ON DELETE CASCADE,
           title TEXT NOT NULL,
           scheduled_at TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'scheduled',
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS calendar_events (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           title TEXT NOT NULL,
           starts_at TEXT NOT NULL,
           ends_at TEXT NOT NULL,
            meeting_url TEXT,
            recurrence TEXT,
           provider TEXT NOT NULL DEFAULT 'local',
           external_id TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           updated_at TEXT,
           recording_auto_started TEXT
         );
         CREATE TABLE IF NOT EXISTS calendar_connections (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           provider TEXT NOT NULL,
           account_label TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'needs_auth',
           last_synced_at TEXT,
           UNIQUE(workspace_id, provider, account_label)
         );
         CREATE TABLE IF NOT EXISTS email_accounts (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           provider TEXT NOT NULL,
           account_label TEXT NOT NULL,
           imap_host TEXT,
           smtp_host TEXT,
           status TEXT NOT NULL DEFAULT 'needs_auth',
           UNIQUE(workspace_id, provider, account_label)
         );
         CREATE TABLE IF NOT EXISTS emails (
           id TEXT PRIMARY KEY NOT NULL,
           account_id TEXT NOT NULL REFERENCES email_accounts(id) ON DELETE CASCADE,
           thread_id TEXT,
           subject TEXT NOT NULL,
           sender TEXT,
           recipients TEXT,
           body TEXT,
           received_at TEXT,
           provider_id TEXT,
           UNIQUE(account_id, provider_id)
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS email_search USING fts5(
           subject,
           sender,
           body,
           email_id UNINDEXED,
           workspace_id UNINDEXED
         );
         CREATE TABLE IF NOT EXISTS chunk_embeddings (
           chunk_id TEXT PRIMARY KEY NOT NULL REFERENCES source_chunks(id) ON DELETE CASCADE,
           model TEXT NOT NULL,
           vector_json TEXT NOT NULL,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS notes (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           title TEXT NOT NULL,
           content TEXT NOT NULL,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS note_search USING fts5(
           title,
           content,
           note_id UNINDEXED,
           workspace_id UNINDEXED
         );
         CREATE TABLE IF NOT EXISTS web_pages (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           url TEXT NOT NULL,
           title TEXT NOT NULL,
           fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS voice_profiles (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           speaker_label TEXT NOT NULL,
           sample_count INTEGER NOT NULL DEFAULT 0,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS calendar_participants (
           id TEXT PRIMARY KEY NOT NULL,
           event_id TEXT NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
           name TEXT NOT NULL,
           email TEXT,
           status TEXT NOT NULL DEFAULT 'invited'
         );
         CREATE TABLE IF NOT EXISTS token_usage (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           provider TEXT NOT NULL,
           model TEXT NOT NULL,
           input_tokens INTEGER NOT NULL DEFAULT 0,
           output_tokens INTEGER NOT NULL DEFAULT 0,
           cost_cents INTEGER NOT NULL DEFAULT 0,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS cost_limits (
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           limit_cents INTEGER NOT NULL,
           PRIMARY KEY(workspace_id, capability)
         );
         CREATE TABLE IF NOT EXISTS provider_fallback (
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           priority INTEGER NOT NULL,
           provider TEXT NOT NULL,
           model TEXT NOT NULL,
           healthy INTEGER NOT NULL DEFAULT 1,
           PRIMARY KEY(workspace_id, capability, priority)
         );
         CREATE TABLE IF NOT EXISTS monitored_folders (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           path TEXT NOT NULL,
           enabled INTEGER NOT NULL DEFAULT 1,
           last_checked_at TEXT
         );
         CREATE TABLE IF NOT EXISTS speaker_embeddings (
           profile_id TEXT PRIMARY KEY NOT NULL REFERENCES voice_profiles(id) ON DELETE CASCADE,
           model TEXT NOT NULL,
           vector_json TEXT NOT NULL,
           sample_count INTEGER NOT NULL DEFAULT 0,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS background_jobs (
           id TEXT PRIMARY KEY NOT NULL,
           job_type TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'queued',
           progress_pct INTEGER NOT NULL DEFAULT 0,
           progress_message TEXT NOT NULL DEFAULT '',
           workspace_id TEXT NOT NULL,
           payload_json TEXT NOT NULL DEFAULT '{}',
           error TEXT,
           retry_count INTEGER NOT NULL DEFAULT 0,
           max_retries INTEGER NOT NULL DEFAULT 3,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           started_at TEXT,
           completed_at TEXT,
           locked_by TEXT
         );
         CREATE TABLE IF NOT EXISTS agent_permissions (
           agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           granted INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY(agent_id, workspace_id, capability)
         );
         CREATE TABLE IF NOT EXISTS workspace_allowlists (
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           PRIMARY KEY(workspace_id, capability)
         );
         CREATE TABLE IF NOT EXISTS audit_log (
           id TEXT PRIMARY KEY NOT NULL,
           action TEXT NOT NULL,
           provider TEXT,
           data_type TEXT,
           workspace_id TEXT,
           details TEXT NOT NULL DEFAULT '',
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS sync_queue (
           id TEXT PRIMARY KEY NOT NULL,
           entity_type TEXT NOT NULL,
           entity_id TEXT NOT NULL,
           provider TEXT NOT NULL,
           action TEXT NOT NULL,
           payload_json TEXT NOT NULL DEFAULT '{}',
           status TEXT NOT NULL DEFAULT 'pending',
           retry_count INTEGER NOT NULL DEFAULT 0,
           error TEXT,
           conflict_details TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           synced_at TEXT
         );
         CREATE TABLE IF NOT EXISTS approval_requests (
           id TEXT PRIMARY KEY NOT NULL,
           agent_id TEXT NOT NULL,
           workspace_id TEXT NOT NULL,
           capability TEXT NOT NULL,
           action_description TEXT NOT NULL,
           payload_json TEXT NOT NULL DEFAULT '{}',
           status TEXT NOT NULL DEFAULT 'pending',
           requested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           resolved_at TEXT,
           resolved_by TEXT
         );
         CREATE TABLE IF NOT EXISTS recording_rules (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           rule_type TEXT NOT NULL,
           pattern TEXT NOT NULL,
           action TEXT NOT NULL DEFAULT 'confirm',
           priority INTEGER NOT NULL DEFAULT 0,
           enabled INTEGER NOT NULL DEFAULT 1
         );
         CREATE TABLE IF NOT EXISTS email_labels (
           id TEXT PRIMARY KEY NOT NULL,
           account_id TEXT NOT NULL REFERENCES email_accounts(id) ON DELETE CASCADE,
           name TEXT NOT NULL,
           provider_label_id TEXT,
           color TEXT
         );
         CREATE TABLE IF NOT EXISTS email_label_assignments (
           email_id TEXT NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
           label_id TEXT NOT NULL REFERENCES email_labels(id) ON DELETE CASCADE,
           PRIMARY KEY(email_id, label_id)
         );
         CREATE TABLE IF NOT EXISTS email_attachments (
           id TEXT PRIMARY KEY NOT NULL,
           email_id TEXT NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
           filename TEXT NOT NULL,
           content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
           size_bytes INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS provider_health_log (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           provider TEXT NOT NULL,
           capability TEXT NOT NULL,
           workspace_id TEXT NOT NULL,
           healthy INTEGER NOT NULL DEFAULT 1,
           response_time_ms INTEGER,
           error_message TEXT,
           consecutive_failures INTEGER NOT NULL DEFAULT 0,
           checked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS token_limits (
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           capability TEXT NOT NULL,
           limit_tokens INTEGER NOT NULL,
           PRIMARY KEY(workspace_id, capability)
         );
         CREATE TABLE IF NOT EXISTS memories (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
           content TEXT NOT NULL,
           source_kind TEXT,
           source_id TEXT,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS bot_runs (
           id TEXT PRIMARY KEY NOT NULL,
           workspace_id TEXT NOT NULL,
           bot_id TEXT NOT NULL,
           status TEXT NOT NULL,
           message TEXT,
           meeting_id TEXT,
           platform TEXT NOT NULL DEFAULT 'teams',
           reported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );"
     ).map_err(|error| error.to_string())?;
    // Upgrade databases created before retry metadata was introduced.
    let _ = connection.execute("ALTER TABLE agent_runs ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0", []);
    let _ = connection.execute("ALTER TABLE calendar_events ADD COLUMN recurrence TEXT", []);
    let _ = connection.execute("ALTER TABLE calendar_events ADD COLUMN location TEXT", []);
    let _ = connection.execute("ALTER TABLE reminders ADD COLUMN calendar_event_id TEXT", []);
    let _ = connection.execute("ALTER TABLE sources ADD COLUMN content_hash TEXT", []);
    let _ = connection.execute("ALTER TABLE sources ADD COLUMN indexed_at TEXT", []);
    let _ = connection.execute("ALTER TABLE calendar_events ADD COLUMN updated_at TEXT", []);
    let _ = connection.execute("ALTER TABLE calendar_events ADD COLUMN recording_auto_started TEXT", []);

    // Register the clean_subject SQL function used by email thread reconstruction.
    connection.create_scalar_function(
        "clean_subject",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let subject: String = ctx.get(0)?;
            Ok(crate::email_complete::clean_subject(&subject))
        },
    )
    .map_err(|e| e.to_string())?;

        Ok(())
}


pub(crate) fn routed_model(connection: &Connection, workspace_id: &str, capability: &str, fallback: &str) -> String {
    // Offline-only mode: if enabled, skip all cloud providers
    let offline_only: bool = connection.query_row(
        "SELECT value = 'true' FROM app_settings WHERE key = 'offlineOnly'",
        [], |row| row.get(0)
    ).unwrap_or(false);

    // Latency preference: if set, prefer local models when latency threshold is tight
    let latency_pref: String = connection.query_row(
        "SELECT value FROM app_settings WHERE key = 'latencyPreference'",
        [], |row| row.get(0)
    ).unwrap_or_else(|_| "balanced".into());

    let prefer_local = offline_only || latency_pref == "low" || latency_pref == "lowest";

    // Try primary route first
    let primary: Result<String, _> = connection.query_row(
        "SELECT model FROM model_routes WHERE workspace_id = ?1 AND capability = ?2",
        params![workspace_id, capability], |row| row.get(0)
    );

    if let Ok(model) = primary {
        // In offline mode or low-latency preference, skip cloud models
        if (offline_only || prefer_local) && is_cloud_model(&model) {
            // fall through to local-only fallback
        } else {
            return model;
        }
    }

    // Try fallback chain (respecting offline-only mode and latency preference)
    if let Ok(chain) = fallback::get_fallback_chain(connection, workspace_id, capability) {
        for fb in &chain.providers {
            if fb.healthy && (!offline_only && !prefer_local || !is_cloud_provider(&fb.provider)) {
                return fb.model.clone();
            }
        }
    }

    // In offline mode or low-latency, force local fallback
    if offline_only || prefer_local {
        return match capability {
            "chat" | "summarization" => default_lmstudio_model(),
            "embeddings" => default_lmstudio_embedding_model(),
            "transcription" => "whisper-large-v3".to_string(),
            _ => fallback.to_string(),
        };
    }

    fallback.to_string()
}

fn is_cloud_model(model: &str) -> bool {
    model.contains("gpt-") || model.contains("claude-") || model.contains("gemini-")
        || model.contains("tts-") || model.contains("whisper-1")
}

fn is_cloud_provider(provider: &str) -> bool {
    matches!(provider.to_lowercase().as_str(), "openai" | "anthropic" | "google")
}

#[derive(serde::Serialize)]
struct WorkspaceRecord {
    id: String,
    name: String,
    source_count: i64,
}

#[derive(serde::Serialize)]
struct IngestionResult {
    discovered: usize,
    inserted: usize,
    directory: String,
}

#[derive(serde::Serialize)]
struct IndexResult {
    sources_indexed: usize,
    chunks_indexed: usize,
}

#[derive(serde::Serialize)]
struct SearchResult {
    source_id: String,
    title: String,
    uri: String,
    chunk_id: String,
    content: String,
}

#[derive(serde::Serialize)]
struct SourceRecord {
    id: String,
    workspace_id: String,
    kind: String,
    title: String,
    uri: Option<String>,
    status: String,
}

#[derive(serde::Serialize)]
struct MeetingRecord {
    id: String,
    title: String,
    recording_path: Option<String>,
    status: String,
}

#[derive(serde::Serialize)]
struct TranscriptionJob {
    id: String,
    meeting_id: String,
    provider: String,
    status: String,
    error: Option<String>,
}

#[derive(serde::Serialize)]
struct TranscriptSegmentRecord {
    id: String,
    speaker: Option<String>,
    start_seconds: f64,
    end_seconds: f64,
    text: String,
    confidence: Option<f64>,
    speaker_confidence: Option<f64>,
}

#[derive(serde::Serialize)]
struct MeetingSummary {
    meeting_id: String,
    model: String,
    summary: String,
    actions: Vec<MeetingAction>,
}

#[derive(serde::Serialize)]
struct MeetingAction {
    id: String,
    title: String,
    owner: Option<String>,
    due_at: Option<String>,
    status: String,
}

#[derive(serde::Serialize)]
struct TaskRecord {
    id: String,
    workspace_id: String,
    title: String,
    due_at: Option<String>,
    status: String,
}

#[derive(serde::Serialize)]
struct ActionRecord {
    id: String,
    meeting_id: String,
    title: String,
    status: String,
}

#[derive(serde::Serialize)]
pub(crate) struct ReminderRecord {
    id: String,
    workspace_id: String,
    task_id: Option<String>,
    calendar_event_id: Option<String>,
    title: String,
    scheduled_at: String,
    status: String,
}

#[derive(serde::Serialize)]
struct CalendarEventRecord {
    id: String,
    workspace_id: String,
    title: String,
    starts_at: String,
    ends_at: String,
    meeting_url: Option<String>,
    provider: String,
    recurrence: Option<String>,
}

#[derive(serde::Serialize)]
struct CalendarConnectionRecord {
    id: String,
    workspace_id: String,
    provider: String,
    account_label: String,
    status: String,
    last_synced_at: Option<String>,
}

#[derive(serde::Serialize)]
struct EmailAccountRecord {
    id: String,
    workspace_id: String,
    provider: String,
    account_label: String,
    imap_host: Option<String>,
    smtp_host: Option<String>,
    status: String,
}

#[derive(serde::Serialize)]
struct EmailSyncResult {
    account_id: String,
    fetched: usize,
    stored: usize,
}

#[derive(serde::Serialize)]
struct EmailRecord {
    id: String,
    account_id: String,
    subject: String,
    sender: Option<String>,
    recipients: Option<String>,
    body: Option<String>,
    received_at: Option<String>,
}

#[derive(serde::Deserialize)]
struct OllamaEmbeddingResponse { embedding: Vec<f32> }

#[derive(serde::Serialize)]
struct EmbeddingResult { chunks_embedded: usize, model: String }

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let (mut dot, mut left_norm, mut right_norm) = (0.0, 0.0, 0.0);
    for (a, b) in left.iter().zip(right.iter()) { dot += a * b; left_norm += a * a; right_norm += b * b; }
    if left_norm == 0.0 || right_norm == 0.0 { 0.0 } else { dot / (left_norm.sqrt() * right_norm.sqrt()) }
}

#[derive(serde::Deserialize)]
pub(crate) struct OllamaResponse { response: String }

#[derive(serde::Serialize)]
struct AssistantCitation {
    title: String,
    uri: Option<String>,
    chunk_id: String,
    meeting_id: Option<String>,
    start_seconds: Option<f64>,
}

#[derive(serde::Serialize)]
struct AssistantReply { conversation_id: String, response: String, model: String, citations: Vec<AssistantCitation> }

#[derive(serde::Serialize)]
struct AgentRecord {
    id: String,
    workspace_id: String,
    name: String,
    prompt: String,
    schedule: String,
    permission_mode: String,
    status: String,
    last_run_at: Option<String>,
}

#[derive(serde::Serialize)]
struct AgentRunRecord {
    id: String,
    agent_id: String,
    status: String,
    output: Option<String>,
    error: Option<String>,
    retry_count: i64,
}

#[derive(serde::Serialize)]
struct WorkspaceDeletionPreview {
    workspace_id: String,
    workspace_name: String,
    sources: i64,
    meetings: i64,
    transcript_segments: i64,
    tasks: i64,
    reminders: i64,
    notes: i64,
    emails: i64,
    agents: i64,
    recordings: Vec<String>,
}

struct Recorder {
    child: Child,
    /// Pipe used to send ffmpeg the "q" command for a graceful stop, so the
    /// container (moov atom in m4a) is finalized and the file stays decodable.
    stdin: Option<std::process::ChildStdin>,
    stderr: Option<std::process::ChildStderr>,
    path: std::path::PathBuf,
    workspace_id: String,
    title: String,
    /// Calendar event that auto-started this recording (auto-stop tracking).
    event_id: Option<String>,
    /// RFC3339 end of the calendar event; the scheduler stops the recording
    /// shortly after this time when auto-started.
    auto_stop_at: Option<String>,
}
/// Shared recorder process. A static (not tauri-managed) so the Tauri command
/// and the loopback REST API can start/stop the same capture.
static RECORDER: Mutex<Option<Recorder>> = Mutex::new(None);

/// Push-to-talk voice input: FFmpeg mic capture + local Whisper transcription.
struct VoiceCapture {
    child: Child,
    path: std::path::PathBuf,
    language: String,
}
struct VoiceCaptureState(Mutex<Option<VoiceCapture>>);

fn terminate_capture_child(mut child: Child) -> Result<(), String> {
    match child.kill() {
        Ok(()) => {}
        Err(error) => {
            // The process may already have exited; only report an error if it
            // is still running and therefore could still hold the microphone.
            if child.try_wait().map_err(|probe| probe.to_string())?.is_none() {
                return Err(error.to_string());
            }
        }
    }
    child.wait().map(|_| ()).map_err(|error| error.to_string())
}

/// Stop child capture processes when the application exits. Child processes do
/// not automatically terminate when the Tauri process is closed, so leaving
/// them alive would keep the microphone open after the UI disappears.
fn cleanup_capture_processes(app: &tauri::AppHandle) {
    let mut stopped_recording: Option<(String, String)> = None;
    if let Ok(mut active) = RECORDER.lock() {
        if let Some(recorder) = active.take() {
            let path = recorder.path.to_string_lossy().into_owned();
            let workspace_id = recorder.workspace_id.clone();
            if let Err(error) = terminate_capture_child(recorder.child) {
                log::warn!("application_exit: failed to stop recording capture (workspace={workspace_id}, path={path}): {error}");
            }
            stopped_recording = Some((workspace_id, path));
        }
    } else {
        log::error!("application_exit: recorder state lock unavailable; capture may remain active");
    }

    if let Some(state) = app.try_state::<VoiceCaptureState>() {
        if let Ok(mut active) = state.0.lock() {
            if let Some(capture) = active.take() {
                let path = capture.path.to_string_lossy().into_owned();
                if let Err(error) = terminate_capture_child(capture.child) {
                    log::warn!("application_exit: failed to stop voice capture (path={path}): {error}");
                }
                log::info!("application_exit: stopped voice capture (path={path})");
            }
        } else {
            log::error!("application_exit: voice capture state lock unavailable; capture may remain active");
        }
    }

    if let Some((workspace_id, path)) = stopped_recording {
        log::warn!("application_exit: stopped active recording (workspace={workspace_id}, path={path})");
        if let Ok(connection) = open_database(app) {
            let _ = retention::record_audit(
                &connection,
                "recording_stopped_on_exit",
                None,
                Some("audio"),
                Some(&workspace_id),
                &format!("Capture process stopped during application exit: {path}"),
            );
        }
    }
}

#[derive(serde::Serialize)]
struct RecordingStatus {
    active: bool,
    path: Option<String>,
    meeting_id: Option<String>,
    queued: bool,
}

/// Workspace export/restore format (#8).
///
/// v2 adds:
/// - `schema_version` + `content_hash` (SHA-256 over the canonical payload)
///   so tampered/truncated backups are rejected on import and by
///   `verify_backup`;
/// - `row_counts` per domain for restore verification;
/// - full domain coverage (notes, calendar, memories, source chunks, model
///   routes, permissions, web pages, email accounts, jobs, bot runs, speaker
///   embeddings, agent runs).
/// New fields default so legacy v1 exports still import.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct WorkspaceExport {
    version: u32,
    #[serde(default)]
    schema_version: u32,
    exported_at: String,
    workspace_id: String,
    #[serde(default)]
    content_hash: String,
    #[serde(default)]
    row_counts: std::collections::HashMap<String, usize>,
    workspaces: Vec<serde_json::Value>,
    sources: Vec<serde_json::Value>,
    #[serde(default)]
    source_chunks: Vec<serde_json::Value>,
    meetings: Vec<serde_json::Value>,
    transcript_segments: Vec<serde_json::Value>,
    #[serde(default)]
    meeting_summaries: Vec<serde_json::Value>,
    #[serde(default)]
    meeting_actions: Vec<serde_json::Value>,
    #[serde(default)]
    transcription_jobs: Vec<serde_json::Value>,
    tasks: Vec<serde_json::Value>,
    reminders: Vec<serde_json::Value>,
    emails: Vec<serde_json::Value>,
    #[serde(default)]
    email_accounts: Vec<serde_json::Value>,
    embeddings: Vec<serde_json::Value>,
    voice_profiles: Vec<serde_json::Value>,
    #[serde(default)]
    speaker_embeddings: Vec<serde_json::Value>,
    agents: Vec<serde_json::Value>,
    #[serde(default)]
    agent_runs: Vec<serde_json::Value>,
    #[serde(default)]
    notes: Vec<serde_json::Value>,
    #[serde(default)]
    calendar_events: Vec<serde_json::Value>,
    #[serde(default)]
    calendar_participants: Vec<serde_json::Value>,
    #[serde(default)]
    model_routes: Vec<serde_json::Value>,
    #[serde(default)]
    memories: Vec<serde_json::Value>,
    #[serde(default)]
    web_pages: Vec<serde_json::Value>,
    #[serde(default)]
    bot_runs: Vec<serde_json::Value>,
    #[serde(default)]
    agent_permissions: Vec<serde_json::Value>,
    #[serde(default)]
    workspace_allowlists: Vec<serde_json::Value>,
}

#[derive(serde::Serialize)]
struct ModelRouteRecord {
    workspace_id: String,
    capability: String,
    provider: String,
    model: String,
}

#[derive(serde::Serialize)]
struct NoteRecord {
    id: String,
    workspace_id: String,
    title: String,
    content: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Serialize)]
struct WebPageRecord {
    id: String,
    workspace_id: String,
    url: String,
    title: String,
    fetched_at: String,
}

#[derive(serde::Serialize)]
struct VoiceProfileRecord {
    id: String,
    workspace_id: String,
    speaker_label: String,
    sample_count: i64,
}

#[derive(serde::Serialize)]
struct EmailSendResult {
    sent: bool,
    message: String,
}

#[derive(serde::Deserialize)]
struct WhisperSegment {
    start: f64,
    end: f64,
    text: String,
    /// openai-whisper per-segment mean log-probability (absent in whisper.cpp
    /// JSON builds); converted to a 0..1 confidence = exp(avg_logprob).
    #[serde(default)]
    avg_logprob: Option<f64>,
    /// Probability the segment is actually silence/no speech (0..1).
    #[serde(default)]
    no_speech_prob: Option<f64>,
}

#[derive(serde::Deserialize)]
struct WhisperOutput {
    segments: Vec<WhisperSegment>,
    /// Language whisper detected for the audio (top-level JSON field).
    #[serde(default)]
    language: Option<String>,
}

#[tauri::command]
fn local_status(app: tauri::AppHandle) -> Result<String, String> {
    let path = database_path(&app)?;
    let _ = open_database(&app)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Return the last `limit` lines of the application log (Diagnostics view).
#[tauri::command]
fn read_app_log(limit: Option<usize>) -> Result<String, String> {
    Ok(logging::read_tail(limit.unwrap_or(300)))
}

/// Absolute path of the application log file (Diagnostics view).
#[tauri::command]
fn app_log_path() -> Result<String, String> {
    Ok(logging::log_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "Logging not initialized".into()))
}

/// Forward a frontend-originated message into the application log. The web
/// view cannot write files, so errors caught in JS are shipped here.
#[tauri::command]
fn log_event(level: String, source: String, message: String) -> Result<(), String> {
    match level.to_lowercase().as_str() {
        "debug" => log::debug!("[ui:{source}] {message}"),
        "warn" => log::warn!("[ui:{source}] {message}"),
        "error" => log::error!("[ui:{source}] {message}"),
        _ => log::info!("[ui:{source}] {message}"),
    }
    Ok(())
}

#[tauri::command]
fn list_workspaces(app: tauri::AppHandle) -> Result<Vec<WorkspaceRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, name, source_count FROM workspaces ORDER BY created_at").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok(WorkspaceRecord { id: row.get(0)?, name: row.get(1)?, source_count: row.get(2)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn create_workspace(app: tauri::AppHandle, name: String) -> Result<WorkspaceRecord, String> {
    if name.trim().is_empty() { return Err("Workspace name is required".into()); }
    let id = format!("workspace-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO workspaces (id, name, source_count) VALUES (?1, ?2, 0)", params![id, name.trim()]).map_err(|error| error.to_string())?;
    Ok(WorkspaceRecord { id, name: name.trim().into(), source_count: 0 })
}

#[tauri::command]
fn save_setting(app: tauri::AppHandle, key: String, value: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_setting(app: tauri::AppHandle, key: String) -> Result<Option<String>, String> {
    let connection = open_database(&app)?;
    connection.query_row("SELECT value FROM app_settings WHERE key = ?1", params![key], |row| row.get(0)).map(Some).or_else(|error| match error { rusqlite::Error::QueryReturnedNoRows => Ok(None), other => Err(other.to_string()) })
}

#[tauri::command]
fn set_data_directory(app: tauri::AppHandle, directory: String) -> Result<String, String> {
    let target = std::path::PathBuf::from(directory.trim());
    if !target.is_absolute() { return Err("Data directory must be an absolute path".into()); }
    std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let current = database_path(&app)?;
    let destination = target.join("database/neural-agent-os.sqlite");
    if current != destination && current.is_file() && !destination.exists() {
        std::fs::create_dir_all(target.join("database")).map_err(|error| error.to_string())?;
        std::fs::copy(&current, &destination).map_err(|error| error.to_string())?;
        for suffix in ["-wal", "-shm"] {
            let sidecar = std::path::PathBuf::from(format!("{}{}", current.to_string_lossy(), suffix));
            if sidecar.is_file() { let _ = std::fs::copy(&sidecar, target.join("database").join(format!("neural-agent-os.sqlite{}", suffix))); }
        }
    }
    let default_directory = app.path().app_data_dir().map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&default_directory).map_err(|error| error.to_string())?;
    std::fs::write(default_directory.join("data-directory.txt"), target.to_string_lossy().as_bytes()).map_err(|error| error.to_string())?;
    set_data_root(target.clone());
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
fn delete_workspace(app: tauri::AppHandle, workspace_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if count <= 1 { return Err("The last workspace cannot be deleted".into()); }
    connection.execute("DELETE FROM workspaces WHERE id = ?1", params![workspace_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn preview_workspace_deletion(app: tauri::AppHandle, workspace_id: String) -> Result<WorkspaceDeletionPreview, String> {
    let connection = open_database(&app)?;
    let workspace_name: String = connection.query_row("SELECT name FROM workspaces WHERE id = ?1", params![workspace_id.clone()], |row| row.get(0)).map_err(|error| error.to_string())?;
    let count = |table: &str, column: &str| -> Result<i64, String> {
        connection.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"), params![workspace_id.clone()], |row| row.get(0)).map_err(|error| error.to_string())
    };
    let mut recording_statement = connection.prepare("SELECT recording_path FROM meetings WHERE workspace_id = ?1 AND recording_path IS NOT NULL").map_err(|error| error.to_string())?;
    let recordings = recording_statement.query_map(params![workspace_id.clone()], |row| row.get::<_, String>(0)).map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    let transcript_segments: i64 = connection.query_row("SELECT COUNT(*) FROM transcript_segments WHERE meeting_id IN (SELECT id FROM meetings WHERE workspace_id = ?1)", params![workspace_id.clone()], |row| row.get(0)).map_err(|error| error.to_string())?;
    let emails: i64 = connection.query_row("SELECT COUNT(*) FROM emails WHERE account_id IN (SELECT id FROM email_accounts WHERE workspace_id = ?1)", params![workspace_id.clone()], |row| row.get(0)).map_err(|error| error.to_string())?;
    Ok(WorkspaceDeletionPreview { workspace_id: workspace_id.clone(), workspace_name, sources: count("sources", "workspace_id")?, meetings: count("meetings", "workspace_id")?, transcript_segments, tasks: count("tasks", "workspace_id")?, reminders: count("reminders", "workspace_id")?, notes: count("notes", "workspace_id")?, emails, agents: count("agents", "workspace_id")?, recordings })
}

#[tauri::command]
fn ingest_directory(app: tauri::AppHandle, workspace_id: String, directory: String) -> Result<IngestionResult, String> {
    let root = std::path::PathBuf::from(&directory);
    if !root.is_dir() {
        return Err(format!("Directory does not exist: {directory}"));
    }

    let connection = open_database(&app)?;
    let mut discovered = 0;
    let mut inserted = 0;
    let supported = ["pdf", "md", "markdown", "txt", "docx", "html", "htm", "json", "csv"];

    for entry in WalkDir::new(&root).follow_links(false).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_file() || !path.extension().and_then(|extension| extension.to_str()).map(|extension| supported.contains(&extension.to_lowercase().as_str())).unwrap_or(false) {
            continue;
        }
        discovered += 1;
        let uri = path.to_string_lossy().into_owned();
        let id = format!("source-{}", uuid::Uuid::new_v4());
        let title = path.file_name().and_then(|name| name.to_str()).unwrap_or("Untitled source");
        let changed = connection.execute(
            "INSERT OR IGNORE INTO sources (id, workspace_id, kind, title, uri, status) VALUES (?1, ?2, 'file', ?3, ?4, 'pending')",
            params![id, workspace_id, title, uri],
        ).map_err(|error| error.to_string())?;
        inserted += changed;
    }

    connection.execute(
        "UPDATE workspaces SET source_count = (SELECT COUNT(*) FROM sources WHERE workspace_id = ?1) WHERE id = ?1",
        params![workspace_id],
    ).map_err(|error| error.to_string())?;
    Ok(IngestionResult { discovered, inserted, directory })
}

fn supported_text(extension: &str) -> bool {
    ["md", "markdown", "txt", "html", "htm", "json", "csv", "pdf", "docx"].contains(&extension)
}

fn extract_docx(path: &str) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut document = archive.by_name("word/document.xml").map_err(|error| error.to_string())?;
    let mut xml = String::new();
    document.read_to_string(&mut xml).map_err(|error| error.to_string())?;
    let mut reader = Reader::from_str(&xml);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(event)) => text.push_str(&event.unescape().map_err(|error| error.to_string())?),
            Ok(Event::End(event)) if event.name().as_ref() == b"w:p" => text.push('\n'),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(text)
}

fn extract_source(path: &str, extension: &str) -> Result<String, String> {
    match extension {
        "pdf" => pdf_extract::extract_text(path).map_err(|error| error.to_string()),
        "docx" => extract_docx(path),
        _ => std::fs::read_to_string(path).map_err(|error| error.to_string()),
    }
}

#[tauri::command]
fn index_sources(app: tauri::AppHandle, workspace_id: String) -> Result<IndexResult, String> {
    let connection = open_database(&app)?;
    // Incremental indexing: only sources that are new, never hashed, or whose
    // content hash changed since the last index (status is not a reliable
    // change signal on its own).
    let mut sources = connection.prepare(
        // Read every URI-backed source so changed files can be detected by
        // content hash. The unchanged fast path below avoids rebuilding its
        // chunks/FTS rows; filtering only by status would miss edits made
        // after a source was first indexed.
        "SELECT id, uri, status, content_hash FROM sources WHERE workspace_id = ?1 AND uri IS NOT NULL",
    ).map_err(|error| error.to_string())?;
    let rows = sources.query_map(params![workspace_id], |row| Ok((
        row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?,
        row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?,
    ))).map_err(|error| error.to_string())?;
    let mut sources_indexed = 0;
    let mut chunks_indexed = 0;

    for row in rows {
        let (source_id, uri, status, previous_hash) = row.map_err(|error| error.to_string())?;
        let Some(uri) = uri else { continue };
        let extension = std::path::Path::new(&uri).extension().and_then(|value| value.to_str()).unwrap_or("").to_lowercase();
        if !supported_text(&extension) { continue; }
        let content = extract_source(&uri, &extension).map_err(|error| format!("Could not extract {uri}: {error}"))?;
        let content_hash = content_sha256(&content);
        // Skip unchanged, already-indexed sources (incremental fast path).
        if status == "indexed" && previous_hash.as_deref() == Some(content_hash.as_str()) {
            log::debug!("index_sources: unchanged source {source_id}; skipping derived index");
            continue;
        }
        log::info!("index_sources: indexing changed/new source {source_id} (previous_hash_present={})", previous_hash.is_some());
        let chunks: Vec<&str> = content.as_bytes().chunks(1800).filter_map(|bytes| std::str::from_utf8(bytes).ok()).map(str::trim).filter(|chunk| !chunk.is_empty()).collect();
        connection.execute("DELETE FROM source_search WHERE source_id = ?1", params![source_id]).map_err(|error| error.to_string())?;
        connection.execute("DELETE FROM source_chunks WHERE source_id = ?1", params![source_id]).map_err(|error| error.to_string())?;
        for (index, chunk) in chunks.iter().enumerate() {
            let chunk_id = format!("chunk-{}", uuid::Uuid::new_v4());
            connection.execute("INSERT INTO source_chunks (id, source_id, chunk_index, content) VALUES (?1, ?2, ?3, ?4)", params![chunk_id, source_id, index, chunk]).map_err(|error| error.to_string())?;
            connection.execute("INSERT INTO source_search (content, source_id, chunk_id) VALUES (?1, ?2, ?3)", params![chunk, source_id, chunk_id]).map_err(|error| error.to_string())?;
            chunks_indexed += 1;
        }
        connection.execute(
            "UPDATE sources SET status = 'indexed', content_hash = ?1, indexed_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![content_hash, source_id],
        ).map_err(|error| error.to_string())?;
        sources_indexed += 1;
    }
    Ok(IndexResult { sources_indexed, chunks_indexed })
}

/// SHA-256 hex digest of source content, used for duplicate detection and
/// incremental re-indexing.
fn content_sha256(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Build a safe FTS5 MATCH expression from free-text input. Every
/// whitespace-separated term is emitted as a quoted literal string (FTS5
/// escapes embedded quotes by doubling them), so user queries containing FTS5
/// syntax characters (`,`, `(`, `)`, `"`, `-`, `*`, `^`, `:`, `NEAR`…) are
/// matched literally and can never raise `fts5: syntax error near …`.
pub(crate) fn fts5_match_terms(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[tauri::command]
fn search_sources(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<SearchResult>, String> {
    let query = query.trim();
    if query.is_empty() { return Ok(Vec::new()); }
    let connection = open_database(&app)?;
    let match_query = fts5_match_terms(&query);
    let mut statement = connection.prepare(
        "SELECT search.source_id, sources.title, sources.uri, search.chunk_id, search.content
         FROM source_search search JOIN sources ON sources.id = search.source_id
         WHERE sources.workspace_id = ?1 AND source_search MATCH ?2
         ORDER BY rank LIMIT 20"
    ).map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id.clone(), match_query], |row| Ok(SearchResult {
        source_id: row.get(0)?, title: row.get(1)?, uri: row.get::<_, Option<String>>(2)?.unwrap_or_default(), chunk_id: row.get(3)?, content: row.get(4)?
    })).map_err(|error| error.to_string())?;
    let mut results = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    let mut email_statement = connection.prepare("SELECT email_id, subject, sender, body FROM email_search WHERE workspace_id = ?1 AND email_search MATCH ?2 ORDER BY rank LIMIT 20").map_err(|error| error.to_string())?;
    let email_rows = email_statement.query_map(params![workspace_id, match_query], |row| Ok(SearchResult {
        source_id: row.get(0)?, title: format!("Email: {}", row.get::<_, String>(1)?), uri: row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "email".into()), chunk_id: row.get(0)?, content: row.get::<_, Option<String>>(3)?.unwrap_or_default()
    })).map_err(|error| error.to_string())?;
    results.extend(email_rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?);
    Ok(results)
}

#[tauri::command]
fn list_sources(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<SourceRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, kind, title, uri, status FROM sources WHERE workspace_id = ?1 ORDER BY created_at DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(SourceRecord { id: row.get(0)?, workspace_id: row.get(1)?, kind: row.get(2)?, title: row.get(3)?, uri: row.get(4)?, status: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_source(app: tauri::AppHandle, source_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    // Cascading delete: source → chunks → search index → embeddings
    // First, delete embeddings for all chunks of this source
    connection.execute(
        "DELETE FROM chunk_embeddings WHERE chunk_id IN (SELECT id FROM source_chunks WHERE source_id = ?1)",
        params![source_id]
    ).map_err(|error| error.to_string())?;
    // Delete FTS5 search entries
    connection.execute("DELETE FROM source_search WHERE source_id = ?1", params![source_id]).map_err(|error| error.to_string())?;
    // Delete chunks (CASCADE will clean up chunk_embeddings references not caught above)
    connection.execute("DELETE FROM source_chunks WHERE source_id = ?1", params![source_id]).map_err(|error| error.to_string())?;
    // Delete the source record itself
    let changed = connection.execute("DELETE FROM sources WHERE id = ?1", params![source_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Source was not found".into()); }
    Ok(())
}

// ── Selective derived-data deletion ─────────────────────────────────────────

#[tauri::command]
fn delete_source_embeddings(app: tauri::AppHandle, source_id: String) -> Result<usize, String> {
    let connection = open_database(&app)?;
    let deleted = connection.execute(
        "DELETE FROM chunk_embeddings WHERE chunk_id IN (SELECT id FROM source_chunks WHERE source_id = ?1)",
        params![source_id],
    ).map_err(|e| e.to_string())?;
    Ok(deleted)
}

#[tauri::command]
fn delete_transcript_segments(app: tauri::AppHandle, meeting_id: String) -> Result<usize, String> {
    let connection = open_database(&app)?;
    let deleted = connection.execute(
        "DELETE FROM transcript_segments WHERE meeting_id = ?1",
        params![meeting_id],
    ).map_err(|e| e.to_string())?;
    // Reset meeting status so it can be re-transcribed
    connection.execute(
        "UPDATE meetings SET status = 'ready_for_transcription' WHERE id = ?1",
        params![meeting_id],
    ).map_err(|e| e.to_string())?;
    Ok(deleted)
}

#[tauri::command]
fn delete_meeting_summary(app: tauri::AppHandle, meeting_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    connection.execute(
        "DELETE FROM meeting_summaries WHERE meeting_id = ?1",
        params![meeting_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_meeting_actions(app: tauri::AppHandle, meeting_id: String) -> Result<usize, String> {
    let connection = open_database(&app)?;
    let deleted = connection.execute(
        "DELETE FROM meeting_actions WHERE meeting_id = ?1",
        params![meeting_id],
    ).map_err(|e| e.to_string())?;
    Ok(deleted)
}

#[tauri::command]
fn delete_meeting_recording(app: tauri::AppHandle, meeting_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let path: Option<String> = connection.query_row(
        "SELECT recording_path FROM meetings WHERE id = ?1",
        params![meeting_id], |row| row.get(0),
    ).ok().flatten();
    if let Some(ref p) = path {
        let _ = std::fs::remove_file(p);
        // Also remove the notice marker if present
        let notice = std::path::Path::new(p).with_extension("notice.txt");
        let _ = std::fs::remove_file(notice);
    }
    connection.execute(
        "UPDATE meetings SET recording_path = NULL WHERE id = ?1",
        params![meeting_id],
    ).map_err(|e| e.to_string())?;
    retention::record_audit(&connection, "recording_deleted", None, Some("audio"), None, &format!("Deleted recording file for meeting {meeting_id}"))?;
    Ok(())
}

#[tauri::command]
fn import_meeting_recording(app: tauri::AppHandle, workspace_id: String, path: String, title: String) -> Result<MeetingRecord, String> {
    // Resolve relative/~ input paths against the user's home so imports work
    // regardless of the packaged app's working directory (/).
    let resolved_input = if let Some(rest) = path.trim().strip_prefix("~/") {
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest)
    } else {
        let p = std::path::PathBuf::from(path.trim());
        if p.is_absolute() { p } else { std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(p) }
    };
    let recording = resolved_input;
    if !recording.is_file() { return Err(format!("Recording does not exist: {}", recording.display())); }
    let extension = recording.extension().and_then(|value| value.to_str()).unwrap_or("").to_lowercase();
    if !["mp3", "wav", "m4a", "mp4", "mov", "webm", "ogg", "flac"].contains(&extension.as_str()) { return Err("Unsupported audio or video format".into()); }
    // Timestamp-named recordings: when no title is supplied, use the file name
    // (e.g. `meeting-2026-08-11_12-34-56.m4a`) so the meeting, its transcript,
    // and its summary all carry the recorded-at timestamp.
    let title = if title.trim().is_empty() {
        recording.file_stem().and_then(|s| s.to_str()).unwrap_or("Meeting").to_string()
    } else {
        title.trim().to_string()
    };
    let id = format!("meeting-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO meetings (id, workspace_id, title, recording_path, status) VALUES (?1, ?2, ?3, ?4, 'ready_for_transcription')", params![id, workspace_id, title, path]).map_err(|error| error.to_string())?;
    Ok(MeetingRecord { id, title, recording_path: Some(path), status: "ready_for_transcription".into() })
}

#[tauri::command]
fn list_meetings(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<MeetingRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, title, recording_path, status FROM meetings WHERE workspace_id = ?1 ORDER BY rowid DESC LIMIT 50").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(MeetingRecord { id: row.get(0)?, title: row.get(1)?, recording_path: row.get(2)?, status: row.get(3)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn queue_transcription(app: tauri::AppHandle, meeting_id: String, provider: String, language: Option<String>) -> Result<TranscriptionJob, String> {
    log::info!("queue_transcription: meeting={meeting_id} provider={provider} language={language:?}");
    let connection = open_database(&app)?;
    let workspace_id: String = connection.query_row("SELECT workspace_id FROM meetings WHERE id = ?1", params![meeting_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    let provider = routed_model(&connection, &workspace_id, "transcription", &provider);
    let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM meetings WHERE id = ?1)", params![meeting_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    if !exists { return Err("Meeting was not found".into()); }
    let id = format!("transcription-{}", uuid::Uuid::new_v4());
    connection.execute("INSERT INTO transcription_jobs (id, meeting_id, provider, status, language) VALUES (?1, ?2, ?3, 'queued', ?4)", params![id, meeting_id, provider, language]).map_err(|error| error.to_string())?;
    if let Some(lang) = language.as_deref().filter(|l| !l.trim().is_empty()) {
        let _ = connection.execute("UPDATE meetings SET language = ?1 WHERE id = ?2", params![lang, meeting_id]);
    }
    connection.execute("UPDATE meetings SET status = 'transcription_queued' WHERE id = ?1", params![meeting_id]).map_err(|error| error.to_string())?;
    Ok(TranscriptionJob { id, meeting_id, provider, status: "queued".into(), error: None })
}

#[tauri::command]
fn list_transcription_jobs(app: tauri::AppHandle, meeting_id: String) -> Result<Vec<TranscriptionJob>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, meeting_id, provider, status, error FROM transcription_jobs WHERE meeting_id = ?1 ORDER BY created_at DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![meeting_id], |row| Ok(TranscriptionJob { id: row.get(0)?, meeting_id: row.get(1)?, provider: row.get(2)?, status: row.get(3)?, error: row.get(4)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

/// Run Whisper on a meeting's recording and store transcript segments, then
/// auto-diarize. Returns the number of segments. Shared by the manual
/// transcription command and the scheduled queue (sequential, one at a time).
/// Map an openai-whisper segment to a 0..1 confidence value.
/// Base = exp(mean token log-probability); when the backend also reports a
/// no-speech probability, segments that are mostly silence are down-weighted
/// (they are often hallucinated). None when the JSON backend reports neither.
fn whisper_confidence(segment: &WhisperSegment) -> Option<f32> {
    let base = segment.avg_logprob.map(|lp| (lp.exp().clamp(0.0, 1.0)) as f32);
    let no_speech = segment.no_speech_prob.map(|p| (1.0 - p).clamp(0.0, 1.0) as f32);
    match (base, no_speech) {
        (Some(conf), Some(ns)) => Some(conf * ns),
        (Some(conf), None) => Some(conf),
        (None, Some(ns)) => Some(ns),
        (None, None) => None,
    }
}

pub(crate) fn transcribe_meeting(connection: &Connection, meeting_id: &str, provider: &str, model: &str, language: &str) -> Result<usize, String> {
    log::info!("transcribe_meeting: meeting={meeting_id} provider={provider} model={model}");
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM meetings WHERE id = ?1)",
        params![meeting_id],
        |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if !exists {
        log::error!("transcribe_meeting: meeting {meeting_id} does not exist");
        return Err(format!("Meeting not found: {meeting_id}"));
    }
    let recording_path: String = connection.query_row(
        "SELECT recording_path FROM meetings WHERE id = ?1",
        params![meeting_id],
        |row| row.get::<_, Option<String>>(0).map(|value| value.unwrap_or_default()),
    ).map_err(|error| error.to_string())?;
    if recording_path.is_empty() {
        log::error!("transcribe_meeting: meeting {meeting_id} has no recording path");
        return Err("Meeting has no recording file to transcribe".into());
    }
    let recording = std::path::Path::new(&recording_path);
    if !recording.is_file() {
        log::error!("transcribe_meeting: recording file missing on disk: path={recording_path} meeting_id={meeting_id}");
        return Err(format!("Permanent: Recording file does not exist: {recording_path}"));
    }
    let recording_bytes = std::fs::metadata(recording).map(|m| m.len()).unwrap_or(0);
    if recording_bytes == 0 {
        log::error!("transcribe_meeting: empty recording path={recording_path} meeting_id={meeting_id} bytes=0");
        return Err(format!("Permanent: Recording file is empty: {recording_path}"));
    }
    log::info!("transcribe_meeting: input validated meeting_id={meeting_id} path={recording_path} bytes={recording_bytes}");
    // Any whisper-compatible CLI works: openai-whisper, faster-whisper,
    // whisper.cpp, or a local wrapper — resolved via NEURAL_WHISPER_BIN,
    // well-known install locations, or PATH.
    let whisper = match resolve_binary("whisper", "NEURAL_WHISPER_BIN") {
        Ok(bin) => bin,
        Err(error) => { log::error!("transcribe_meeting: whisper binary unavailable: {error}"); return Err(error); }
    };
    // The workspace's transcription route model wins; fall back to the
    // NEURAL_WHISPER_MODEL env var, then the default small model.
    let whisper_model = if model.trim().is_empty() {
        std::env::var("NEURAL_WHISPER_MODEL").unwrap_or_else(|_| "small".into())
    } else {
        model.trim().to_string()
    };
    // Explicit language selection (en/hi/te) wins; empty means auto-detect
    // (whisper detects the language and reports it in the JSON output).
    let whisper_language: Option<String> = match language.trim() {
        "" => None,
        lang => Some(lang.to_string()),
    };
    log::info!("transcribe_meeting: whisper={whisper} model={whisper_model} language={whisper_language:?} audio={recording_path}");
    let mut whisper_cmd = std::process::Command::new(&whisper);
    whisper_cmd.arg(recording_path.as_str())
        .args(["--output_format", "json", "--output_dir", &data_subdir("transcripts").to_string_lossy()]);
    // Only pass --model to CLIs that understand it (openai-whisper); mlx-whisper
    // uses its own model naming (mlx-community/...) and errors on routed names
    // like "whisper-large-v3", so let it fall back to its fast default.
    if whisper_caps(&whisper).0 {
        whisper_cmd.args(["--model", &whisper_model]);
    }
    if let Some(lang) = &whisper_language {
        whisper_cmd.args(["--language", lang]);
    }
    let output = whisper_cmd.output();
    let result = match output {
        Ok(output) if output.status.success() => {
            let json_path = data_subdir("transcripts").join(format!("{}.json", std::path::Path::new(&recording_path).file_stem().and_then(|name| name.to_str()).unwrap_or("audio")));
            log::info!("transcribe_meeting: whisper exited 0, expecting JSON at {}", json_path.display());
            // Some whisper builds (e.g. mlx_whisper with an undecodable file)
            // exit 0 while writing nothing — surface the captured output so
            // the real cause (e.g. "moov atom not found") is visible.
            if !json_path.is_file() {
                let stdout_tail = String::from_utf8_lossy(&output.stdout);
                let stderr_tail = String::from_utf8_lossy(&output.stderr);
                let detail = if !stderr_tail.trim().is_empty() { stderr_tail } else { stdout_tail };
                let detail = detail.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
                log::error!("transcribe_meeting: whisper exited 0 but wrote no JSON for {recording_path}; output: {detail}");
                return Err(format!(
                    "Whisper exited successfully but produced no transcript for {recording_path}.                      This usually means the audio file is empty or corrupt. Decoder output: {detail}"
                ));
            }
            let content = match std::fs::read_to_string(&json_path) {
                Ok(content) => content,
                Err(error) => {
                    log::error!("transcribe_meeting: could not read {}: {error}", json_path.display());
                    return Err(format!("Could not read whisper output {}: {error}", json_path.display()));
                }
            };
            match serde_json::from_str::<WhisperOutput>(&content) {
                Ok(transcript) => {
                    connection.execute("DELETE FROM transcript_segments WHERE meeting_id = ?1", params![meeting_id]).map_err(|error| error.to_string())?;
                    let count = transcript.segments.len();
                    for (index, segment) in transcript.segments.iter().enumerate() {
                        connection.execute("INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![format!("segment-{}-{}", meeting_id, index), meeting_id, segment.start, segment.end, segment.text.trim(), whisper_confidence(segment)]).map_err(|error| error.to_string())?;
                    }
                    // Record the language: explicit selection wins; otherwise
                    // keep what whisper detected from the audio.
                    let effective_language = if let Some(lang) = &whisper_language {
                        Some(lang.as_str())
                    } else {
                        transcript.language.as_deref()
                    };
                    if let Some(lang) = effective_language {
                        let _ = connection.execute("UPDATE meetings SET language = ?1 WHERE id = ?2", params![lang, meeting_id]);
                    }
                    connection.execute("UPDATE meetings SET status = 'transcribed' WHERE id = ?1", params![meeting_id]).map_err(|error| error.to_string())?;
                    log::info!("transcribe_meeting: stored {count} segments for meeting {meeting_id} (language={effective_language:?})");
                    // Auto-diarize after transcription completes: prefer the
                    // audio-based backend (scripts/diarize.py / NEURAL_DIARIZE_BIN);
                    // fall back to transcript-text embedding matching.
                    let ws_id: String = connection.query_row("SELECT workspace_id FROM meetings WHERE id = ?1", params![meeting_id], |row| row.get(0)).unwrap_or_default();
                    let audio_result = diarization::diarize_audio(connection, meeting_id, &ws_id, &recording_path);
                    match audio_result {
                        Ok(Some(assigned)) => {
                            log::info!("transcribe_meeting: audio diarization assigned {assigned} segments for {meeting_id}");
                            let _ = retention::record_audit(connection, "diarization_audio", Some("local"), Some("meeting"), Some(&ws_id), &format!("Audio-based diarization assigned {assigned} segments for meeting {meeting_id}"));
                        }
                        Ok(None) => {
                            log::info!("transcribe_meeting: audio diarization unavailable; using text-embedding fallback for {meeting_id}");
                            let _ = retention::record_audit(connection, "diarization_text_fallback", Some("local"), Some("meeting"), Some(&ws_id), &format!("Audio diarizer not installed; text-embedding fallback used for meeting {meeting_id}"));
                            match diarization::diarize_transcript(connection, meeting_id, &ws_id, "nomic-embed-text", 0.7) {
                                Ok(_) => log::info!("transcribe_meeting: text-embedding diarization complete for {meeting_id}"),
                                Err(error) => log::warn!("transcribe_meeting: text-embedding diarization failed for {meeting_id}: {error}"),
                            }
                        }
                        Err(error) => {
                            log::warn!("transcribe_meeting: audio diarization failed for {meeting_id}: {error}; using text-embedding fallback");
                            let _ = retention::record_audit(connection, "diarization_audio_failed", Some("local"), Some("meeting"), Some(&ws_id), &format!("Audio diarization error for meeting {meeting_id}: {error}"));
                            match diarization::diarize_transcript(connection, meeting_id, &ws_id, "nomic-embed-text", 0.7) {
                                Ok(_) => log::info!("transcribe_meeting: text-embedding diarization complete for {meeting_id}"),
                                Err(fallback_error) => log::warn!("transcribe_meeting: text-embedding diarization failed for {meeting_id}: {fallback_error}"),
                            }
                        }
                    }
                    Ok(count)
                }
                Err(error) => {
                    let snippet: String = content.chars().take(300).collect();
                    log::error!("transcribe_meeting: whisper JSON parse failed at {}: {error}; content: {snippet}", json_path.display());
                    Err(format!("Whisper JSON output could not be parsed at {}: {error}", json_path.display()))
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            log::error!("transcribe_meeting: whisper failed (exit {:?}) for {recording_path}: {}", output.status.code(), stderr);
            Err(if stderr.is_empty() { format!("Whisper failed with exit code {:?}", output.status.code()) } else { format!("Whisper failed: {stderr}") })
        }
        Err(error) => {
            log::error!("transcribe_meeting: failed to launch whisper ({whisper}): {error}");
            Err(format!("Transcription runtime unavailable: {error}. Install Whisper or set NEURAL_WHISPER_BIN."))
        }
    };
    if result.is_err() {
        let _ = connection.execute("UPDATE meetings SET status = 'transcription_failed' WHERE id = ?1", params![meeting_id]);
    }
    result
}

#[tauri::command]
fn process_transcription_job(app: tauri::AppHandle, job_id: String) -> Result<TranscriptionJob, String> {
    log::info!("process_transcription_job: job={job_id}");
    let connection = open_database(&app)?;
    let (meeting_id, provider, language): (String, String, Option<String>) = connection.query_row(
        "SELECT meeting_id, provider, language FROM transcription_jobs WHERE id = ?1",
        params![job_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|error| error.to_string())?;
    let workspace_id: String = connection.query_row(
        "SELECT workspace_id FROM meetings WHERE id = ?1",
        params![meeting_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let model = routed_model(&connection, &workspace_id, "transcription", "");
    connection.execute("UPDATE transcription_jobs SET status = 'processing', updated_at = CURRENT_TIMESTAMP WHERE id = ?1", params![job_id]).map_err(|error| error.to_string())?;
    let result = transcribe_meeting(&connection, &meeting_id, &provider, &model, language.as_deref().unwrap_or(""));
    match &result {
        Ok(count) => {
            connection.execute("UPDATE transcription_jobs SET status = 'completed', updated_at = CURRENT_TIMESTAMP WHERE id = ?1", params![job_id]).map_err(|error| error.to_string())?;
            log::info!("process_transcription_job: job_id={job_id} meeting_id={meeting_id} completed segments={count}");
            let _ = retention::record_audit(&connection, "transcription_completed", Some("local"), Some("transcription"), Some(&workspace_id), &format!("job_id={job_id} meeting_id={meeting_id} segments={count}"));
            Ok(TranscriptionJob { id: job_id.clone(), meeting_id: meeting_id.clone(), provider, status: "completed".into(), error: None })
        }
        Err(error) => {
            log::error!("process_transcription_job: job_id={job_id} meeting_id={meeting_id} failed: {error}");
            let _ = retention::record_audit(&connection, "transcription_failed", Some("local"), Some("transcription"), Some(&workspace_id), &format!("job_id={job_id} meeting_id={meeting_id} error={error}"));
            connection.execute("UPDATE transcription_jobs SET status = 'failed', error = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2", params![error, job_id]).map_err(|db_error| db_error.to_string())?;
            Err(error.clone())
        }
    }
}

#[tauri::command]
fn list_transcript_segments(app: tauri::AppHandle, meeting_id: String) -> Result<Vec<TranscriptSegmentRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, speaker, start_seconds, end_seconds, text, confidence, speaker_confidence FROM transcript_segments WHERE meeting_id = ?1 ORDER BY start_seconds").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![meeting_id], |row| Ok(TranscriptSegmentRecord { id: row.get(0)?, speaker: row.get(1)?, start_seconds: row.get(2)?, end_seconds: row.get(3)?, text: row.get(4)?, confidence: row.get(5)?, speaker_confidence: row.get(6)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn update_transcript_speaker(app: tauri::AppHandle, segment_id: String, speaker: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let changed = connection.execute("UPDATE transcript_segments SET speaker = ?1 WHERE id = ?2", params![speaker.trim(), segment_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Transcript segment was not found".into()); }
    Ok(())
}

#[tauri::command]
fn summarize_meeting(app: tauri::AppHandle, meeting_id: String, model: String) -> Result<MeetingSummary, String> {
    log::info!("summarize_meeting: meeting={meeting_id} requested_model={model}");
    let connection = open_database(&app)?;
    let workspace_id: String = connection.query_row("SELECT workspace_id FROM meetings WHERE id = ?1", params![meeting_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    let model = routed_model(&connection, &workspace_id, "summarization", &model);
    let mut statement = connection.prepare("SELECT COALESCE(speaker, 'Unknown speaker'), start_seconds, text FROM transcript_segments WHERE meeting_id = ?1 ORDER BY start_seconds").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![meeting_id], |row| Ok(format!("[{}s] {}: {}", row.get::<_, f64>(1)?, row.get::<_, String>(0)?, row.get::<_, String>(2)?))).map_err(|error| error.to_string())?;
    let transcript = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?.join("\n");
    if transcript.is_empty() { return Err("This meeting has no transcript segments to summarize".into()); }
    let prompt = format!("Summarize this meeting. Return concise markdown with sections Summary, Decisions, and Action items. For each action item include owner if known and due date if known.\n\nTRANSCRIPT:\n{transcript}");
    let summary_text = chat::chat_completion(&connection, &workspace_id, "summarization", &model, &prompt)?;
    connection.execute("INSERT INTO meeting_summaries (meeting_id, model, summary) VALUES (?1, ?2, ?3) ON CONFLICT(meeting_id) DO UPDATE SET model = excluded.model, summary = excluded.summary, created_at = CURRENT_TIMESTAMP", params![meeting_id, model, summary_text]).map_err(|error| error.to_string())?;
    connection.execute("DELETE FROM meeting_actions WHERE meeting_id = ?1", params![meeting_id]).map_err(|error| error.to_string())?;
    let in_actions = summary_text.lines().skip_while(|line| !line.to_lowercase().contains("action item")).skip(1);
    let mut actions = Vec::new();
    for line in in_actions {
        let title = line.trim().trim_start_matches(['-', '*', '[', ']', ' ']).trim();
        if title.is_empty() || title.starts_with('#') { continue; }
        let action = MeetingAction { id: format!("action-{}", uuid::Uuid::new_v4()), title: title.to_string(), owner: None, due_at: None, status: "open".into() };
        connection.execute("INSERT INTO meeting_actions (id, meeting_id, title, status) VALUES (?1, ?2, ?3, 'open')", params![action.id, meeting_id, action.title]).map_err(|error| error.to_string())?;
        actions.push(action);
        if actions.len() >= 20 { break; }
    }
    log::info!("summarize_meeting: meeting={meeting_id} summary {} chars, {} actions extracted", summary_text.len(), actions.len());
    Ok(MeetingSummary { meeting_id, model, summary: summary_text, actions })
}

#[tauri::command]
fn promote_action_to_task(app: tauri::AppHandle, action_id: String, workspace_id: String) -> Result<TaskRecord, String> {
    let connection = open_database(&app)?;
    let title: String = connection.query_row("SELECT title FROM meeting_actions WHERE id = ?1", params![action_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    let id = format!("task-{}", uuid::Uuid::new_v4());
    connection.execute("INSERT INTO tasks (id, workspace_id, title, status) VALUES (?1, ?2, ?3, 'open')", params![id, workspace_id, title]).map_err(|error| error.to_string())?;
    connection.execute("UPDATE meeting_actions SET status = 'promoted' WHERE id = ?1", params![action_id]).map_err(|error| error.to_string())?;
    Ok(TaskRecord { id, workspace_id, title, due_at: None, status: "open".into() })
}

/// Load a meeting's stored summary and extracted action items (None when the
/// meeting has never been summarized). Lets the UI re-open saved summaries.
#[tauri::command]
fn get_meeting_summary(app: tauri::AppHandle, meeting_id: String) -> Result<Option<MeetingSummary>, String> {
    let connection = open_database(&app)?;
    let row: Option<(String, String, String)> = match connection.query_row(
        "SELECT meeting_id, model, summary FROM meeting_summaries WHERE meeting_id = ?1",
        params![meeting_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ) {
        Ok(values) => Some(values),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(e.to_string()),
    };
    let Some((mid, model, summary)) = row else { return Ok(None) };
    let mut stmt = connection.prepare(
        "SELECT id, title, owner, due_at, status FROM meeting_actions WHERE meeting_id = ?1 ORDER BY id"
    ).map_err(|e| e.to_string())?;
    let actions = stmt.query_map(params![meeting_id], |r| Ok(MeetingAction {
        id: r.get(0)?, title: r.get(1)?, owner: r.get(2)?, due_at: r.get(3)?, status: r.get(4)?,
    })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    log::info!("get_meeting_summary: meeting={meeting_id} actions={}", actions.len());
    Ok(Some(MeetingSummary { meeting_id: mid, model, summary, actions }))
}

#[tauri::command]
fn list_open_actions(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<ActionRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT actions.id, actions.meeting_id, actions.title, actions.status FROM meeting_actions actions JOIN meetings ON meetings.id = actions.meeting_id WHERE meetings.workspace_id = ?1 AND actions.status = 'open' ORDER BY actions.rowid DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(ActionRecord { id: row.get(0)?, meeting_id: row.get(1)?, title: row.get(2)?, status: row.get(3)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn list_tasks(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<TaskRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, title, due_at, status FROM tasks WHERE workspace_id = ?1 ORDER BY rowid DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(TaskRecord { id: row.get(0)?, workspace_id: row.get(1)?, title: row.get(2)?, due_at: row.get(3)?, status: row.get(4)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn update_task_status(app: tauri::AppHandle, task_id: String, status: String) -> Result<(), String> {
    if !["open", "completed", "archived"].contains(&status.as_str()) { return Err("Unsupported task status".into()); }
    let connection = open_database(&app)?;
    connection.execute("UPDATE tasks SET status = ?1 WHERE id = ?2", params![status, task_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn create_task(app: tauri::AppHandle, workspace_id: String, title: String, due_at: Option<String>) -> Result<TaskRecord, String> {
    if title.trim().is_empty() { return Err("Task title is required".into()); }
    if let Some(ref due) = due_at { if chrono::DateTime::parse_from_rfc3339(due).is_err() { return Err("due_at must be an RFC3339 timestamp".into()); } }
    let connection = open_database(&app)?;
    let id = format!("task-{}", uuid::Uuid::new_v4());
    connection.execute("INSERT INTO tasks (id, workspace_id, title, due_at, status) VALUES (?1, ?2, ?3, ?4, 'open')", params![id, workspace_id, title.trim(), due_at]).map_err(|error| error.to_string())?;
    Ok(TaskRecord { id, workspace_id, title: title.trim().to_string(), due_at, status: "open".into() })
}

#[tauri::command]
fn delete_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let changed = connection.execute("DELETE FROM tasks WHERE id = ?1", params![task_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Task not found".into()); }
    Ok(())
}

#[tauri::command]
fn schedule_task_reminder(app: tauri::AppHandle, workspace_id: String, task_id: String, title: String, scheduled_at: String) -> Result<ReminderRecord, String> {
    if chrono::DateTime::parse_from_rfc3339(&scheduled_at).is_err() { return Err("scheduled_at must be an RFC3339 timestamp".into()); }
    let connection = open_database(&app)?;
    let id = format!("reminder-{}", uuid::Uuid::new_v4());
    connection.execute("INSERT INTO reminders (id, workspace_id, task_id, title, scheduled_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, workspace_id, task_id, title, scheduled_at]).map_err(|error| error.to_string())?;
    Ok(ReminderRecord { id, workspace_id, task_id: Some(task_id), calendar_event_id: None, title, scheduled_at, status: "scheduled".into() })
}

#[tauri::command]
fn list_reminders(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<ReminderRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, task_id, calendar_event_id, title, scheduled_at, status FROM reminders WHERE workspace_id = ?1 AND status = 'scheduled' ORDER BY scheduled_at").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(ReminderRecord { id: row.get(0)?, workspace_id: row.get(1)?, task_id: row.get(2)?, calendar_event_id: row.get(3)?, title: row.get(4)?, scheduled_at: row.get(5)?, status: row.get(6)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_reminder(app: tauri::AppHandle, reminder_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let changed = connection.execute("UPDATE reminders SET status = 'cancelled' WHERE id = ?1 AND status = 'scheduled'", params![reminder_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Reminder not found or already completed".into()); }
    Ok(())
}

#[tauri::command]
fn delete_reminder(app: tauri::AppHandle, reminder_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let changed = connection.execute("DELETE FROM reminders WHERE id = ?1", params![reminder_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Reminder not found".into()); }
    Ok(())
}

#[tauri::command]
fn create_calendar_event(app: tauri::AppHandle, workspace_id: String, title: String, starts_at: String, ends_at: String, meeting_url: Option<String>, recurrence: Option<String>) -> Result<CalendarEventRecord, String> {
    if chrono::DateTime::parse_from_rfc3339(&starts_at).is_err() || chrono::DateTime::parse_from_rfc3339(&ends_at).is_err() { return Err("Calendar times must be RFC3339 timestamps".into()); }
    let id = format!("event-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, meeting_url, recurrence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![id, workspace_id, title, starts_at, ends_at, meeting_url, recurrence]).map_err(|error| error.to_string())?;
    // Enqueue sync for connected providers
    let mut prov_stmt = connection.prepare("SELECT provider FROM calendar_connections WHERE workspace_id = ?1 AND status = 'synced'").ok();
    if let Some(ref mut stmt) = prov_stmt {
        let providers: Vec<String> = stmt.query_map(params![workspace_id], |row| row.get(0)).ok().and_then(|r| r.collect::<Result<Vec<_>, _>>().ok()).unwrap_or_default();
        for provider in providers {
            let _ = sync::enqueue_sync(&connection, "calendar_event", &id, &provider, "create", &serde_json::json!({"title": title, "starts_at": starts_at, "ends_at": ends_at, "meeting_url": meeting_url}));
        }
    }
    Ok(CalendarEventRecord { id, workspace_id, title, starts_at, ends_at, meeting_url, provider: "local".into(), recurrence })
}

#[tauri::command]
fn update_calendar_event(app: tauri::AppHandle, event_id: String, title: String, starts_at: String, ends_at: String, meeting_url: Option<String>, recurrence: Option<String>) -> Result<(), String> {
    if title.trim().is_empty() || chrono::DateTime::parse_from_rfc3339(&starts_at).is_err() || chrono::DateTime::parse_from_rfc3339(&ends_at).is_err() { return Err("Event title and RFC3339 times are required".into()); }
    let connection = open_database(&app)?;
    let changed = connection.execute("UPDATE calendar_events SET title = ?1, starts_at = ?2, ends_at = ?3, meeting_url = ?4, recurrence = ?5, updated_at = CURRENT_TIMESTAMP WHERE id = ?6", params![title.trim(), starts_at, ends_at, meeting_url, recurrence, event_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Calendar event was not found".into()); }
    Ok(())
}

#[tauri::command]
fn delete_calendar_event(app: tauri::AppHandle, event_id: String, agent_id: Option<String>) -> Result<(), String> {
    let connection = open_database(&app)?;
    // Approval enforcement for agent-initiated deletes
    if let Some(ref agent) = agent_id {
        let (workspace_id, _): (String, String) = connection.query_row(
            "SELECT workspace_id, provider FROM calendar_events WHERE id = ?1",
            params![event_id], |row| Ok((row.get(0)?, row.get(1)?))
        ).unwrap_or_default();
        let perm = permissions::check_permission(&connection, agent, &workspace_id, "schedule_events");
        if !perm.allowed {
            return Err(format!("Permission denied: {}", perm.reason));
        }
        let approval = approvals::check_and_request_approval(
            &connection, agent, &workspace_id, "schedule_events",
            &format!("Delete calendar event {}", event_id),
            &serde_json::json!({"event_id": event_id}),
        )?;
        if let Some(pending) = approval {
            return Err(format!("Approval required. Request ID: {}", pending.id));
        }
    }
    // Enqueue sync for connected providers before deleting
    let (_workspace_id, provider): (String, String) = connection.query_row(
        "SELECT workspace_id, provider FROM calendar_events WHERE id = ?1",
        params![event_id], |row| Ok((row.get(0)?, row.get(1)?))
    ).unwrap_or_default();
    if provider != "local" {
        let _ = sync::enqueue_sync(&connection, "calendar_event", &event_id, &provider, "delete", &serde_json::json!({"event_id": event_id}));
    }
    connection.execute("DELETE FROM calendar_events WHERE id = ?1", params![event_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_calendar_events(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<CalendarEventRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, title, starts_at, ends_at, meeting_url, provider, recurrence FROM calendar_events WHERE workspace_id = ?1 ORDER BY starts_at LIMIT 100").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(CalendarEventRecord { id: row.get(0)?, workspace_id: row.get(1)?, title: row.get(2)?, starts_at: row.get(3)?, ends_at: row.get(4)?, meeting_url: row.get(5)?, provider: row.get(6)?, recurrence: row.get(7)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn connect_calendar_provider(app: tauri::AppHandle, workspace_id: String, provider: String, account_label: String) -> Result<CalendarConnectionRecord, String> {
    if !["google", "outlook"].contains(&provider.as_str()) { return Err("Unsupported calendar provider".into()); }
    let id = format!("calendar-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO calendar_connections (id, workspace_id, provider, account_label, status) VALUES (?1, ?2, ?3, ?4, 'needs_auth') ON CONFLICT(workspace_id, provider, account_label) DO UPDATE SET status = 'needs_auth'", params![id, workspace_id, provider, account_label]).map_err(|error| error.to_string())?;
    Ok(CalendarConnectionRecord { id, workspace_id, provider, account_label, status: "needs_auth".into(), last_synced_at: None })
}

#[tauri::command]
fn list_calendar_connections(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<CalendarConnectionRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, provider, account_label, status, last_synced_at FROM calendar_connections WHERE workspace_id = ?1").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(CalendarConnectionRecord { id: row.get(0)?, workspace_id: row.get(1)?, provider: row.get(2)?, account_label: row.get(3)?, status: row.get(4)?, last_synced_at: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

/// Provider-default IMAP host when the user doesn't supply one (mirrors the
/// SMTP defaults used by `smtp_send`).
fn default_imap_host(provider: &str) -> Option<&'static str> {
    match provider {
        "gmail" => Some("imap.gmail.com"),
        "outlook" => Some("outlook.office365.com"),
        _ => None,
    }
}

/// Provider-default SMTP host (kept in sync with `smtp_send`'s fallback).
fn default_smtp_host(provider: &str) -> Option<&'static str> {
    match provider {
        "gmail" => Some("smtp.gmail.com"),
        "outlook" => Some("smtp.office365.com"),
        _ => None,
    }
}

/// Well-known IMAP/SMTP hosts inferred from the account address domain, so a
/// `user@gmail.com` / `user@yahoo.com` / `user@company.com` address gets the
/// right mail servers without the user typing them. Unknown domains return
/// None (the user supplies hosts manually).
fn hosts_for_address(address: &str) -> (Option<&'static str>, Option<&'static str>) {
    let domain = address.rsplit('@').next().unwrap_or("").trim().to_lowercase();
    match domain.as_str() {
        "gmail.com" | "googlemail.com" => (Some("imap.gmail.com"), Some("smtp.gmail.com")),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" | "office365.com" => (Some("outlook.office365.com"), Some("smtp-mail.outlook.com")),
        "yahoo.com" | "ymail.com" | "rocketmail.com" => (Some("imap.mail.yahoo.com"), Some("smtp.mail.yahoo.com")),
        "icloud.com" | "me.com" | "mac.com" => (Some("imap.mail.me.com"), Some("smtp.mail.me.com")),
        "aol.com" => (Some("imap.aol.com"), Some("smtp.aol.com")),
        "zoho.com" | "zohomail.com" => (Some("imap.zoho.com"), Some("smtp.zoho.com")),
        _ => (None, None),
    }
}

#[tauri::command]
fn connect_email_account(app: tauri::AppHandle, workspace_id: String, provider: String, account_label: String, imap_host: Option<String>, smtp_host: Option<String>) -> Result<EmailAccountRecord, String> {
    if !["gmail", "outlook", "imap"].contains(&provider.as_str()) { return Err("Unsupported email provider".into()); }
    // Default the hosts from the provider, then from the account address
    // domain (so `user@yahoo.com` gets imap.mail.yahoo.com automatically).
    let (imap_from_address, smtp_from_address) = hosts_for_address(&account_label);
    let imap_host = imap_host
        .or_else(|| default_imap_host(&provider).map(String::from))
        .or_else(|| imap_from_address.map(String::from));
    let smtp_host = smtp_host
        .or_else(|| default_smtp_host(&provider).map(String::from))
        .or_else(|| smtp_from_address.map(String::from));
    let id = format!("email-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO email_accounts (id, workspace_id, provider, account_label, imap_host, smtp_host) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(workspace_id, provider, account_label) DO UPDATE SET imap_host = excluded.imap_host, smtp_host = excluded.smtp_host, status = 'needs_auth'", params![id, workspace_id, provider, account_label, imap_host, smtp_host]).map_err(|error| error.to_string())?;
    Ok(EmailAccountRecord { id, workspace_id, provider, account_label, imap_host, smtp_host, status: "needs_auth".into() })
}

#[tauri::command]
fn list_email_accounts(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<EmailAccountRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, provider, account_label, imap_host, smtp_host, status FROM email_accounts WHERE workspace_id = ?1").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(EmailAccountRecord { id: row.get(0)?, workspace_id: row.get(1)?, provider: row.get(2)?, account_label: row.get(3)?, imap_host: row.get(4)?, smtp_host: row.get(5)?, status: row.get(6)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn store_email_secret(account_id: String, secret_kind: String, value: String) -> Result<(), String> {
    if !["username", "password", "oauth_refresh_token"].contains(&secret_kind.as_str()) { return Err("Unsupported email secret".into()); }
    let entry = keyring::Entry::new("neural-agent-os/email", &format!("{account_id}:{secret_kind}")).map_err(|error| error.to_string())?;
    entry.set_password(&value).map_err(|error| error.to_string())
}

#[tauri::command]
fn store_provider_secret(provider: String, secret_kind: String, value: String) -> Result<(), String> {
    if provider.trim().is_empty() || secret_kind.trim().is_empty() || value.is_empty() { return Err("Provider, secret kind, and value are required".into()); }
    let entry = keyring::Entry::new("neural-agent-os/provider", &format!("{provider}:{secret_kind}")).map_err(|error| error.to_string())?;
    entry.set_password(&value).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_provider_secret(provider: String, secret_kind: String) -> Result<(), String> {
    let entry = keyring::Entry::new("neural-agent-os/provider", &format!("{provider}:{secret_kind}")).map_err(|error| error.to_string())?;
    match entry.delete_credential() { Ok(()) | Err(keyring::Error::NoEntry) => Ok(()), Err(error) => Err(error.to_string()) }
}

#[tauri::command]
fn sync_email_account(app: tauri::AppHandle, account_id: String) -> Result<EmailSyncResult, String> {
    let connection = open_database(&app)?;
    let (provider, account_label, imap_host): (String, String, Option<String>) = connection.query_row("SELECT provider, account_label, imap_host FROM email_accounts WHERE id = ?1", params![account_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(|error| error.to_string())?;
    let (imap_from_address, _) = hosts_for_address(&account_label);
    let host = imap_host.clone()
        .or_else(|| default_imap_host(&provider).map(String::from))
        .or_else(|| imap_from_address.map(String::from))
        .ok_or_else(|| -> String { "IMAP host is required for this provider".to_string() })?;
    let username = keyring::Entry::new("neural-agent-os/email", &format!("{account_id}:username")).map_err(|error| error.to_string())?.get_password().map_err(|error| error.to_string())?;
    let password = keyring::Entry::new("neural-agent-os/email", &format!("{account_id}:password")).map_err(|error| error.to_string())?.get_password().map_err(|error| error.to_string())?;
    let tls = native_tls::TlsConnector::builder().build().map_err(|error| error.to_string())?;
    let client = imap::connect((host.as_str(), 993), &host, &tls).map_err(|error| error.to_string())?;
    let mut session = client.login(&username, &password).map_err(|error| error.0.to_string())?;
    session.select("INBOX").map_err(|error| error.to_string())?;
    let total = session.search("ALL").map_err(|error| error.to_string())?;
    let mut ids: Vec<u32> = total.iter().copied().collect();
    ids.sort_unstable_by(|left, right| right.cmp(left));
    ids.truncate(20);
    let mut stored = 0;
    for id in &ids {
        let fetches = session.fetch(id.to_string(), "RFC822") .map_err(|error| error.to_string())?;
        let Some(fetch) = fetches.iter().next() else { continue };
        let Some(raw) = fetch.body() else { continue };
        let parsed = mailparse::parse_mail(raw).map_err(|error| error.to_string())?;
        let subject = parsed.headers.get_first_value("Subject").unwrap_or_else(|| "(No subject)".into());
        let sender = parsed.headers.get_first_value("From");
        let recipients = parsed.headers.get_first_value("To");
        let body = parsed.get_body().ok();
        let email_id = format!("email-{}-{}", account_id, id);
        let inserted = connection.execute("INSERT OR IGNORE INTO emails (id, account_id, subject, sender, recipients, body, provider_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![email_id, account_id, subject, sender, recipients, body, id.to_string()]).map_err(|error| error.to_string())?;
        if inserted > 0 {
            let workspace_id: String = connection.query_row("SELECT workspace_id FROM email_accounts WHERE id = ?1", params![account_id], |row| row.get(0)).map_err(|error| error.to_string())?;
            connection.execute("INSERT INTO email_search (subject, sender, body, email_id, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5)", params![subject, sender.unwrap_or_default(), body.clone().unwrap_or_default(), email_id, workspace_id]).map_err(|error| error.to_string())?;
            stored += inserted;
        }
    }
    session.logout().map_err(|error| error.to_string())?;
    connection.execute("UPDATE email_accounts SET status = 'synced' WHERE id = ?1", params![account_id]).map_err(|error| error.to_string())?;
    Ok(EmailSyncResult { account_id, fetched: ids.len(), stored })
}

/// Sync an email account using OAuth REST APIs (Gmail API / Microsoft Graph).
/// Falls back to IMAP if no OAuth token is available.
#[tauri::command]
fn sync_email_oauth(app: tauri::AppHandle, account_id: String) -> Result<EmailSyncResult, String> {
    let connection = open_database(&app)?;
    let (provider, account_label, workspace_id): (String, String, String) = connection.query_row(
        "SELECT provider, account_label, workspace_id FROM email_accounts WHERE id = ?1",
        params![account_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|error| error.to_string())?;

    // Try to obtain an OAuth access token (only for gmail/outlook)
    let access_token = match provider.as_str() {
        "gmail" | "outlook" => oauth::refresh_access_token(&provider).ok(),
        _ => None,
    };

    let fetched: usize;
    let mut stored = 0usize;

    if let Some(token) = access_token {
        // OAuth REST sync
        let messages = match provider.as_str() {
            "gmail" => fetch_gmail_messages(&token)?,
            "outlook" => fetch_outlook_messages(&token)?,
            _ => vec![],
        };
        fetched = messages.len();
        for raw in messages {
            let email_id = format!("email-{}-oauth-{}", account_id, raw.provider_id);
            let inserted = connection.execute(
                "INSERT OR IGNORE INTO emails (id, account_id, subject, sender, recipients, body, received_at, provider_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![email_id, account_id, raw.subject, raw.sender, raw.recipients, raw.body, raw.received_at, raw.provider_id],
            ).map_err(|e| e.to_string())?;
            if inserted > 0 {
                let _ = connection.execute(
                    "INSERT INTO email_search (subject, sender, body, email_id, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![raw.subject, raw.sender.unwrap_or_default(), raw.body.clone().unwrap_or_default(), email_id, workspace_id],
                );
                stored += 1;
            }
        }
        connection.execute("UPDATE email_accounts SET status = 'synced' WHERE id = ?1", params![account_id]).map_err(|e| e.to_string())?;
        retention::record_audit(
            &connection, "email_oauth_sync", Some(&provider), Some("email"), Some(&workspace_id),
            &format!("OAuth sync of {account_label}: {stored} new of {fetched} fetched via REST API"),
        )?;
        return Ok(EmailSyncResult { account_id, fetched, stored });
    }

    // Fall back to IMAP/SMTP sync
    sync_email_account(app, account_id)
}

struct FetchedMessage {
    provider_id: String,
    subject: String,
    sender: Option<String>,
    recipients: Option<String>,
    body: Option<String>,
    received_at: Option<String>,
}

/// Fetch recent messages from the Gmail API
fn fetch_gmail_messages(access_token: &str) -> Result<Vec<FetchedMessage>, String> {
    let client = reqwest::blocking::Client::new();
    let list = client.get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
        .bearer_auth(access_token)
        .query(&[("maxResults", "50"), ("q", "in:inbox newer_than:30d")])
        .send()
        .map_err(|e| format!("Gmail API request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Gmail API error: {e}"))?
        .json::<serde_json::Value>()
        .map_err(|e| format!("Gmail API parse error: {e}"))?;

    let mut messages = Vec::new();
    if let Some(items) = list["messages"].as_array() {
        for item in items.iter().take(50) {
            let id = item["id"].as_str().unwrap_or("").to_string();
            if id.is_empty() { continue; }
            let detail = client.get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}"))
                .bearer_auth(access_token)
                .query(&[("format", "metadata"), ("metadataHeaders", "From"), ("metadataHeaders", "To"), ("metadataHeaders", "Subject"), ("metadataHeaders", "Date")])
                .send()
                .ok()
                .and_then(|r| r.json::<serde_json::Value>().ok());
            if let Some(d) = detail {
                let headers = d["payload"]["headers"].as_array().cloned().unwrap_or_default();
                let header = |name: &str| headers.iter().find(|h| h["name"].as_str() == Some(name)).and_then(|h| h["value"].as_str().map(String::from));
                let snippet = d["snippet"].as_str().unwrap_or("").to_string();
                messages.push(FetchedMessage {
                    provider_id: id,
                    subject: header("Subject").unwrap_or_else(|| "(No subject)".into()),
                    sender: header("From"),
                    recipients: header("To"),
                    body: Some(snippet),
                    received_at: header("Date"),
                });
            }
        }
    }
    Ok(messages)
}

/// Fetch recent messages from the Microsoft Graph API
fn fetch_outlook_messages(access_token: &str) -> Result<Vec<FetchedMessage>, String> {
    let client = reqwest::blocking::Client::new();
    let response = client.get("https://graph.microsoft.com/v1.0/me/messages")
        .bearer_auth(access_token)
        .query(&[("$top", "50"), ("$select", "id,subject,from,toRecipients,bodyPreview,receivedDateTime"), ("$orderby", "receivedDateTime desc")])
        .send()
        .map_err(|e| format!("Outlook API request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Outlook API error: {e}"))?
        .json::<serde_json::Value>()
        .map_err(|e| format!("Outlook API parse error: {e}"))?;

    let mut messages = Vec::new();
    if let Some(items) = response["value"].as_array() {
        for item in items {
            let id = item["id"].as_str().unwrap_or("").to_string();
            let sender = item["from"]["emailAddress"]["address"].as_str().map(String::from);
            let recipients = item["toRecipients"].as_array()
                .map(|arr| arr.iter().filter_map(|r| r["emailAddress"]["address"].as_str().map(String::from)).collect::<Vec<_>>().join(", "));
            messages.push(FetchedMessage {
                provider_id: id,
                subject: item["subject"].as_str().unwrap_or("(No subject)").to_string(),
                sender,
                recipients,
                body: item["bodyPreview"].as_str().map(String::from),
                received_at: item["receivedDateTime"].as_str().map(String::from),
            });
        }
    }
    Ok(messages)
}

#[tauri::command]
fn list_emails(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<EmailRecord>, String> {
    let connection = open_database(&app)?;
    let pattern = format!("%{}%", query.trim());
    let mut statement = connection.prepare("SELECT emails.id, emails.account_id, emails.subject, emails.sender, emails.recipients, emails.body, emails.received_at FROM emails JOIN email_accounts ON email_accounts.id = emails.account_id WHERE email_accounts.workspace_id = ?1 AND (?2 = '%%' OR emails.subject LIKE ?2 OR emails.body LIKE ?2 OR emails.sender LIKE ?2) ORDER BY emails.received_at DESC, emails.rowid DESC LIMIT 100").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id, pattern], |row| Ok(EmailRecord { id: row.get(0)?, account_id: row.get(1)?, subject: row.get(2)?, sender: row.get(3)?, recipients: row.get(4)?, body: row.get(5)?, received_at: row.get(6)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn embed_sources(app: tauri::AppHandle, workspace_id: String, model: String) -> Result<EmbeddingResult, String> {
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "embeddings", &model);
    let provider = chat::routed_provider(&connection, &workspace_id, "embeddings");
    let mut statement = connection.prepare("SELECT chunks.id, chunks.content FROM source_chunks chunks JOIN sources ON sources.id = chunks.source_id WHERE sources.workspace_id = ?1").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|error| error.to_string())?;
    let mut chunks_embedded = 0;
    for row in rows {
        let (chunk_id, content) = row.map_err(|error| error.to_string())?;
        let embedding = embed_text(&provider, &model, &content)?;
        connection.execute("INSERT INTO chunk_embeddings (chunk_id, model, vector_json) VALUES (?1, ?2, ?3) ON CONFLICT(chunk_id) DO UPDATE SET model = excluded.model, vector_json = excluded.vector_json, created_at = CURRENT_TIMESTAMP", params![chunk_id, model, serde_json::to_string(&embedding).map_err(|error| error.to_string())?]).map_err(|error| error.to_string())?;
        chunks_embedded += 1;
    }
    Ok(EmbeddingResult { chunks_embedded, model })
}

#[tauri::command]
fn semantic_search_sources(app: tauri::AppHandle, workspace_id: String, query: String, model: String) -> Result<Vec<SearchResult>, String> {
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "embeddings", &model);
    // Cloud egress audit for semantic search
    if model.contains("text-embedding-") || model.contains("embed-") && !model.contains("nomic") {
        let _ = retention::record_audit(&connection, "cloud_embeddings", Some("openai"), Some("search_query"), Some(&workspace_id),
            &format!("Sent search query ({} chars) to cloud embedding model {}", query.len(), model));
    }
    let model = routed_model(&connection, &workspace_id, "embeddings", &model);
    let provider = chat::routed_provider(&connection, &workspace_id, "embeddings");
    let query_embedding = embed_text(&provider, &model, &query)?;
    let mut statement = connection.prepare("SELECT embeddings.chunk_id, embeddings.vector_json, chunks.source_id, chunks.content, sources.title, sources.uri FROM chunk_embeddings embeddings JOIN source_chunks chunks ON chunks.id = embeddings.chunk_id JOIN sources ON sources.id = chunks.source_id WHERE sources.workspace_id = ?1").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, Option<String>>(5)?.unwrap_or_default()))).map_err(|error| error.to_string())?;
    let mut ranked = Vec::new();
    for row in rows {
        let (chunk_id, vector_json, source_id, content, title, uri) = row.map_err(|error| error.to_string())?;
        let vector = serde_json::from_str::<Vec<f32>>(&vector_json).map_err(|error| error.to_string())?;
        ranked.push((cosine_similarity(&query_embedding, &vector), SearchResult { source_id, title, uri, chunk_id, content }));
    }
    ranked.sort_by(|left, right| right.0.partial_cmp(&left.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(ranked.into_iter().take(20).map(|(_, result)| result).collect())
}

#[tauri::command]
fn ask_assistant(app: tauri::AppHandle, workspace_id: String, prompt: String, model: String) -> Result<AssistantReply, String> {
    if prompt.trim().is_empty() { return Err("Prompt cannot be empty".into()); }
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "chat", &model);

    // Query-relevant context: use FTS5 search to find relevant chunks
    let match_query = fts5_match_terms(&prompt);

    // Search document chunks
    let mut doc_results: Vec<String> = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT search.content FROM source_search search
         JOIN sources ON sources.id = search.source_id
         WHERE sources.workspace_id = ?1 AND source_search MATCH ?2
         ORDER BY rank LIMIT 8"
    ) {
        if let Ok(rows) = stmt.query_map(params![workspace_id.clone(), match_query.clone()], |row| row.get::<_, String>(0)) {
            doc_results = rows.filter_map(|r| r.ok()).collect();
        }
    }
    let documents = doc_results.join("\n");

    // Search emails with query relevance
    let email_pattern = format!("%{}%", prompt.split_whitespace().take(3).collect::<Vec<_>>().join("%"));
    let mut email_results: Vec<String> = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT subject || ': ' || COALESCE(body, '') FROM emails
         JOIN email_accounts ON email_accounts.id = emails.account_id
         WHERE email_accounts.workspace_id = ?1
         AND (emails.subject LIKE ?2 OR emails.body LIKE ?2)
         ORDER BY emails.received_at DESC LIMIT 5"
    ) {
        if let Ok(rows) = stmt.query_map(params![workspace_id.clone(), email_pattern], |row| row.get::<_, String>(0)) {
            email_results = rows.filter_map(|r| r.ok()).collect();
        }
    }
    let emails = email_results.join("\n");

    // Search transcript segments
    let transcript_pattern = format!("%{}%", prompt.split_whitespace().take(4).collect::<Vec<_>>().join("%"));
    let mut transcript_results: Vec<String> = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT COALESCE(ts.speaker, 'Speaker') || ': ' || ts.text FROM transcript_segments ts
         JOIN meetings ON meetings.id = ts.meeting_id
         WHERE meetings.workspace_id = ?1 AND ts.text LIKE ?2
         ORDER BY ts.start_seconds LIMIT 10"
    ) {
        if let Ok(rows) = stmt.query_map(params![workspace_id.clone(), transcript_pattern], |row| row.get::<_, String>(0)) {
            transcript_results = rows.filter_map(|r| r.ok()).collect();
        }
    }
    let transcripts = transcript_results.join("\n");

    // Query-relevant citations: use FTS5 to find matching sources
    let mut citations = Vec::new();
    if let Ok(mut cite_stmt) = connection.prepare(
        "SELECT sc.id, s.title, s.uri FROM source_chunks sc
         JOIN sources s ON s.id = sc.source_id
         JOIN source_search ss ON ss.chunk_id = sc.id
         WHERE s.workspace_id = ?1 AND source_search MATCH ?2
         ORDER BY rank LIMIT 5"
    ) {
        if let Ok(rows) = cite_stmt.query_map(params![workspace_id.clone(), match_query], |row| {
            Ok(AssistantCitation { chunk_id: row.get(0)?, title: row.get(1)?, uri: row.get(2)?, meeting_id: None, start_seconds: None })
        }) {
            citations = rows.filter_map(|r| r.ok()).collect();
        }
    }

    // Meeting-timestamp citations: matching transcript segments carry the
    // meeting id and start second so answers can cite the exact moment.
    if let Ok(mut ts_cite) = connection.prepare(
        "SELECT ts.id, m.title, m.id, ts.start_seconds FROM transcript_segments ts JOIN meetings m ON m.id = ts.meeting_id WHERE m.workspace_id = ?1 AND ts.text LIKE ?2 ORDER BY ts.start_seconds LIMIT 5"
    ) {
        if let Ok(rows) = ts_cite.query_map(params![workspace_id.clone(), transcript_pattern], |row| {
            Ok(AssistantCitation { title: row.get(1)?, uri: None, chunk_id: row.get(0)?, meeting_id: row.get(2)?, start_seconds: row.get(3)? })
        }) {
            for citation in rows {
                if let Ok(citation) = citation {
                    citations.push(citation);
                }
            }
        }
    }

    // Fallback to recent sources if no query matches found
    if citations.is_empty() {
        if let Ok(mut fallback) = connection.prepare(
            "SELECT source_chunks.id, sources.title, sources.uri FROM source_chunks
             JOIN sources ON sources.id = source_chunks.source_id
             WHERE sources.workspace_id = ?1 ORDER BY source_chunks.rowid DESC LIMIT 3"
        ) {
            if let Ok(rows) = fallback.query_map(params![workspace_id.clone()], |row| {
                Ok(AssistantCitation { chunk_id: row.get(0)?, title: row.get(1)?, uri: row.get(2)?, meeting_id: None, start_seconds: None })
            }) {
                citations = rows.filter_map(|r| r.ok()).collect();
            }
        }
    }

    let context = format!("RELEVANT DOCUMENTS:\n{documents}\n\nRELEVANT EMAILS:\n{emails}\n\nRELEVANT TRANSCRIPTS:\n{transcripts}");
    let full_prompt = format!("You are a local personal assistant. Answer the question using the context when relevant. Cite specific sources when you use them. Say when the context does not contain the answer.\n\nCONTEXT:\n{context}\n\nUSER QUESTION:\n{prompt}");
    let response_text = chat::chat_completion(&connection, &workspace_id, "chat", &model, &full_prompt)?;
    let conversation_id = format!("conversation-{}", uuid::Uuid::new_v4());
    connection.execute("INSERT INTO conversations (id, workspace_id, role, content, model) VALUES (?1, ?2, 'user', ?3, ?4), (?5, ?2, 'assistant', ?6, ?4)", params![conversation_id, workspace_id, prompt, model, format!("{}-reply", conversation_id), response_text]).map_err(|error| error.to_string())?;
    Ok(AssistantReply { conversation_id, response: response_text, model, citations })
}

#[tauri::command]
fn create_agent(app: tauri::AppHandle, workspace_id: String, name: String, prompt: String, schedule: String, permission_mode: String) -> Result<AgentRecord, String> {
    if name.trim().is_empty() || prompt.trim().is_empty() || schedule.trim().is_empty() { return Err("Agent name, prompt, and schedule are required".into()); }
    if !["approval_required", "read_only", "full"].contains(&permission_mode.as_str()) { return Err("Unsupported agent permission mode".into()); }
    let id = format!("agent-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO agents (id, workspace_id, name, prompt, schedule, permission_mode) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![id, workspace_id, name, prompt, schedule, permission_mode]).map_err(|error| error.to_string())?;
    Ok(AgentRecord { id, workspace_id, name, prompt, schedule, permission_mode, status: "paused".into(), last_run_at: None })
}

#[tauri::command]
fn list_agents(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<AgentRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, name, prompt, schedule, permission_mode, status, last_run_at FROM agents WHERE workspace_id = ?1 ORDER BY created_at DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(AgentRecord { id: row.get(0)?, workspace_id: row.get(1)?, name: row.get(2)?, prompt: row.get(3)?, schedule: row.get(4)?, permission_mode: row.get(5)?, status: row.get(6)?, last_run_at: row.get(7)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn update_agent_status(app: tauri::AppHandle, agent_id: String, status: String) -> Result<(), String> {
    if !["paused", "active", "archived"].contains(&status.as_str()) { return Err("Unsupported agent status".into()); }
    let connection = open_database(&app)?;
    connection.execute("UPDATE agents SET status = ?1 WHERE id = ?2", params![status, agent_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn update_agent(app: tauri::AppHandle, agent_id: String, name: String, prompt: String, schedule: String, permission_mode: String) -> Result<AgentRecord, String> {
    if name.trim().is_empty() || prompt.trim().is_empty() || schedule.trim().is_empty() { return Err("Agent name, prompt, and schedule are required".into()); }
    if !["approval_required", "read_only", "full"].contains(&permission_mode.as_str()) { return Err("Unsupported agent permission mode".into()); }
    let connection = open_database(&app)?;
    let changed = connection.execute("UPDATE agents SET name = ?1, prompt = ?2, schedule = ?3, permission_mode = ?4 WHERE id = ?5", params![name.trim(), prompt.trim(), schedule.trim(), permission_mode, agent_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Agent not found".into()); }
    let record = connection.query_row("SELECT id, workspace_id, name, prompt, schedule, permission_mode, status, last_run_at FROM agents WHERE id = ?1", params![agent_id], |row| Ok(AgentRecord { id: row.get(0)?, workspace_id: row.get(1)?, name: row.get(2)?, prompt: row.get(3)?, schedule: row.get(4)?, permission_mode: row.get(5)?, status: row.get(6)?, last_run_at: row.get(7)? })).map_err(|error| error.to_string())?;
    Ok(record)
}

#[tauri::command]
fn list_agent_runs(app: tauri::AppHandle, agent_id: String) -> Result<Vec<AgentRunRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, agent_id, status, output, error, retry_count FROM agent_runs WHERE agent_id = ?1 ORDER BY started_at DESC LIMIT 20").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![agent_id], |row| Ok(AgentRunRecord { id: row.get(0)?, agent_id: row.get(1)?, status: row.get(2)?, output: row.get(3)?, error: row.get(4)?, retry_count: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

/// One candidate for automatic recording, produced by the scheduler.
#[derive(Debug, PartialEq)]
struct AutoRecordPlan {
    event_id: String,
    workspace_id: String,
    title: String,
    ends_at: Option<String>,
    action: String, // automatic | confirm | disabled | notice_required
}

/// Find calendar events starting within the auto-record window (±2 min) that
/// have not yet been auto-recorded, and evaluate the recording rules for each.
/// Pure DB logic, unit-testable.
fn plan_auto_recordings(connection: &Connection) -> Result<Vec<AutoRecordPlan>, String> {
    let now = Utc::now();
    let floor = (now - chrono::Duration::seconds(60)).to_rfc3339();
    let ceiling = (now + chrono::Duration::seconds(120)).to_rfc3339();
    let mut stmt = connection.prepare(
        "SELECT id, workspace_id, title, starts_at, ends_at, meeting_url FROM calendar_events
         WHERE starts_at >= ?1 AND starts_at <= ?2 AND recording_auto_started IS NULL
         ORDER BY starts_at ASC LIMIT 10",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![floor, ceiling], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?))
    }).map_err(|e| e.to_string())?;
    let mut plans = Vec::new();
    for row in rows {
        let (event_id, workspace_id, title, _starts_at, ends_at, meeting_url) = row.map_err(|e| e.to_string())?;
        let participants: Vec<String> = {
            let mut pstmt = connection.prepare(
                "SELECT name FROM calendar_participants WHERE event_id = ?1",
            ).map_err(|e| e.to_string())?;
            let rows = pstmt.query_map(params![event_id], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
        };
        let action = calendar_complete::evaluate_recording_rules(connection, &workspace_id, &title, meeting_url.as_deref(), &participants)?;
        log::debug!("plan_auto_recordings: event={event_id} title={title} action={action}");
        plans.push(AutoRecordPlan { event_id, workspace_id, title, ends_at, action });
    }
    Ok(plans)
}

/// Scheduler-driven auto recording (#3): starts recordings for meetings that
/// match an `automatic`/`notice_required` recording rule when they come due,
/// and stops auto-started recordings shortly after the event's scheduled end.
fn run_auto_recording(app: &tauri::AppHandle) {
    // 1) Auto-stop: an auto-started recording whose event ended (>2 min ago).
    {
        let stop_at = RECORDER.lock().ok().and_then(|rec| rec.as_ref().and_then(|r| r.auto_stop_at.clone()));
        if let Some(end) = stop_at {
            let overdue = chrono::DateTime::parse_from_rfc3339(&end)
                .map(|parsed| chrono::Utc::now() > parsed + chrono::Duration::minutes(2))
                .unwrap_or(false);
            if overdue {
                let (ws, event_id, path) = RECORDER.lock().ok().map(|rec| rec.as_ref().map(|r| (r.workspace_id.clone(), r.event_id.clone().unwrap_or_default(), r.path.to_string_lossy().into_owned()))).flatten().unwrap_or_default();
                match stop_local_recording(app.clone()) {
                    Ok(_) => {
                        log::info!("run_auto_recording: auto-stopped recording for event={event_id} path={path}");
                        if let Ok(conn) = open_database(app) {
                            let _ = retention::record_audit(&conn, "auto_record_stopped", None, Some("audio"), Some(&ws), &format!("Auto-stopped recording for event {event_id} at {path}"));
                        }
                    }
                    Err(e) => log::error!("run_auto_recording: auto-stop failed for event={event_id}: {e}"),
                }
            }
        }
    }
    // 2) Auto-start: only when no recording is already active.
    let already_active = RECORDER.lock().map(|rec| rec.is_some()).unwrap_or(true);
    if already_active { return; }
    let Ok(connection) = open_database(app) else { return; };
    let plans = match plan_auto_recordings(&connection) { Ok(p) => p, Err(e) => { log::warn!("run_auto_recording: planning failed: {e}"); return; } };
    for plan in plans {
        match plan.action.as_str() {
            "automatic" | "notice_required" => {
                let path = data_subdir("recordings").join(format!("meeting-{}-{}.m4a", plan.event_id, Utc::now().format("%Y%m%dT%H%M%S")));
                let result = start_local_recording(
                    app.clone(), path.to_string_lossy().into_owned(), "default".into(),
                    Some(plan.workspace_id.clone()), Some(plan.title.clone()), None, vec![], true,
                );
                match result {
                    Ok(_) => {
                        let _ = connection.execute("UPDATE calendar_events SET recording_auto_started = ?1 WHERE id = ?2", params![Utc::now().to_rfc3339(), plan.event_id]);
                        if let Ok(mut rec) = RECORDER.lock() {
                            if let Some(r) = rec.as_mut() {
                                r.event_id = Some(plan.event_id.clone());
                                r.auto_stop_at = plan.ends_at.clone();
                            }
                        }
                        log::info!("run_auto_recording: started event={} title={} path={}", plan.event_id, plan.title, path.display());
                        let _ = retention::record_audit(&connection, "auto_record_started", None, Some("audio"), Some(&plan.workspace_id), &format!("Auto-started recording for event {} ({}): {}", plan.event_id, plan.title, path.display()));
                        best_effort_notify(app, "Recording started", &format!("Auto-recording: {}", plan.title));
                        return; // one recording at a time
                    }
                    Err(e) => {
                        log::error!("run_auto_recording: start failed event={} title={}: {e}", plan.event_id, plan.title);
                        let _ = retention::record_audit(&connection, "auto_record_failed", None, Some("audio"), Some(&plan.workspace_id), &format!("Auto-record start failed for event {}: {e}", plan.event_id));
                    }
                }
            }
            "confirm" => log::debug!("run_auto_recording: event={} requires confirmation; not auto-starting", plan.event_id),
            _ => log::debug!("run_auto_recording: event={} disabled by rules", plan.event_id),
        }
    }
}

fn scheduler_tick(app: &tauri::AppHandle) -> Result<(), String> {
    let connection = open_database(app)?;
    // Automatic meeting detection + recording (plan #3).
    run_auto_recording(app);
    // Process one queued job per tick (jobs run sequentially).
    match job_queue::process_next_job(app, &connection, "scheduler") {
        Ok(Some(job)) => log::info!("scheduler_tick: processed job {} ({})", job.id, job.job_type),
        Ok(None) => {}
        Err(error) => log::warn!("scheduler_tick: job processing error: {error}"),
    }
    let _ = sync::process_sync_queue(app);
    let _ = approvals::expire_old_approvals(&connection);
    // Pull remote calendar changes on schedule (every 15 minutes)
    if let Ok(mut stmt) = connection.prepare("SELECT id FROM calendar_connections WHERE status = 'synced'") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for conn_id in rows.filter_map(|r| r.ok()) {
                let last_pull: Option<String> = connection.query_row(
                    "SELECT last_synced_at FROM calendar_connections WHERE id = ?1",
                    params![conn_id], |row| row.get(0)
                ).ok().flatten();
                let should_pull = last_pull.map_or(true, |t| {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&t) {
                        chrono::Utc::now().signed_duration_since(dt.with_timezone(&chrono::Utc)).num_minutes() >= 15
                    } else { true }
                });
                if should_pull {
                    let _ = sync::pull_remote_calendar_events(app, &conn_id);
                }
            }
        }
    }
    // Refresh provider health every 5 minutes
    let last_health: Option<String> = connection.query_row(
        "SELECT checked_at FROM provider_health_log WHERE workspace_id = 'personal' ORDER BY checked_at DESC LIMIT 1",
        [], |row| row.get(0)
    ).ok();
    let should_check = last_health.map_or(true, |t| {
        let parsed = chrono::DateTime::parse_from_rfc3339(&t)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let diff = chrono::Utc::now().signed_duration_since(parsed);
        diff.num_minutes() >= 5
    });
    if should_check {
        if let Ok(mut stmt) = connection.prepare("SELECT id FROM workspaces") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                let ws_ids: Vec<String> = rows.filter_map(|r| r.ok()).collect();
                for ws_id in ws_ids {
                    let _ = health::refresh_provider_health(&connection, &ws_id);
                    // Auto-fix unhealthy providers by marking fallback entries
                    if let Ok(health_entries) = health::get_provider_health(&connection, &ws_id) {
                        for h in &health_entries {
                            if !h.healthy && h.consecutive_failures >= 2 {
                                let _ = fallback::mark_fallback_unhealthy(&connection, &ws_id, &h.capability, &h.provider);
                            }
                        }
                    }
                }
            }
        }
    }
    // Scheduled automatic backup every 24 hours
    let last_backup: Option<String> = connection.query_row(
        "SELECT MAX(created_at) FROM audit_log WHERE action = 'scheduled_backup'",
        [], |row| row.get(0)
    ).ok().flatten();
    let should_backup = last_backup.map_or(true, |t| {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&t) {
            chrono::Utc::now().signed_duration_since(dt.with_timezone(&chrono::Utc)).num_hours() >= 24
        } else { true }
    });
    if should_backup {
        let backup_dir = data_subdir("backups");
        let _ = std::fs::create_dir_all(&backup_dir);
        if let Ok(mut stmt) = connection.prepare("SELECT id FROM workspaces") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for ws_id in rows.filter_map(|r| r.ok()) {
                    let backup_path = backup_dir.join(format!("neural-backup-{}-{}.json", ws_id, Utc::now().format("%Y%m%dT%H%M%S")));
                    let _ = retention::schedule_backup(app, &connection, &ws_id, backup_path.to_string_lossy().as_ref());
                }
            }
        }
    }
    let _ = monitoring::check_monitored_folders(app, &connection);
    // Retention policy: auto-cleanup old recordings (30+ days) if retention setting enabled
    // Permanent retention is the privacy-safe default from the product plan;
    // cleanup only runs after the user explicitly enables the policy.
    let auto_cleanup: bool = connection.query_row(
        "SELECT value = 'true' FROM app_settings WHERE key = 'autoRetentionCleanup'",
        [], |row| row.get(0)
    ).unwrap_or(false);
    if auto_cleanup {
        // Use per-workspace retention policies when configured
        if let Ok(mut stmt) = connection.prepare("SELECT id FROM workspaces") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for ws_id in rows.filter_map(|r| r.ok()) {
                    let retention_days = retention::get_workspace_retention(&connection, &ws_id)?.unwrap_or(30);
                    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
                    let candidates: Vec<(String, String)> = if let Ok(mut stmt) = connection.prepare(
                        "SELECT id, recording_path FROM meetings WHERE workspace_id = ?1 AND recording_path IS NOT NULL AND COALESCE(started_at, created_at) < ?2",
                    ) {
                        stmt.query_map(
                            rusqlite::params![ws_id, cutoff.to_rfc3339()],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        ).map(|rows| rows.filter_map(Result::ok).collect()).unwrap_or_default()
                    } else { Vec::new() };
                    for (meeting_id, recording_path) in candidates {
                        match std::fs::remove_file(&recording_path) {
                            Ok(()) => log::info!("scheduler_tick: removed expired recording {recording_path}"),
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => log::debug!("scheduler_tick: expired recording already absent {recording_path}"),
                            Err(error) => { log::warn!("scheduler_tick: failed to remove expired recording {recording_path}: {error}"); continue; }
                        }
                        let _ = connection.execute("UPDATE meetings SET recording_path = NULL WHERE id = ?1", rusqlite::params![meeting_id]);
                    }
                }
            }
        }
        // Recording files are removed above before their DB paths are nulled.
        // Also cleanup old sync queue items (7+ days resolved)
        let _ = connection.execute(
            "DELETE FROM sync_queue WHERE status IN ('synced', 'failed', 'conflict') AND created_at < datetime('now', '-7 days')",
            [],
        );
        // Cleanup old audit log entries (90+ days)
        let _ = connection.execute(
            "DELETE FROM audit_log WHERE created_at < datetime('now', '-90 days')",
            [],
        );
        // Cleanup expired approval requests (7+ days resolved)
        let _ = connection.execute(
            "DELETE FROM approval_requests WHERE status IN ('approved', 'denied', 'expired') AND resolved_at < datetime('now', '-7 days')",
            [],
        );
    }
    let mut reminders = connection.prepare("SELECT id, title FROM reminders WHERE status = 'scheduled' AND julianday(scheduled_at) <= julianday('now')").map_err(|error| error.to_string())?;
    let due_reminders = reminders.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|error| error.to_string())?;
    for reminder in due_reminders {
        let (reminder_id, title) = reminder.map_err(|error| error.to_string())?;
        app.notification().builder().title("Neural reminder").body(title).show().map_err(|error| error.to_string())?;
        connection.execute("UPDATE reminders SET status = 'delivered' WHERE id = ?1", params![reminder_id]).map_err(|error| error.to_string())?;
    }
    let mut retry_statement = connection.prepare("SELECT agent_id FROM agent_runs current WHERE current.id = (SELECT latest.id FROM agent_runs latest WHERE latest.agent_id = current.agent_id ORDER BY latest.started_at DESC LIMIT 1) AND current.status = 'failed' AND current.retry_count < 3 AND current.started_at >= datetime('now', '-10 minutes') AND current.completed_at <= datetime('now', '-20 seconds')") .map_err(|error| error.to_string())?;
    let retry_agents = retry_statement.query_map([], |row| row.get::<_, String>(0)).map_err(|error| error.to_string())?;
    for agent_id in retry_agents {
        let agent_id = agent_id.map_err(|error| error.to_string())?;
        let active: bool = connection.query_row("SELECT status = 'active' FROM agents WHERE id = ?1", params![agent_id.clone()], |row| row.get(0)).unwrap_or(false);
        if active {
            let _ = run_agent_now(app.clone(), agent_id, default_lmstudio_model());
        }
    }
    let mut statement = connection.prepare("SELECT id, schedule FROM agents WHERE status = 'active'").map_err(|error| error.to_string())?;
    let agents = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|error| error.to_string())?;
    let now = Utc::now();
    for agent in agents {
        let (agent_id, expression) = agent.map_err(|error| error.to_string())?;
        let Ok(schedule) = Schedule::from_str(&expression) else { continue };
        let due = schedule.after(&(now - chrono::Duration::seconds(30))).next().map(|next| (next - now).num_seconds().abs() <= 30).unwrap_or(false);
        if !due { continue; }
        let already_ran: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE agent_id = ?1 AND started_at >= datetime('now', '-50 seconds'))", params![agent_id], |row| row.get(0)).map_err(|error| error.to_string())?;
        if !already_ran { let _ = run_agent_now(app.clone(), agent_id, default_lmstudio_model()); }
    }
    Ok(())
}

/// Decode percent-encoded characters (%XX) in query-string values.
/// '+' is treated as a space by callers before this runs.
fn percent_decode(input: &str) -> String {
    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn start_local_api(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let Ok(server) = tiny_http::Server::http("127.0.0.1:8787") else { return; };
        for mut request in server.incoming_requests() {
            let mut body_bytes: Vec<u8> = Vec::new();
            // Only read a body when the client actually sent one (Content-Length
            // > 0). Reading to EOF on a body-less GET over a keep-alive
            // connection would block the single-threaded server and stall every
            // other request. Bodies are read as raw bytes so binary payloads
            // (e.g. encrypted Teams-bot recordings) survive intact; JSON routes
            // convert lossily below.
            let body_length = request.body_length().unwrap_or(0);
            if body_length > 0 {
                let _ = request.as_reader().take(body_length as u64).read_to_end(&mut body_bytes);
            }
            let request_body = String::from_utf8_lossy(&body_bytes).into_owned();
            let full_url = request.url().to_string();
            let path = full_url.split('?').next().unwrap_or("/");
            let method = request.method().to_string();
            log::info!("[api] {method} {path}");
            let query_value = |key: &str| full_url.split('?').nth(1).and_then(|query| query.split('&').find_map(|part| {
                let mut pair = part.splitn(2, '=');
                (pair.next() == Some(key)).then(|| percent_decode(pair.next().unwrap_or("").replace('+', " ").as_str()))
            }));
            let (status, body) = match path {
                "/health" => (200, serde_json::json!({ "status": "ok", "local": true, "version": env!("CARGO_PKG_VERSION"), "timestamp": Utc::now().to_rfc3339() }).to_string()),
                "/diagnostics" => {
                    let db_path = database_path(&app).unwrap_or_default();
                    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                    let workspaces = list_workspaces(app.clone()).map(|w| w.len()).unwrap_or(0);
                    (200, serde_json::json!({
                        "status": "ok",
                        "version": env!("CARGO_PKG_VERSION"),
                        "database_path": db_path.to_string_lossy(),
                        "database_size_bytes": db_size,
                        "workspace_count": workspaces,
                        "ollama_url": std::env::var("NEURAL_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434/api/generate".into()),
                        "whisper_bin": resolve_binary("whisper", "NEURAL_WHISPER_BIN").unwrap_or_else(|_| "whisper".into()),
                        "ffmpeg_bin": resolve_binary("ffmpeg", "NEURAL_FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".into()),
                        "api_port": 8787,
                        "loopback_only": true,
                    }).to_string())
                }
                "/v1/workspaces" => match list_workspaces(app.clone()) { Ok(workspaces) => (200, serde_json::to_string(&workspaces).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) },
                "/v1/search" => match (query_value("workspace_id"), query_value("q")) { (Some(workspace_id), Some(query)) => match search_sources(app.clone(), workspace_id, query) { Ok(results) => (200, serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, _ => (400, serde_json::json!({ "error": "workspace_id and q are required" }).to_string()) },
                "/v1/tasks" => match query_value("workspace_id") { Some(workspace_id) => match list_tasks(app.clone(), workspace_id) { Ok(tasks) => (200, serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/meetings" => match query_value("workspace_id") { Some(workspace_id) => match list_meetings(app.clone(), workspace_id) { Ok(meetings) => (200, serde_json::to_string(&meetings).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/notes" => match query_value("workspace_id") { Some(workspace_id) => match list_notes(app.clone(), workspace_id) { Ok(notes) => (200, serde_json::to_string(&notes).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/transcripts" => match (query_value("workspace_id"), query_value("q")) { (Some(workspace_id), Some(query)) => match conversation_search(app.clone(), workspace_id, query) { Ok(results) => (200, serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, _ => (400, serde_json::json!({ "error": "workspace_id and q are required" }).to_string()) },
                "/v1/calendar/events" => match query_value("workspace_id") { Some(workspace_id) => match list_calendar_events(app.clone(), workspace_id) { Ok(events) => (200, serde_json::to_string(&events).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/emails" => match query_value("workspace_id") { Some(workspace_id) => match list_emails(app.clone(), workspace_id, String::new()) { Ok(emails) => (200, serde_json::to_string(&emails).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/approvals" => match query_value("workspace_id") { Some(workspace_id) => match list_pending_approvals(app.clone(), workspace_id) { Ok(approvals) => (200, serde_json::to_string(&approvals).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/permissions" => match (query_value("agent_id"), query_value("workspace_id")) { (Some(agent_id), Some(workspace_id)) => match check_all_permissions(app.clone(), agent_id, workspace_id) { Ok(perms) => (200, serde_json::to_string(&perms).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, _ => (400, serde_json::json!({ "error": "agent_id and workspace_id are required" }).to_string()) },
                "/v1/memories" => match query_value("workspace_id") { Some(workspace_id) => match list_memories(app.clone(), workspace_id) { Ok(memories) => (200, serde_json::to_string(&memories).unwrap_or_else(|_| "[]".into())), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/context/export" => match query_value("workspace_id") { Some(workspace_id) => match export_context(app.clone(), workspace_id.clone()) { Ok(context) => (200, serde_json::json!({ "workspace_id": workspace_id, "context": context }).to_string()), Err(error) => (500, serde_json::json!({ "error": error }).to_string()) }, None => (400, serde_json::json!({ "error": "workspace_id is required" }).to_string()) },
                "/v1/recording/start" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else {
                        let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                        let rec_path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let source = payload.get("source").and_then(|v| v.as_str()).unwrap_or("default").to_string();
                        let workspace_id = payload.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
                        match start_local_recording(app.clone(), rec_path, source, workspace_id, None, None, Vec::new(), true) {
                            Ok(status) => (200, serde_json::to_string(&status).unwrap_or_default()),
                            Err(e) => (500, serde_json::json!({ "error": e }).to_string()),
                        }
                    }
                },
                "/v1/recording/stop" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else {
                        match stop_local_recording(app.clone()) {
                            Ok(status) => (200, serde_json::to_string(&status).unwrap_or_default()),
                            Err(e) => (500, serde_json::json!({ "error": e }).to_string()),
                        }
                    }
                },
                                "/v1/tools/call" => {
                    // Generic gated tool surface (#9): full agent_tools
                    // catalogue with API-key auth, capability/approval
                    // enforcement, workspace scoping, logging, and audit.
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else if let Ok(conn) = open_database(&app) {
                        if let Err(auth_err) = require_api_auth(&conn, &request) {
                            (401, serde_json::json!({ "error": auth_err }).to_string())
                        } else {
                            let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                            let name = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let args = payload.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}));
                            match agent_tools::tool_by_name(&name) {
                                None => (400, serde_json::json!({ "error": format!("Unknown tool: {name}") }).to_string()),
                                Some(tool) => {
                                    let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let agent_id = args.get("agent_id").and_then(|v| v.as_str()).unwrap_or("cli").to_string();
                                    if tool.name != "list_agent_runs" && workspace_id.is_empty() {
                                        (400, serde_json::json!({ "error": "workspace_id is required" }).to_string())
                                    } else {
                                        let run_id = format!("rest-{}", uuid::Uuid::new_v4());
                                        match agent_tools::execute_agent_tool(&app, &conn, &workspace_id, &agent_id, &run_id, &name, &args) {
                                            Ok(value) => (200, serde_json::json!({ "tool": name, "result": value }).to_string()),
                                            Err(error) => {
                                                let status = if error.starts_with("Permission denied") || error.starts_with("Approval required") { 403 } else { 400 };
                                                (status, serde_json::json!({ "error": error }).to_string())
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else { (500, serde_json::json!({ "error": "Database unavailable" }).to_string()) }
                },
                "/v1/ask" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else {
                        let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                        let ws_id = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("");
                        let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                        let model = payload.get("model").and_then(|v| v.as_str()).unwrap_or(&default_lmstudio_model()).to_string();
                        if ws_id.is_empty() || prompt.is_empty() { (400, serde_json::json!({ "error": "workspace_id and prompt are required" }).to_string()) }
                        else { match ask_assistant(app.clone(), ws_id.to_string(), prompt.to_string(), model) { Ok(reply) => (200, serde_json::to_string(&reply).unwrap_or_default()), Err(e) => (500, serde_json::json!({ "error": e }).to_string()) } }
                    }
                },
                // Write endpoints with permission gating for external agents
                "/v1/notes/create" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else if let Ok(conn) = open_database(&app) {
                        match require_api_auth(&conn, &request) {
                            Err(auth_err) => (401, serde_json::json!({ "error": auth_err }).to_string()),
                            Ok(()) => {
                                let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                                let ws_id = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("");
                                let agent = payload.get("agent_id").and_then(|v| v.as_str()).unwrap_or("rest-api");
                                let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
                                let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                if ws_id.is_empty() || title.is_empty() { (400, serde_json::json!({ "error": "workspace_id and title are required" }).to_string()) }
                                else {
                                    let perm = permissions::check_permission(&conn, agent, ws_id, "create_notes");
                                    if !perm.allowed { (403, serde_json::json!({ "error": format!("Permission denied: {}", perm.reason) }).to_string()) }
                                    else { match create_note(app.clone(), ws_id.to_string(), title.to_string(), content.to_string()) { Ok(note) => (201, serde_json::to_string(&note).unwrap_or_default()), Err(e) => (500, serde_json::json!({ "error": e }).to_string()) } }
                                }
                            }
                        }
                    } else { (500, serde_json::json!({ "error": "Database unavailable" }).to_string()) }
                },
                "/v1/reminders/create" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else if let Ok(conn) = open_database(&app) {
                        match require_api_auth(&conn, &request) {
                            Err(auth_err) => (401, serde_json::json!({ "error": auth_err }).to_string()),
                            Ok(()) => {
                                let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                                let ws_id = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("");
                                let agent = payload.get("agent_id").and_then(|v| v.as_str()).unwrap_or("rest-api");
                                let task_id = payload.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
                                let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
                                let scheduled_at = payload.get("scheduled_at").and_then(|v| v.as_str()).unwrap_or("");
                                if ws_id.is_empty() || title.is_empty() || scheduled_at.is_empty() { (400, serde_json::json!({ "error": "workspace_id, title, and scheduled_at are required" }).to_string()) }
                                else {
                                    let perm = permissions::check_permission(&conn, agent, ws_id, "create_reminders");
                                    if !perm.allowed { (403, serde_json::json!({ "error": format!("Permission denied: {}", perm.reason) }).to_string()) }
                                    else { match schedule_task_reminder(app.clone(), ws_id.to_string(), task_id.to_string(), title.to_string(), scheduled_at.to_string()) { Ok(reminder) => (201, serde_json::to_string(&reminder).unwrap_or_default()), Err(e) => (500, serde_json::json!({ "error": e }).to_string()) } }
                                }
                            }
                        }
                    } else { (500, serde_json::json!({ "error": "Database unavailable" }).to_string()) }
                },
                "/v1/tasks/create" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else if let Ok(conn) = open_database(&app) {
                        match require_api_auth(&conn, &request) {
                            Err(auth_err) => (401, serde_json::json!({ "error": auth_err }).to_string()),
                            Ok(()) => {
                                let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                                let ws_id = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("");
                                let agent = payload.get("agent_id").and_then(|v| v.as_str()).unwrap_or("rest-api");
                                let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
                                let due_at = payload.get("due_at").and_then(|v| v.as_str()).map(|s| s.to_string());
                                if ws_id.is_empty() || title.is_empty() { (400, serde_json::json!({ "error": "workspace_id and title are required" }).to_string()) }
                                else {
                                    let perm = permissions::check_permission(&conn, agent, ws_id, "modify_tasks");
                                    if !perm.allowed { (403, serde_json::json!({ "error": format!("Permission denied: {}", perm.reason) }).to_string()) }
                                    else { match create_task(app.clone(), ws_id.to_string(), title.to_string(), due_at) { Ok(task) => (201, serde_json::to_string(&task).unwrap_or_default()), Err(e) => (500, serde_json::json!({ "error": e }).to_string()) } }
                                }
                            }
                        }
                    } else { (500, serde_json::json!({ "error": "Database unavailable" }).to_string()) }
                },
                "/oauth/callback" => {
                    let code = query_value("code").unwrap_or_default();
                    let _state = query_value("state").unwrap_or_default();
                    let provider = if request_body.contains("google") || full_url.contains("google") { "google" } else { "outlook" };
                    if code.is_empty() {
                        (400, serde_json::json!({ "error": "authorization code is required" }).to_string())
                    } else {
                        match exchange_oauth_code(app.clone(), provider.to_string(), code) {
                            Ok(message) => (200, format!("<html><body style='font-family:system-ui;display:grid;place-items:center;min-height:100vh;background:#10131a;color:#e9e9ed;text-align:center'><div><h2 style='color:#bb9cff'>✓ Authorized</h2><p>{message}</p><p style='color:#858b99;font-size:12px'>You may close this window and return to Neural Agent OS.</p></div></body></html>")),
                            Err(error) => (500, format!("<html><body style='font-family:system-ui;display:grid;place-items:center;min-height:100vh;background:#10131a;color:#e9e9ed;text-align:center'><div><h2 style='color:#ff8496'>Authorization Failed</h2><p>{error}</p></div></body></html>")),
                        }
                    }
                }
                "/mcp" => {
                    let rpc = serde_json::from_str::<serde_json::Value>(&request_body).unwrap_or_default();
                    let method = rpc.get("method").and_then(|value| value.as_str()).unwrap_or("");
                    let id = rpc.get("id").cloned().unwrap_or(serde_json::Value::Null);
                    match method {
                        "tools/list" => (200, serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": agent_tools::mcp_tools_list() }).to_string()),
                        "tools/call" => {
                            // Shared tool surface (#9): the same catalogue and
                            // permission/approval enforcement as the agent
                            // runtime (agent_tools::execute_agent_tool).
                            let rpc_args = rpc.get("params").cloned().unwrap_or_default();
                            let name = rpc_args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let params = rpc_args.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}));
                            match agent_tools::tool_by_name(&name) {
                                None => (400, serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": format!("Unknown tool: {name}") } }).to_string()),
                                Some(tool) => {
                                    let workspace_id = params.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let agent_id = params.get("agent_id").and_then(|v| v.as_str()).unwrap_or("mcp-client").to_string();
                                    if tool.name != "list_agent_runs" && workspace_id.is_empty() {
                                        (400, serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "workspace_id is required" } }).to_string())
                                    } else if let Ok(conn) = open_database(&app) {
                                        // Consistent authentication: every tool call
                                        // (read or write) requires an API key.
                                        if let Err(auth_err) = require_api_auth(&conn, &request) {
                                            let error_body = serde_json::json!({
                                                "jsonrpc": "2.0", "id": id,
                                                "error": { "code": -32001, "message": format!("Authentication required: {auth_err}") }
                                            }).to_string();
                                            let resp = tiny_http::Response::from_string(error_body).with_status_code(401);
                                            let _ = request.respond(resp);
                                            continue;
                                        }
                                        let run_id = format!("mcp-{}", uuid::Uuid::new_v4());
                                        match agent_tools::execute_agent_tool(&app, &conn, &workspace_id, &agent_id, &run_id, &name, &params) {
                                            Ok(value) => (200, serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": { "content": [{ "type": "text", "text": value.to_string() }] } }).to_string()),
                                            Err(error) => {
                                                let gated = error.starts_with("Permission denied") || error.starts_with("Approval required");
                                                let code = if gated { -32000 } else { -32602 };
                                                let status = if gated { 403 } else { 400 };
                                                (status, serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": error } }).to_string())
                                            }
                                        }
                                    } else {
                                        (500, serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": "Database unavailable" } }).to_string())
                                    }
                                }
                            }
                        }
                        _ => (400, serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } }).to_string()),
                    }
                }
                "/teams-bot/status" => {
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else if let Err(auth_err) = teams_bot_auth(&request) { (401, serde_json::json!({ "error": auth_err }).to_string()) }
                    else {
                        let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                        let bot_id = payload.get("bot_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                        let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                        let message = payload.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let meeting_id = payload.get("meeting_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let ws_id: String = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("personal").to_string();
                        log::info!("[teams-bot] status bot_id={bot_id} status={status} meeting_id={meeting_id:?} workspace={ws_id}");
                        let _ = open_database(&app).and_then(|conn| {
                            plan_extras::record_bot_run(&conn, &ws_id, &bot_id, &status, Some(&message), meeting_id.as_deref())?;
                            retention::record_audit(&conn, "teams_bot_status", Some("teams-bot"), Some("meeting"), Some(&ws_id), &format!("Bot {bot_id} reported {status}: {message}"))
                        });
                        if let Some(mid) = meeting_id {
                            let _ = open_database(&app).and_then(|conn| {
                                conn.execute("INSERT INTO transcription_jobs (id, meeting_id, provider, status) VALUES (?1, ?2, 'teams-bot', ?3)", params![format!("teams-status-{}", uuid::Uuid::new_v4()), mid, status])
                                    .map_err(|e| e.to_string())
                            });
                        }
                        (200, serde_json::json!({ "received": true, "bot_id": bot_id }).to_string())
                    }
                }
                "/teams-bot/meetings" => {
                    // Return upcoming meetings that have meeting URLs for bot participation.
                    // Bot-only endpoint: requires the shared-secret token.
                    if let Err(auth_err) = teams_bot_auth(&request) { (401, serde_json::json!({ "error": auth_err }).to_string()) }
                    else {
                        match open_database(&app) {
                            Ok(conn) => {
                                let now = Utc::now();
                                let cutoff = now + chrono::Duration::hours(48);
                                let stmt = conn.prepare("SELECT id, workspace_id, title, starts_at, ends_at, meeting_url FROM calendar_events WHERE meeting_url IS NOT NULL AND starts_at >= ?1 AND starts_at <= ?2 ORDER BY starts_at LIMIT 20").map_err(|e| e.to_string());
                                match stmt {
                                    Ok(mut s) => {
                                        let rows: Result<Vec<serde_json::Value>, String> = s.query_map(params![now.to_rfc3339(), cutoff.to_rfc3339()], |row| Ok(serde_json::json!({
                                            "id": row.get::<_, String>(0)?,
                                            "workspace_id": row.get::<_, String>(1)?,
                                            "title": row.get::<_, String>(2)?,
                                            "starts_at": row.get::<_, String>(3)?,
                                            "ends_at": row.get::<_, String>(4)?,
                                            "meeting_url": row.get::<_, Option<String>>(5)?,
                                        }))).map_err(|e| e.to_string()).and_then(|r| r.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string()));
                                        match rows {
                                            Ok(r) => { log::info!("[teams-bot] meetings returned {} upcoming", r.len()); (200, serde_json::to_string(&r).unwrap_or_default()) }
                                            Err(e) => (500, serde_json::json!({ "error": e }).to_string())
                                        }
                                    }
                                    Err(e) => (500, serde_json::json!({ "error": e }).to_string())
                                }
                            }
                            Err(e) => (500, serde_json::json!({ "error": e }).to_string())
                        }
                    }
                }
                "/vexa/webhook" => {
                    let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                    let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    let data = payload.get("data").unwrap_or(&serde_json::Value::Null);
                    match event {
                        "bot_scheduled" => {
                            let bot_id = data.get("bot_id").and_then(|v| v.as_str()).unwrap_or("");
                            let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("Vexa Meeting");
                            let _ = open_database(&app).and_then(|conn| {
                                let mid = format!("meeting-{}", uuid::Uuid::new_v4());
                                conn.execute("INSERT INTO meetings (id, workspace_id, title, status) VALUES (?1, 'personal', ?2, 'vexabot_scheduled')", params![mid, title])
                                    .map_err(|e| e.to_string())
                            });
                            (200, serde_json::json!({ "received": true, "bot_id": bot_id }).to_string())
                        }
                        "transcription_complete" => {
                            let bot_id = data.get("bot_id").and_then(|v| v.as_str()).unwrap_or("");
                            let transcript = data.get("transcript");
                            // Store transcript segments in DB
                            if let Some(segments) = transcript.and_then(|t| t.get("segments")).and_then(|s| s.as_array()) {
                                if let Ok(conn) = open_database(&app) {
                                    let mid = format!("meeting-{}", uuid::Uuid::new_v4());
                                    let _ = conn.execute("INSERT INTO meetings (id, workspace_id, title, status) VALUES (?1, 'personal', ?2, 'transcribed')", params![mid, format!("Vexa meeting {bot_id}")]);
                                    for (i, seg) in segments.iter().enumerate() {
                                        let text = seg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                        let speaker = seg.get("speaker").and_then(|s| s.as_str());
                                        let start = seg.get("start").and_then(|s| s.as_f64()).unwrap_or(0.0);
                                        let end = seg.get("end").and_then(|e| e.as_f64()).unwrap_or(0.0);
                                        let _ = conn.execute("INSERT INTO transcript_segments (id, meeting_id, speaker, start_seconds, end_seconds, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![format!("vexaseg-{bot_id}-{i}"), mid, speaker, start, end, text]);
                                    }
                                }
                            }
                            (200, serde_json::json!({ "received": true }).to_string())
                        }
                        _ => (200, serde_json::json!({ "received": true, "event": event }).to_string()),
                    }
                }
                "/vexa/transcript" => {
                    let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                    let bot_id = payload.get("bot_id").and_then(|v| v.as_str()).unwrap_or("");
                    let transcript = payload.get("transcript");
                    if let Some(segments) = transcript.and_then(|t| t.get("segments")).and_then(|s| s.as_array()) {
                        if let Ok(conn) = open_database(&app) {
                            for (i, seg) in segments.iter().enumerate() {
                                let text = seg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                let speaker = seg.get("speaker").and_then(|s| s.as_str());
                                let start = seg.get("start").and_then(|s| s.as_f64()).unwrap_or(0.0);
                                let end = seg.get("end").and_then(|e| e.as_f64()).unwrap_or(0.0);
                                let seg_id = format!("vexa-{}-{}", bot_id, i);
                                let _ = conn.execute("INSERT OR IGNORE INTO transcript_segments (id, meeting_id, speaker, start_seconds, end_seconds, text) VALUES (?1, (SELECT id FROM meetings WHERE status = 'vexabot_scheduled' LIMIT 1), ?2, ?3, ?4, ?5)", params![seg_id, speaker, start, end, text]);
                            }
                        }
                        (200, serde_json::json!({ "stored": true, "segments": segments.len() }).to_string())
                    } else {
                        (400, serde_json::json!({ "error": "No transcript segments found" }).to_string())
                    }
                }
                "/vexa/event" => {
                    let payload: serde_json::Value = serde_json::from_str(&request_body).unwrap_or_default();
                    let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    if let Ok(conn) = open_database(&app) {
                        let _ = retention::record_audit(
                            &conn,
                            &format!("vexa_{event}"),
                            Some("vexa"),
                            Some("meeting"),
                            Some("personal"),
                            &format!("Vexa event: {event}"),
                        );
                    }
                    (200, serde_json::json!({ "received": true }).to_string())
                }
                "/teams-bot/upload" => {
                    // Encrypted recording upload from the Teams bot. Contract:
                    //   POST /teams-bot/upload?meeting_id=<id>&title=<title>
                    //   Header: X-Teams-Bot-Token: <shared secret>
                    //   Body: nonce(12) || AES-256-GCM ciphertext+tag of the recording
                    // The body is read as raw bytes (binary-safe) and the recording
                    // is decrypted and validated locally before it is stored.
                    if method.as_str() != "POST" { (405, serde_json::json!({ "error": "POST required" }).to_string()) }
                    else if let Err(auth_err) = teams_bot_auth(&request) { (401, serde_json::json!({ "error": auth_err }).to_string()) }
                    else {
                        let meeting_id = query_value("meeting_id").unwrap_or_default();
                        let title = query_value("title").unwrap_or_else(|| "Teams recording".into());
                        if meeting_id.is_empty() {
                            (400, serde_json::json!({ "error": "meeting_id is required" }).to_string())
                        } else {
                            match teams_bot::shared_secret() {
                                None => (500, serde_json::json!({ "error": "NAO_TEAMS_BOT_SECRET is not configured on the app" }).to_string()),
                                Some(secret) => {
                                    let key = teams_bot::derive_key(&secret);
                                    match teams_bot::decrypt_recording(&key, &body_bytes) {
                                        Err(e) => {
                                            log::error!("[teams-bot] upload rejected meeting_id={meeting_id}: {e}");
                                            let _ = open_database(&app).and_then(|conn| retention::record_audit(&conn, "teams_bot_upload_failed", Some("teams-bot"), Some("meeting"), Some("personal"), &format!("Upload for {meeting_id} rejected: {e}")));
                                            (400, serde_json::json!({ "error": e }).to_string())
                                        }
                                        Ok(plaintext) => {
                                            let recordings_dir = data_subdir("teams-recordings");
                                            let _ = std::fs::create_dir_all(&recordings_dir);
                                            let output_path = recordings_dir.join(format!("{}.wav", meeting_id));
                                            match std::fs::write(&output_path, &plaintext) {
                                                Ok(_) => {
                                                    let result = open_database(&app).and_then(|conn| {
                                                        let mid = format!("meeting-{}", uuid::Uuid::new_v4());
                                                        conn.execute("INSERT INTO meetings (id, workspace_id, title, recording_path, status) VALUES (?1, 'personal', ?2, ?3, 'ready_for_transcription')", params![mid, title, output_path.to_string_lossy().into_owned()])
                                                            .map_err(|e| e.to_string())?;
                                                        retention::record_audit(&conn, "teams_bot_upload", Some("teams-bot"), Some("meeting"), Some("personal"), &format!("Encrypted recording {meeting_id} received, decrypted and stored at {}", output_path.display()))?;
                                                        Ok(mid)
                                                    });
                                                    match result {
                                                        Ok(mid) => {
                                                            log::info!("[teams-bot] upload ok meeting_id={meeting_id} decrypted={} bytes stored={} meeting_row={mid}", plaintext.len(), output_path.display());
                                                            (200, serde_json::json!({ "received": true, "meeting_id": meeting_id, "meeting_row": mid, "decrypted": true, "bytes": plaintext.len(), "path": output_path.to_string_lossy() }).to_string())
                                                        }
                                                        Err(e) => (500, serde_json::json!({ "error": format!("Failed to record meeting: {e}") }).to_string())
                                                    }
                                                }
                                                Err(e) => (500, serde_json::json!({ "error": format!("Failed to save recording: {e}") }).to_string())
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => (404, serde_json::json!({ "error": "not_found" }).to_string()),
            };
            if status >= 400 {
                log::error!("[api] {method} {path} -> HTTP {status}: {body}");
            } else {
                log::info!("[api] {method} {path} -> HTTP {status}");
            }
            let response = tiny_http::Response::from_string(body).with_status_code(status).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").expect("valid header"));
            let _ = request.respond(response);
        }
    });
}

// ── Vexa integration endpoints ─────────────────────────────────────────────
// These are handled in the main API server loop above.
// The /vexa/webhook and /vexa/transcript routes are added below.
// For now, add /vexa/* routes to the API server by patching start_local_api.
// (Routes registered above in the match block)

const MAX_AGENT_STEPS: usize = 6;

/// Fire a desktop notification without ever panicking the caller: the
/// notification plugin may not be registered (tests, plugin load failure).
fn best_effort_notify(app: &tauri::AppHandle, title: &str, body: &str) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.notification().builder().title(title).body(body).show();
    }));
    if result.is_err() {
        log::warn!("best_effort_notify: notification plugin unavailable, skipped notification \"{title}\"");
    }
}

/// The agent tool-calling loop (#7), independent of the Tauri runtime so it is
/// unit-testable with a scripted model + fake tool executor. Runs up to
/// MAX_AGENT_STEPS rounds: the model may emit a JSON tool call (executed via
/// `execute_tool`, result fed back) or plain text (final answer). Returns
/// `Ok((final_text, steps))` or `Err(message)` when the model errored or the
/// step budget was exhausted without a final answer.
struct AgentLoopOutcome {
    final_text: String,
    steps: Vec<serde_json::Value>,
    error: Option<String>,
}

fn run_agent_loop(
    workspace_id: &str,
    agent_id: &str,
    agent_name: &str,
    prompt: &str,
    mut model_call: impl FnMut(&str) -> Result<String, String>,
    mut execute_tool: impl FnMut(&str, &serde_json::Value) -> Result<serde_json::Value, String>,
) -> AgentLoopOutcome {
    let mut transcript = format!(
        "You are {agent_name}, an autonomous assistant for the '{workspace_id}' workspace.\n{}\n\nTask: {prompt}\n\nNow begin.",
        agent_tools::tool_list_prompt()
    );
    let mut steps: Vec<serde_json::Value> = Vec::new();
    let mut last_error: Option<String> = None;
    let mut final_text = String::new();

    for step in 0..MAX_AGENT_STEPS {
        log::info!("run_agent_loop: run_agent={agent_id} step={step} calling model");
        let response = match model_call(&transcript) {
            Ok(response) => response,
            Err(error) => {
                log::error!("run_agent_loop: agent={agent_id} step={step} model error: {error}");
                steps.push(serde_json::json!({ "step": step, "status": "error", "error": error }));
                last_error = Some(error);
                break;
            }
        };
        match agent_tools::parse_agent_tool_call(&response) {
            Some((tool, args)) => {
                log::info!("run_agent_loop: agent={agent_id} step={step} tool_call={tool}");
                match execute_tool(&tool, &args) {
                    Ok(result) => {
                        log::info!("run_agent_loop: agent={agent_id} step={step} tool={tool} ok");
                        steps.push(serde_json::json!({ "step": step, "tool": tool, "status": "ok", "result": result }));
                        transcript = format!(
                            "{transcript}\n\n[step {step}] You called {tool} and received:\n{result}\nContinue with the next tool or give the final answer as plain text."
                        );
                    }
                    Err(error) => {
                        log::warn!("run_agent_loop: agent={agent_id} step={step} tool={tool} error={error}");
                        steps.push(serde_json::json!({ "step": step, "tool": tool, "status": "error", "error": error }));
                        transcript = format!(
                            "{transcript}\n\n[step {step}] Tool {tool} failed: {error}\nRecover by adjusting the arguments, using another tool, or giving the final answer."
                        );
                    }
                }
            }
            None => {
                final_text = response.trim().to_string();
                log::info!("run_agent_loop: agent={agent_id} step={step} final_answer_chars={}", final_text.len());
                break;
            }
        }
    }
    // Budget exhausted or model error without a final answer.
    if final_text.is_empty() {
        let message = last_error.unwrap_or_else(|| format!("Reached the maximum of {MAX_AGENT_STEPS} tool steps without a final answer"));
        log::warn!("run_agent_loop: agent={agent_id} stopped: {message}");
        return AgentLoopOutcome { final_text, steps, error: Some(message) };
    }
    AgentLoopOutcome { final_text, steps, error: None }
}

#[tauri::command]
fn run_agent_now(app: tauri::AppHandle, agent_id: String, model: String) -> Result<AgentRunRecord, String> {
    let connection = open_database(&app)?;
    let (workspace_id, prompt, permission_mode, agent_name): (String, String, String, String) = connection.query_row(
        "SELECT workspace_id, prompt, permission_mode, name FROM agents WHERE id = ?1",
        params![agent_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    ).map_err(|error| error.to_string())?;
    log::info!("run_agent_now: agent={agent_id} name={agent_name} workspace={workspace_id} permission_mode={permission_mode}");
    // Permission enforcement: only allow if agent has read_only or higher
    if permission_mode == "approval_required" {
        // Check if there's a pending approval for this agent's actions
        let has_approval: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM approval_requests WHERE agent_id = ?1 AND workspace_id = ?2 AND status = 'approved' AND resolved_at >= datetime('now', '-1 hour'))",
            params![agent_id, workspace_id], |row| row.get(0)
        ).unwrap_or(false);
        if !has_approval {
            return Err("Agent requires approval before execution. Approve pending requests first.".into());
        }
    }
    let model = routed_model(&connection, &workspace_id, "chat", &model);
    // Cost limit check for cloud models
    if model.contains("gpt-") || model.contains("claude-") || model.contains("gemini-") {
        if let Ok(Some(limit)) = costs::get_cost_limit(&connection, &workspace_id, "chat") {
            if costs::check_cost_limit(&connection, &workspace_id, limit)? {
                return Err(format!("Chat cost limit of {} cents exceeded for this workspace. Switch to a local model.", limit));
            }
        }
    }
    let run_id = format!("agent-run-{}", uuid::Uuid::new_v4());
    let retry_count: i64 = connection.query_row("SELECT COALESCE(MAX(retry_count), -1) + 1 FROM agent_runs WHERE agent_id = ?1 AND status = 'failed' AND started_at >= datetime('now', '-10 minutes')", params![agent_id.clone()], |row| row.get(0)).unwrap_or(0);
    connection.execute("INSERT INTO agent_runs (id, agent_id, status, retry_count) VALUES (?1, ?2, 'running', ?3)", params![run_id, agent_id, retry_count]).map_err(|error| error.to_string())?;

    // Tool-calling loop: every tool call is permission-gated, logged, audited,
    // and recoverable (tool failures are fed back to the model, not fatal).
    let outcome = run_agent_loop(
        &workspace_id, &agent_id, &agent_name, &prompt,
        |transcript| chat::chat_completion(&connection, &workspace_id, "chat", &model, transcript),
        |tool, args| agent_tools::execute_agent_tool(&app, &connection, &workspace_id, &agent_id, &run_id, tool, args),
    );
    let steps_text = agent_steps_text(&outcome.steps);
    let output = if steps_text.is_empty() {
        outcome.final_text.clone()
    } else {
        format!("{}\n\n--- tool activity ---\n{steps_text}", outcome.final_text)
    };
    if let Some(error) = &outcome.error {
        connection.execute("UPDATE agent_runs SET status = 'failed', output = ?1, error = ?2, completed_at = CURRENT_TIMESTAMP WHERE id = ?3", params![output, error, run_id]).map_err(|db_error| db_error.to_string())?;
        let _ = retention::record_audit(&connection, "agent_run_failed", Some("agent"), Some("agent_run"), Some(&workspace_id), &format!("Agent {agent_name} ({agent_id}) run {run_id} failed after {} tool steps: {error}", outcome.steps.len()));
        best_effort_notify(&app, "Agent failed", &format!("{agent_name} failed: {error}"));
        return Err(error.clone());
    }
    connection.execute("UPDATE agent_runs SET status = 'completed', output = ?1, completed_at = CURRENT_TIMESTAMP WHERE id = ?2", params![output, run_id]).map_err(|error| error.to_string())?;
    connection.execute("UPDATE agents SET last_run_at = CURRENT_TIMESTAMP WHERE id = ?1", params![agent_id]).map_err(|error| error.to_string())?;
    let _ = retention::record_audit(&connection, "agent_run_completed", Some("agent"), Some("agent_run"), Some(&workspace_id), &format!("Agent {agent_name} ({agent_id}) run {run_id} completed with {} tool steps", outcome.steps.len()));
    best_effort_notify(&app, "Agent completed", &format!("{agent_name} finished successfully."));
    Ok(AgentRunRecord { id: run_id, agent_id, status: "completed".into(), output: Some(output), error: None, retry_count })
}

/// Render the tool-call trace for the run output and logs.
fn agent_steps_text(steps: &[serde_json::Value]) -> String {
    steps.iter().enumerate().map(|(n, s)| {
        let tool = s.get("tool").and_then(|v| v.as_str()).unwrap_or("model");
        let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
        format!("{}. {tool} \u{2192} {status}", n + 1)
    }).collect::<Vec<_>>().join("\n")
}

#[derive(serde::Serialize)]
struct RecordingDecision {
    allowed: bool,
    action: String, // automatic, confirm, disabled, notice_required
    reason: String,
}

#[tauri::command]
fn check_recording_allowed(app: tauri::AppHandle, workspace_id: Option<String>, event_title: Option<String>, meeting_url: Option<String>, participants: Vec<String>) -> Result<RecordingDecision, String> {
    let connection = open_database(&app)?;
    let ws_id = workspace_id.unwrap_or_else(|| "personal".into());

    // Evaluate recording rules when event context is provided
    if let Some(title) = event_title {
        let action = calendar_complete::evaluate_recording_rules(
            &connection, &ws_id, &title, meeting_url.as_deref(), &participants,
        )?;
        return Ok(RecordingDecision {
            allowed: action != "disabled",
            action: action.clone(),
            reason: match action.as_str() {
                "automatic" => "Matches a recording rule: record automatically".into(),
                "confirm" => "No automatic rule matched — confirmation required".into(),
                _ => "Matches a rule that disables recording for this meeting".into(),
            },
        });
    }

    // No event context: fall back to the global recording mode
    let mode: String = connection.query_row(
        "SELECT value FROM app_settings WHERE key = 'recordingMode'",
        [], |row| row.get(0),
    ).unwrap_or_else(|_| "confirm".into());
    Ok(RecordingDecision {
        allowed: mode != "disabled",
        action: mode.clone(),
        reason: format!("Global recording mode: {mode}"),
    })
}

#[tauri::command]
fn start_local_recording(app: tauri::AppHandle, path: String, source: String, workspace_id: Option<String>, event_title: Option<String>, meeting_url: Option<String>, participants: Vec<String>, confirmed: bool) -> Result<RecordingStatus, String> {
    let mut active = RECORDER.lock().map_err(|_| "Recorder state is unavailable".to_string())?;
    if active.is_some() { return Err("A recording is already active".into()); }

    // Enforce the recording rule engine before capture starts
    let connection = open_database(&app)?;
    let ws_id = workspace_id.unwrap_or_else(|| "personal".into());
    let decision = check_recording_allowed(
        app.clone(),
        Some(ws_id.clone()),
        event_title,
        meeting_url,
        participants,
    )?;
    if !decision.allowed {
        return Err(format!("Recording blocked by rule engine: {}", decision.reason));
    }
    if decision.action == "confirm" && !confirmed {
        return Err("Recording requires confirmation. Set confirmed=true after notifying participants.".into());
    }

    // Resolve relative/~/ paths against a writable location (packaged apps run
    // from `/`, which is read-only).
    let output = resolve_output_path(&app, &path)?;
    if output.extension().is_none() { return Err("Recording path must include an extension such as .wav or .m4a".into()); }
    if let Some(parent) = output.parent() { std::fs::create_dir_all(parent).map_err(|error| format!("Could not create recording folder {}: {error}", parent.display()))?; }

    // Record a notice marker file when configured
    let notice_enabled: bool = connection.query_row(
        "SELECT value = 'true' FROM app_settings WHERE key = 'recordingNotice'",
        [], |row| row.get(0),
    ).unwrap_or(false);
    if notice_enabled {
        let notice_path = output.with_extension("notice.txt");
        let _ = std::fs::write(&notice_path, "This conversation is being recorded by Neural Agent OS.");
    }

    let ffmpeg = resolve_binary("ffmpeg", "NEURAL_FFMPEG_BIN")?;
    let mut command = Command::new(&ffmpeg);
    command.arg("-y");
    #[cfg(target_os = "macos")]
    {
        if source == "combined" {
            // Combined capture: microphone + system audio mixed into one track.
            command.args(["-f", "avfoundation", "-i", ":0", "-f", "avfoundation", "-i", ":1", "-filter_complex", "amix=inputs=2:duration=longest"]);
        } else {
            command.args(["-f", "avfoundation", "-i", if source == "system" { ":1" } else { ":0" }]);
        }
    }
    #[cfg(target_os = "windows")]
    { command.args(["-f", "dshow", "-i", &source]); }
    #[cfg(target_os = "linux")]
    { command.args(["-f", "pulse", "-i", &source]); }
    let output_str = output.to_string_lossy().into_owned();
    // +faststart writes the moov atom up front so an interrupted capture still
    // yields a decodable file; stdin stays open so stop_local_recording can
    // send ffmpeg "q" for a clean finalize instead of SIGKILL.
    command.args(["-movflags", "+faststart", "-ac", "1", "-ar", "16000", output_str.as_str()])
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    log::info!("start_local_recording: launching ffmpeg -> {output_str} (source={source})");
    let mut child = command.spawn().map_err(|error| { log::error!("start_local_recording: ffmpeg spawn failed: {error}"); format!("Unable to start FFmpeg: {error}") })?;
    // FFmpeg can spawn successfully and then exit immediately when the
    // microphone/system device is unavailable. Detect that now so the UI/API
    // reports a clean start failure instead of returning 200 and failing on
    // stop later.
    std::thread::sleep(std::time::Duration::from_millis(250));
    if let Some(status) = child.try_wait().map_err(|error| format!("Unable to inspect FFmpeg: {error}"))? {
        let mut stderr = child.stderr.take();
        let mut detail = String::new();
        if let Some(stream) = stderr.as_mut() { use std::io::Read as _; let _ = stream.read_to_string(&mut detail); }
        log::error!("start_local_recording: FFmpeg exited immediately ({status}); stderr={detail}");
        return Err(format!("FFmpeg exited before recording started: {detail}"));
    }
    let stdin = child.stdin.take();
    let stderr = child.stderr.take();
    // Title follows the timestamped file name (e.g. meeting-2026-08-11_13-38-45)
    // so the meeting, its transcript, and its summary carry the recorded-at time.
    let title = output
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Meeting")
        .to_string();
    *active = Some(Recorder { child, stdin, stderr, path: output.clone(), workspace_id: ws_id.clone(), title, event_id: None, auto_stop_at: None });

    let _ = retention::record_audit(
        &connection, "recording_started", None, Some("audio"), Some(&ws_id),
        &format!("Local recording started: {output_str} (rule action: {})", decision.action),
    );
    Ok(RecordingStatus { active: true, path: Some(output_str), meeting_id: None, queued: false })
}

#[tauri::command]
fn stop_local_recording(app: tauri::AppHandle) -> Result<RecordingStatus, String> {
    let mut active = RECORDER.lock().map_err(|_| "Recorder state is unavailable".to_string())?;
    let Some(recorder) = active.take() else { return Ok(RecordingStatus { active: false, path: None, meeting_id: None, queued: false }); };
    let mut child = recorder.child;
    let mut stderr = recorder.stderr;
    let path_str = recorder.path.to_string_lossy().into_owned();
    // Graceful stop: send ffmpeg the "q" command so it finalizes the file
    // (writes the m4a moov atom). Fall back to SIGKILL after a short timeout.
    let mut stopped_gracefully = false;
    if let Some(mut stdin) = recorder.stdin {
        use std::io::Write as _;
        let _ = stdin.write_all(b"q");
        let _ = stdin.flush();
        drop(stdin);
        for _ in 0..15 {
            match child.try_wait() {
                Ok(Some(_)) => { stopped_gracefully = true; break; }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
                Err(_) => break,
            }
        }
    }
    if !stopped_gracefully {
        log::warn!("stop_local_recording: ffmpeg did not exit after graceful stop request; sending SIGKILL (file may be truncated)");
        let _ = child.kill();
    }
    let _ = child.wait();
    let stderr_text = stderr.as_mut().and_then(|stream| {
        use std::io::Read as _;
        let mut text = String::new();
        stream.read_to_string(&mut text).ok().map(|_| text)
    }).unwrap_or_default();
    match std::fs::metadata(&path_str) {
        Ok(meta) if meta.len() > 0 => log::info!("stop_local_recording: stopped capture of {path_str} ({} bytes, graceful={stopped_gracefully})", meta.len()),
        Ok(meta) => {
            log::error!("stop_local_recording: ffmpeg produced an empty recording: {path_str} ({} bytes, graceful={stopped_gracefully}); stderr={stderr_text}", meta.len());
            return Err(format!("Recording process produced an empty file: {path_str}. {stderr_text}"));
        }
        Err(error) => {
            log::error!("stop_local_recording: recorded file missing after stop: {path_str}: {error}; ffmpeg stderr={stderr_text}");
            return Err(format!("Recording process did not produce {path_str}: {stderr_text}"));
        }
    }
    // Auto-pipeline: import the recording as a meeting, then queue its
    // transcription. The scheduler processes jobs one at a time, so several
    // back-to-back recordings queue and transcribe sequentially.
    let meeting = import_meeting_recording(
        app.clone(),
        recorder.workspace_id.clone(),
        path_str.clone(),
        recorder.title.clone(),
    )?;
    let connection = open_database(&app)?;
    let payload = serde_json::json!({ "meeting_id": meeting.id });
    let job = job_queue::enqueue_job(&connection, "transcription", &recorder.workspace_id, &payload, 2)?;
    let _ = retention::record_audit(
        &connection, "recording_stopped", None, Some("audio"), Some(&recorder.workspace_id),
        &format!("Recording saved to {path_str}; transcription queued (meeting {}, job {})", meeting.id, job.id),
    );
    Ok(RecordingStatus { active: false, path: Some(path_str), meeting_id: Some(meeting.id), queued: true })
}

/// Start push-to-talk voice capture from the default microphone to a temp
/// WAV file. Used by the assistant composer when the Web Speech API is not
/// available in the desktop runtime (macOS/Linux WebViews).
#[tauri::command]
fn start_voice_capture(state: tauri::State<'_, VoiceCaptureState>, language: Option<String>) -> Result<String, String> {
    let mut active = state.0.lock().map_err(|_| "Voice capture state unavailable".to_string())?;
    if active.is_some() { return Err("A voice capture is already active".into()); }
    let path = data_subdir("tmp").join(format!("neural-voice-{}.wav", uuid::Uuid::new_v4()));
    let ffmpeg = resolve_binary("ffmpeg", "NEURAL_FFMPEG_BIN")?;
    let mut command = Command::new(&ffmpeg);
    command.arg("-y");
    #[cfg(target_os = "macos")] { command.args(["-f", "avfoundation", "-i", ":0"]); }
    #[cfg(target_os = "windows")] { command.args(["-f", "dshow", "-i", "audio=Microphone"]); }
    #[cfg(target_os = "linux")] { command.args(["-f", "pulse", "-i", "default"]); }
    command
        .args(["-ac", "1", "-ar", "16000", path.to_str().ok_or("Invalid temp path")?])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let child = command.spawn().map_err(|e| format!("Microphone capture failed: {e}. Check FFmpeg and microphone permissions."))?;
    *active = Some(VoiceCapture { child, path, language: language.unwrap_or_else(|| "en".into()) });
    Ok("capturing".into())
}

/// Stop voice capture and transcribe the clip with local Whisper.
#[tauri::command]
fn stop_voice_capture(app: tauri::AppHandle, state: tauri::State<'_, VoiceCaptureState>) -> Result<String, String> {
    let mut active = state.0.lock().map_err(|_| "Voice capture state unavailable".to_string())?;
    let Some(capture) = active.take() else { return Err("No active voice capture".into()); };
    let mut child = capture.child;
    // Ask FFmpeg to finalize the WAV first. SIGKILL can truncate the RIFF/data
    // chunks and causes Whisper to miss the last words or reject the file.
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status();
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let path = capture.path.clone();
    let path_str = path.to_string_lossy().into_owned();
    // The clip may be missing (mic permission denied / ffmpeg exited early);
    // fail with a clear message instead of a confusing Whisper file error.
    if !path.is_file() {
        let _ = std::fs::remove_file(&path);
        return Err("No audio was captured — check microphone permissions and try again.".into());
    }
    let result = local_speech_to_text(app, path_str, Some(capture.language.clone()));
    let _ = std::fs::remove_file(&path);
    result
}

// ── Hands-free voice mode: silence detection ──────────────────────────────
// Lets the frontend auto-answer ~2s after the user stops speaking, without a
// tap. Polls the in-progress WAV (16-bit PCM, mono, 16 kHz from FFmpeg) and
// reports whether speech has been detected and how silent the trailing audio
// is, so the UI can decide when to stop + transcribe automatically.

#[derive(serde::Serialize)]
struct VoiceSilenceStatus {
    has_capture: bool,
    file_exists: bool,
    has_speech: bool,
    trailing_rms: f32,
    duration_secs: f32,
}

/// RMS (0..1) of the trailing `window_secs` of a 16-bit PCM WAV, plus whether
/// any speech-level audio exists anywhere in the clip. Returns None when the
/// file is missing or not a decodable 16-bit PCM WAVE.
fn analyze_wav_silence(path: &std::path::Path, window_secs: f32) -> Option<(f32, bool, f32)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    // Scan chunks for fmt + data (robust to any chunk ordering).
    let mut off = 12usize;
    let mut channels = 1u16;
    let mut sample_rate = 16000u32;
    let mut bits = 16u16;
    let mut data_start = 0usize;
    let mut data_len = 0usize;
    while off + 8 <= bytes.len() {
        let id = &bytes[off..off + 4];
        let size = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().ok()?) as usize;
        match id {
            b"fmt " if size >= 16 => {
                channels = u16::from_le_bytes(bytes[off + 8 + 2..off + 8 + 4].try_into().ok()?);
                sample_rate = u32::from_le_bytes(bytes[off + 8 + 4..off + 8 + 8].try_into().ok()?);
                bits = u16::from_le_bytes(bytes[off + 8 + 14..off + 8 + 16].try_into().ok()?);
            }
            b"data" => {
                data_start = off + 8;
                data_len = size.min(bytes.len().saturating_sub(data_start));
                break;
            }
            _ => {}
        }
        off += 8 + size + (size & 1);
    }
    if data_len == 0 || bits != 16 || channels == 0 {
        return None;
    }
    let bytes_per_frame = (bits / 8) as usize * channels as usize;
    let frames = data_len / bytes_per_frame;
    if frames == 0 {
        return None;
    }
    let duration = frames as f32 / sample_rate as f32;
    let window_frames = ((window_secs * sample_rate as f32) as usize).min(frames);
    let start = data_start + (frames - window_frames) * bytes_per_frame;
    let mut sum_sq: u64 = 0;
    let mut max_abs: u32 = 0;
    // whole-clip speech check samples every frame's absolute value (i16)
    let mut clip_max: u32 = 0;
    let mut idx = data_start;
    while idx + 1 < data_start + data_len {
        let sample = i16::from_le_bytes([bytes[idx], bytes[idx + 1]]);
        let abs = sample.unsigned_abs() as u32;
        clip_max = clip_max.max(abs);
        if idx >= start {
            sum_sq += (sample as i64 * sample as i64) as u64;
            max_abs = max_abs.max(abs);
        }
        idx += bytes_per_frame;
    }
    let rms = ((sum_sq as f64 / window_frames as f64).sqrt() / 32768.0) as f32;
    let has_speech = clip_max > 2400; // ~ -23 dBFS peak (avoids background noise)
    Some((rms, has_speech, duration))
}

/// Streaming voice reply: sends tokens to the frontend as they are generated
/// so Ava can start speaking while the model is still writing. Skips the
/// knowledge-base context lookup (fast path for casual voice chat).
#[tauri::command]
fn stream_voice_reply(prompt: String, model: Option<String>, channel: tauri::ipc::Channel<String>) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".into());
    }
    let base = std::env::var("NEURAL_OPENAI_COMPATIBLE_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:1234/v1".into());
    let model = model
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("NEURAL_VOICE_MODEL").ok())
        .or_else(|| std::env::var("NEURAL_OPENAI_COMPATIBLE_MODEL").ok())
        .unwrap_or_else(|| "aisha/qwen3.5-4b-nothink".into());
    let system = "You are Ava, a cheerful assistant that appears as a 3D human avatar.                   Keep replies SHORT (1-3 short sentences) and conversational. Use an occasional emoji.";
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt }
        ],
        "temperature": 0.35,
        "max_tokens": 96,
        "stream": true
    });
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(format!("{base}/chat/completions"))
        .json(&body)
        .send()
        .map_err(|e| format!("LM Studio unavailable: {e}"))?
        .error_for_status()
        .map_err(|e| format!("LM Studio HTTP error: {e}"))?;

    use std::io::{BufRead, BufReader};
    let reader = BufReader::new(response);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() || !line.starts_with("data:") {
            continue;
        }
        let payload = line.trim_start_matches("data:").trim();
        if payload == "[DONE]" {
            break;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() {
                if !delta.is_empty() {
                    let _ = channel.send(delta.to_string());
                }
            }
        }
    }
    Ok(())
}

/// Poll for hands-free auto-answer: report capture + trailing silence state.
#[tauri::command]
fn check_voice_silence(state: tauri::State<'_, VoiceCaptureState>) -> VoiceSilenceStatus {
    let path = state
        .0
        .lock()
        .map(|active| active.as_ref().map(|c| c.path.clone()))
        .unwrap_or(None);
    let Some(path) = path else {
        return VoiceSilenceStatus { has_capture: false, file_exists: false, has_speech: false, trailing_rms: 0.0, duration_secs: 0.0 };
    };
    let file_exists = path.is_file();
    let (trailing_rms, has_speech, duration) = analyze_wav_silence(&path, 1.2)
        .unwrap_or((0.0, false, 0.0));
    VoiceSilenceStatus { has_capture: true, file_exists, has_speech, trailing_rms, duration_secs: duration }
}

fn table_export(connection: &Connection, sql: &str, workspace_id: &str) -> Result<Vec<serde_json::Value>, String> {
    use rusqlite::types::ValueRef;
    let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
    let columns = statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let rows = statement.query_map(params![workspace_id], |row| {
        let mut record = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            // Preserve INTEGER/REAL fidelity (the old Option<String> reader
            // silently nulled float columns like confidence and start_seconds).
            let value = match row.get_ref(index) {
                Ok(ValueRef::Null) => serde_json::Value::Null,
                Ok(ValueRef::Integer(i)) => serde_json::Value::Number(i.into()),
                Ok(ValueRef::Real(f)) => serde_json::Number::from_f64(f).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                Ok(ValueRef::Text(t)) => serde_json::Value::String(String::from_utf8_lossy(t).into_owned()),
                Ok(ValueRef::Blob(b)) => serde_json::Value::String(format!("<blob:{} bytes>", b.len())),
                Err(_) => serde_json::Value::Null,
            };
            record.insert(column.clone(), value);
        }
        Ok(serde_json::Value::Object(record))
    }).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Build a v2 workspace export (no file I/O, unit-testable). Every domain the
/// workspace owns is included so a restore reproduces the workspace.
fn build_workspace_export(connection: &Connection, workspace_id: &str) -> Result<WorkspaceExport, String> {
    let mut export = WorkspaceExport {
        version: 2,
        schema_version: 2,
        exported_at: Utc::now().to_rfc3339(),
        workspace_id: workspace_id.to_string(),
        content_hash: String::new(),
        row_counts: std::collections::HashMap::new(),
        workspaces: table_export(connection, "SELECT id, name, source_count, created_at FROM workspaces WHERE id = ?1", workspace_id)?,
        sources: table_export(connection, "SELECT id, workspace_id, kind, title, uri, status, content_hash, indexed_at, created_at FROM sources WHERE workspace_id = ?1", workspace_id)?,
        source_chunks: table_export(connection, "SELECT sc.id, sc.source_id, sc.chunk_index, sc.content FROM source_chunks sc JOIN sources s ON s.id = sc.source_id WHERE s.workspace_id = ?1", workspace_id)?,
        meetings: table_export(connection, "SELECT id, workspace_id, title, started_at, duration_seconds, recording_path, status, language, created_at FROM meetings WHERE workspace_id = ?1", workspace_id)?,
        transcript_segments: table_export(connection, "SELECT segments.id, segments.meeting_id, segments.speaker, segments.start_seconds, segments.end_seconds, segments.text, segments.confidence, segments.speaker_confidence FROM transcript_segments segments JOIN meetings ON meetings.id = segments.meeting_id WHERE meetings.workspace_id = ?1", workspace_id)?,
        meeting_summaries: table_export(connection, "SELECT ms.meeting_id, ms.model, ms.summary, ms.created_at FROM meeting_summaries ms JOIN meetings m ON m.id = ms.meeting_id WHERE m.workspace_id = ?1", workspace_id)?,
        meeting_actions: table_export(connection, "SELECT ma.id, ma.meeting_id, ma.title, ma.owner, ma.due_at, ma.status FROM meeting_actions ma JOIN meetings m ON m.id = ma.meeting_id WHERE m.workspace_id = ?1", workspace_id)?,
        transcription_jobs: table_export(connection, "SELECT tj.id, tj.meeting_id, tj.provider, tj.status, tj.language, tj.error FROM transcription_jobs tj JOIN meetings m ON m.id = tj.meeting_id WHERE m.workspace_id = ?1", workspace_id)?,
        tasks: table_export(connection, "SELECT id, workspace_id, title, due_at, status FROM tasks WHERE workspace_id = ?1", workspace_id)?,
        reminders: table_export(connection, "SELECT id, workspace_id, task_id, title, scheduled_at, status FROM reminders WHERE workspace_id = ?1", workspace_id)?,
        emails: table_export(connection, "SELECT emails.id, emails.account_id, emails.subject, emails.sender, emails.recipients, emails.body, emails.received_at FROM emails JOIN email_accounts ON email_accounts.id = emails.account_id WHERE email_accounts.workspace_id = ?1", workspace_id)?,
        email_accounts: table_export(connection, "SELECT id, workspace_id, provider, account_label, imap_host, smtp_host, status FROM email_accounts WHERE workspace_id = ?1", workspace_id)?,
        embeddings: table_export(connection, "SELECT ce.chunk_id, ce.model, ce.vector_json, ce.created_at FROM chunk_embeddings ce JOIN source_chunks sc ON sc.id = ce.chunk_id JOIN sources s ON s.id = sc.source_id WHERE s.workspace_id = ?1", workspace_id)?,
        voice_profiles: table_export(connection, "SELECT id, workspace_id, speaker_label, sample_count, created_at FROM voice_profiles WHERE workspace_id = ?1", workspace_id)?,
        speaker_embeddings: table_export(connection, "SELECT se.profile_id, se.model, se.vector_json, se.sample_count FROM speaker_embeddings se JOIN voice_profiles vp ON vp.id = se.profile_id WHERE vp.workspace_id = ?1", workspace_id)?,
        agents: table_export(connection, "SELECT id, workspace_id, name, prompt, schedule, permission_mode, status, last_run_at FROM agents WHERE workspace_id = ?1", workspace_id)?,
        agent_runs: table_export(connection, "SELECT ar.id, ar.agent_id, ar.status, ar.output, ar.error, ar.retry_count, ar.started_at, ar.completed_at FROM agent_runs ar JOIN agents a ON a.id = ar.agent_id WHERE a.workspace_id = ?1", workspace_id)?,
        notes: table_export(connection, "SELECT id, workspace_id, title, content, created_at, updated_at FROM notes WHERE workspace_id = ?1", workspace_id)?,
        calendar_events: table_export(connection, "SELECT id, workspace_id, title, starts_at, ends_at, meeting_url, recurrence, provider, external_id, created_at, updated_at FROM calendar_events WHERE workspace_id = ?1", workspace_id)?,
        calendar_participants: table_export(connection, "SELECT cp.id, cp.event_id, cp.name, cp.email, cp.status FROM calendar_participants cp JOIN calendar_events ce ON ce.id = cp.event_id WHERE ce.workspace_id = ?1", workspace_id)?,
        model_routes: table_export(connection, "SELECT workspace_id, capability, provider, model FROM model_routes WHERE workspace_id = ?1", workspace_id)?,
        memories: table_export(connection, "SELECT id, workspace_id, content, source_kind, source_id, created_at, updated_at FROM memories WHERE workspace_id = ?1", workspace_id)?,
        web_pages: table_export(connection, "SELECT id, workspace_id, url, title, fetched_at FROM web_pages WHERE workspace_id = ?1", workspace_id)?,
        bot_runs: table_export(connection, "SELECT id, workspace_id, bot_id, status, message, meeting_id, platform, reported_at FROM bot_runs WHERE workspace_id = ?1", workspace_id)?,
        agent_permissions: table_export(connection, "SELECT ap.agent_id, ap.workspace_id, ap.capability, CAST(ap.granted AS TEXT) AS granted FROM agent_permissions ap JOIN agents a ON a.id = ap.agent_id WHERE a.workspace_id = ?1", workspace_id)?,
        workspace_allowlists: table_export(connection, "SELECT workspace_id, capability FROM workspace_allowlists WHERE workspace_id = ?1", workspace_id)?,
    };
    // Row counts per domain (restore verification + diagnostics).
    let domains: Vec<(&str, Vec<serde_json::Value>)> = vec![
        ("workspaces", export.workspaces.clone()), ("sources", export.sources.clone()), ("source_chunks", export.source_chunks.clone()),
        ("meetings", export.meetings.clone()), ("transcript_segments", export.transcript_segments.clone()), ("meeting_summaries", export.meeting_summaries.clone()), ("meeting_actions", export.meeting_actions.clone()), ("transcription_jobs", export.transcription_jobs.clone()),
        ("tasks", export.tasks.clone()), ("reminders", export.reminders.clone()), ("emails", export.emails.clone()), ("email_accounts", export.email_accounts.clone()),
        ("embeddings", export.embeddings.clone()), ("voice_profiles", export.voice_profiles.clone()), ("speaker_embeddings", export.speaker_embeddings.clone()),
        ("agents", export.agents.clone()), ("agent_runs", export.agent_runs.clone()), ("notes", export.notes.clone()),
        ("calendar_events", export.calendar_events.clone()), ("calendar_participants", export.calendar_participants.clone()),
        ("model_routes", export.model_routes.clone()), ("memories", export.memories.clone()), ("web_pages", export.web_pages.clone()),
        ("bot_runs", export.bot_runs.clone()), ("agent_permissions", export.agent_permissions.clone()), ("workspace_allowlists", export.workspace_allowlists.clone()),
    ];
    for (name, rows) in domains {
        export.row_counts.insert(name.to_string(), rows.len());
    }
    // Integrity hash over the canonical payload (content_hash excluded).
    export.content_hash = export_content_hash(&export);
    Ok(export)
}

/// SHA-256 over the export with an empty `content_hash` (the canonical form).
fn export_content_hash(export: &WorkspaceExport) -> String {
    let mut canonical = export.clone();
    canonical.content_hash.clear();
    // Serialize through serde_json::Value so the byte layout is identical to
    // what validate_backup recomputes after re-parsing the file (object keys
    // serialize in the same order either way).
    let value = serde_json::to_value(&canonical).unwrap_or_default();
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    sha256_hex(&bytes)
}

/// Returns true when the embedded hash matches the recomputed canonical hash.
fn export_hash_valid(export: &WorkspaceExport) -> bool {
    if export.content_hash.is_empty() { return false; }
    export.content_hash == export_content_hash(export)
}

#[tauri::command]
fn export_workspace(app: tauri::AppHandle, workspace_id: String, path: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    let export = build_workspace_export(&connection, &workspace_id)?;
    let output = serde_json::to_vec_pretty(&export).map_err(|error| error.to_string())?;
    // Resolve relative/~/ paths against a writable location (packaged apps run
    // from `/`, which is read-only).
    let resolved = resolve_output_path(&app, &path)?;
    if let Some(parent) = resolved.parent() { std::fs::create_dir_all(parent).map_err(|error| format!("Could not create export folder {}: {error}", parent.display()))?; }
    std::fs::write(&resolved, output).map_err(|error| error.to_string())?;
    let total_rows: usize = export.row_counts.values().sum();
    log::info!("export_workspace: workspace_id={workspace_id} path={} rows={total_rows} domains={} hash={}", resolved.display(), export.row_counts.len(), export.content_hash);
    let _ = retention::record_audit(&connection, "workspace_exported", None, Some("workspace_export"), Some(&workspace_id), &format!("Exported {} rows across {} domains to {} (hash {})", total_rows, export.row_counts.len(), resolved.display(), &export.content_hash[..export.content_hash.len().min(12)]));
    Ok(resolved.to_string_lossy().into_owned())
}

#[tauri::command]
fn import_workspace(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let content = std::fs::read_to_string(&path).map_err(|error| format!("Could not read export file: {error}"))?;
    let archive: WorkspaceExport = serde_json::from_str(&content).map_err(|error| format!("Invalid workspace export: {error}"))?;
    if archive.version != 1 && archive.version != 2 {
        return Err(format!("Unsupported workspace export version: {}", archive.version));
    }
    // v2 integrity check: reject tampered/truncated archives before touching
    // the database. v1 archives have no hash and are accepted as-is.
    if archive.version == 2 && !export_hash_valid(&archive) {
        log::error!("import_workspace: integrity check failed for {path} (hash mismatch)");
        return Err("Export integrity check failed: the file was modified or corrupted after export. Regenerate the export and retry.".into());
    }
    log::info!("import_workspace: path={path} version={} schema={} workspace={} domains={}", archive.version, archive.schema_version, archive.workspace_id, archive.row_counts.len());
    let connection = open_database(&app)?;
    let restored = apply_workspace_import(&connection, &archive)?;
    let summary: Vec<String> = {
        let mut keys: Vec<&String> = restored.keys().collect();
        keys.sort();
        keys.iter().map(|key| format!("{}={}", key, restored[*key])).collect()
    };
    log::info!("import_workspace: workspace={} restore summary: {}", archive.workspace_id, summary.join(" "));
    let _ = retention::record_audit(&connection, "workspace_restored", None, Some("workspace_export"), Some(&archive.workspace_id), &format!("Restored from {path}: {}", summary.join(" ")));
    Ok(format!("Restored workspace {} from {} ({} domains, {} rows)", archive.workspace_id, path, restored.len(), restored.values().sum::<usize>()))
}

/// Apply every domain of an export to the database, in foreign-key-safe order.
/// Returns rows inserted per domain (INSERT OR IGNORE keeps existing rows, so
/// "inserted" counts rows actually added on this machine).
fn apply_workspace_import(connection: &Connection, archive: &WorkspaceExport) -> Result<std::collections::HashMap<String, usize>, String> {
    fn exec(connection: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize, String> {
        connection.execute(sql, params).map_err(|e| e.to_string())
    }
    let mut restored: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    // Workspace first (everything references it), then leaf domains.
    for row in &archive.workspaces { exec(&connection, "INSERT OR IGNORE INTO workspaces (id, name, source_count, created_at) VALUES (?1, ?2, ?3, COALESCE(?4, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "name"), field(row, "source_count").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0), field(row, "created_at")])?; }
    for row in &archive.sources { exec(&connection, "INSERT OR IGNORE INTO sources (id, workspace_id, kind, title, uri, status, content_hash, indexed_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, COALESCE(?9, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "kind"), field(row, "title"), field(row, "uri"), field(row, "status").unwrap_or_else(|| "pending".into()), field(row, "content_hash"), field(row, "indexed_at"), field(row, "created_at")])?; }
    for row in &archive.source_chunks { exec(&connection, "INSERT OR IGNORE INTO source_chunks (id, source_id, chunk_index, content) VALUES (?1, ?2, ?3, ?4)", params![field(row, "id"), field(row, "source_id"), field(row, "chunk_index").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0), field(row, "content").unwrap_or_default()])?; }
    for row in &archive.meetings { exec(&connection, "INSERT OR IGNORE INTO meetings (id, workspace_id, title, started_at, duration_seconds, recording_path, status, language, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, 'en'), COALESCE(?9, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "title"), field(row, "started_at"), field(row, "duration_seconds").and_then(|v| v.parse::<i64>().ok()), field(row, "recording_path"), field(row, "status").unwrap_or_else(|| "created".into()), field(row, "language"), field(row, "created_at")])?; }
    for row in &archive.transcript_segments { exec(&connection, "INSERT OR IGNORE INTO transcript_segments (id, meeting_id, speaker, start_seconds, end_seconds, text, confidence, speaker_confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![field(row, "id"), field(row, "meeting_id"), field(row, "speaker"), field(row, "start_seconds").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0), field(row, "end_seconds").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0), field(row, "text").unwrap_or_default(), field(row, "confidence").and_then(|v| v.parse::<f64>().ok()), field(row, "speaker_confidence").and_then(|v| v.parse::<f64>().ok())])?; }
    for row in &archive.meeting_summaries { exec(&connection, "INSERT OR IGNORE INTO meeting_summaries (meeting_id, model, summary, created_at) VALUES (?1, ?2, ?3, COALESCE(?4, CURRENT_TIMESTAMP))", params![field(row, "meeting_id"), field(row, "model").unwrap_or_default(), field(row, "summary").unwrap_or_default(), field(row, "created_at")])?; }
    for row in &archive.meeting_actions { exec(&connection, "INSERT OR IGNORE INTO meeting_actions (id, meeting_id, title, owner, due_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![field(row, "id"), field(row, "meeting_id"), field(row, "title").unwrap_or_default(), field(row, "owner"), field(row, "due_at"), field(row, "status").unwrap_or_else(|| "open".into())])?; }
    for row in &archive.transcription_jobs { exec(&connection, "INSERT OR IGNORE INTO transcription_jobs (id, meeting_id, provider, status, language, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![field(row, "id"), field(row, "meeting_id"), field(row, "provider").unwrap_or_else(|| "unconfigured".into()), field(row, "status").unwrap_or_else(|| "queued".into()), field(row, "language"), field(row, "error")])?; }
    for row in &archive.tasks { exec(&connection, "INSERT OR IGNORE INTO tasks (id, workspace_id, title, due_at, status) VALUES (?1, ?2, ?3, ?4, ?5)", params![field(row, "id"), field(row, "workspace_id"), field(row, "title"), field(row, "due_at"), field(row, "status").unwrap_or_else(|| "open".into())])?; }
    for row in &archive.reminders { exec(&connection, "INSERT OR IGNORE INTO reminders (id, workspace_id, task_id, title, scheduled_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![field(row, "id"), field(row, "workspace_id"), field(row, "task_id"), field(row, "title"), field(row, "scheduled_at"), field(row, "status").unwrap_or_else(|| "scheduled".into())])?; }
    for row in &archive.email_accounts { exec(&connection, "INSERT OR IGNORE INTO email_accounts (id, workspace_id, provider, account_label, imap_host, smtp_host, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![field(row, "id"), field(row, "workspace_id"), field(row, "provider").unwrap_or_default(), field(row, "account_label").unwrap_or_else(|| "Imported".into()), field(row, "imap_host"), field(row, "smtp_host"), field(row, "status").unwrap_or_else(|| "needs_auth".into())])?; }
    for row in &archive.emails { exec(&connection, "INSERT OR IGNORE INTO emails (id, account_id, subject, sender, recipients, body, received_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![field(row, "id"), field(row, "account_id"), field(row, "subject").unwrap_or_default(), field(row, "sender"), field(row, "recipients"), field(row, "body").unwrap_or_default(), field(row, "received_at")])?; }
    for row in &archive.embeddings { exec(&connection, "INSERT OR IGNORE INTO chunk_embeddings (chunk_id, model, vector_json, created_at) VALUES (?1, ?2, ?3, COALESCE(?4, CURRENT_TIMESTAMP))", params![field(row, "chunk_id"), field(row, "model"), field(row, "vector_json"), field(row, "created_at")])?; }
    for row in &archive.voice_profiles { exec(&connection, "INSERT OR IGNORE INTO voice_profiles (id, workspace_id, speaker_label, sample_count, created_at) VALUES (?1, ?2, ?3, COALESCE(?4, 0), COALESCE(?5, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "speaker_label"), field(row, "sample_count").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0), field(row, "created_at")])?; }
    for row in &archive.speaker_embeddings { exec(&connection, "INSERT OR IGNORE INTO speaker_embeddings (profile_id, model, vector_json, sample_count) VALUES (?1, ?2, ?3, COALESCE(?4, 0))", params![field(row, "profile_id"), field(row, "model").unwrap_or_default(), field(row, "vector_json").unwrap_or_default(), field(row, "sample_count").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0)])?; }
    for row in &archive.agents { exec(&connection, "INSERT OR IGNORE INTO agents (id, workspace_id, name, prompt, schedule, permission_mode, status, last_run_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![field(row, "id"), field(row, "workspace_id"), field(row, "name"), field(row, "prompt"), field(row, "schedule"), field(row, "permission_mode").unwrap_or_else(|| "approval_required".into()), field(row, "status").unwrap_or_else(|| "paused".into()), field(row, "last_run_at")])?; }
    for row in &archive.agent_runs { exec(&connection, "INSERT OR IGNORE INTO agent_runs (id, agent_id, status, output, error, retry_count, started_at, completed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE(?7, CURRENT_TIMESTAMP), ?8)", params![field(row, "id"), field(row, "agent_id"), field(row, "status").unwrap_or_else(|| "completed".into()), field(row, "output"), field(row, "error"), field(row, "retry_count").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0), field(row, "started_at"), field(row, "completed_at")])?; }
    for row in &archive.notes { exec(&connection, "INSERT OR IGNORE INTO notes (id, workspace_id, title, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, COALESCE(?5, CURRENT_TIMESTAMP), COALESCE(?6, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "title").unwrap_or_default(), field(row, "content").unwrap_or_default(), field(row, "created_at"), field(row, "updated_at")])?; }
    for row in &archive.calendar_events { exec(&connection, "INSERT OR IGNORE INTO calendar_events (id, workspace_id, title, starts_at, ends_at, meeting_url, recurrence, provider, external_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, 'local'), ?9, COALESCE(?10, CURRENT_TIMESTAMP), ?11)", params![field(row, "id"), field(row, "workspace_id"), field(row, "title"), field(row, "starts_at"), field(row, "ends_at"), field(row, "meeting_url"), field(row, "recurrence"), field(row, "provider"), field(row, "external_id"), field(row, "created_at"), field(row, "updated_at")])?; }
    for row in &archive.calendar_participants { exec(&connection, "INSERT OR IGNORE INTO calendar_participants (id, event_id, name, email, status) VALUES (?1, ?2, ?3, ?4, ?5)", params![field(row, "id"), field(row, "event_id"), field(row, "name").unwrap_or_default(), field(row, "email"), field(row, "status").unwrap_or_else(|| "invited".into())])?; }
    for row in &archive.model_routes { exec(&connection, "INSERT OR IGNORE INTO model_routes (workspace_id, capability, provider, model) VALUES (?1, ?2, ?3, ?4)", params![field(row, "workspace_id"), field(row, "capability"), field(row, "provider").unwrap_or_default(), field(row, "model").unwrap_or_default()])?; }
    for row in &archive.memories { exec(&connection, "INSERT OR IGNORE INTO memories (id, workspace_id, content, source_kind, source_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, CURRENT_TIMESTAMP), COALESCE(?7, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "content").unwrap_or_default(), field(row, "source_kind"), field(row, "source_id"), field(row, "created_at"), field(row, "updated_at")])?; }
    for row in &archive.web_pages { exec(&connection, "INSERT OR IGNORE INTO web_pages (id, workspace_id, url, title, fetched_at) VALUES (?1, ?2, ?3, ?4, COALESCE(?5, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "url").unwrap_or_default(), field(row, "title").unwrap_or_default(), field(row, "fetched_at")])?; }
    for row in &archive.bot_runs { exec(&connection, "INSERT OR IGNORE INTO bot_runs (id, workspace_id, bot_id, status, message, meeting_id, platform, reported_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, CURRENT_TIMESTAMP))", params![field(row, "id"), field(row, "workspace_id"), field(row, "bot_id").unwrap_or_default(), field(row, "status").unwrap_or_default(), field(row, "message"), field(row, "meeting_id"), field(row, "platform").unwrap_or_else(|| "teams".into()), field(row, "reported_at")])?; }
    for row in &archive.agent_permissions { exec(&connection, "INSERT OR IGNORE INTO agent_permissions (agent_id, workspace_id, capability, granted) VALUES (?1, ?2, ?3, ?4)", params![field(row, "agent_id"), field(row, "workspace_id"), field(row, "capability"), field(row, "granted").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0)])?; }
    for row in &archive.workspace_allowlists { exec(&connection, "INSERT OR IGNORE INTO workspace_allowlists (workspace_id, capability) VALUES (?1, ?2)", params![field(row, "workspace_id"), field(row, "capability")])?; }
    // Record inserted counts for the summary (INSERT OR IGNORE reports only new rows).
    for (domain, rows) in [
        ("workspaces", &archive.workspaces), ("sources", &archive.sources), ("source_chunks", &archive.source_chunks),
        ("meetings", &archive.meetings), ("transcript_segments", &archive.transcript_segments), ("meeting_summaries", &archive.meeting_summaries), ("meeting_actions", &archive.meeting_actions), ("transcription_jobs", &archive.transcription_jobs),
        ("tasks", &archive.tasks), ("reminders", &archive.reminders), ("email_accounts", &archive.email_accounts), ("emails", &archive.emails),
        ("embeddings", &archive.embeddings), ("voice_profiles", &archive.voice_profiles), ("speaker_embeddings", &archive.speaker_embeddings),
        ("agents", &archive.agents), ("agent_runs", &archive.agent_runs), ("notes", &archive.notes),
        ("calendar_events", &archive.calendar_events), ("calendar_participants", &archive.calendar_participants),
        ("model_routes", &archive.model_routes), ("memories", &archive.memories), ("web_pages", &archive.web_pages),
        ("bot_runs", &archive.bot_runs), ("agent_permissions", &archive.agent_permissions), ("workspace_allowlists", &archive.workspace_allowlists),
    ] {
        restored.insert(domain.to_string(), rows.len());
    }
    Ok(restored)
}

fn field(row: &serde_json::Value, key: &str) -> Option<String> {
    row.get(key).and_then(|value| match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

#[tauri::command]
fn set_model_route(app: tauri::AppHandle, workspace_id: String, capability: String, provider: String, model: String) -> Result<ModelRouteRecord, String> {
    if capability.trim().is_empty() || provider.trim().is_empty() || model.trim().is_empty() { return Err("Capability, provider, and model are required".into()); }
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO model_routes (workspace_id, capability, provider, model) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(workspace_id, capability) DO UPDATE SET provider = excluded.provider, model = excluded.model", params![workspace_id, capability, provider, model]).map_err(|error| error.to_string())?;
    Ok(ModelRouteRecord { workspace_id, capability, provider, model })
}

/// Derive the Ollama root URL from NEURAL_OLLAMA_URL (which may point at
/// /api/generate or another /api/* path) so /api/pull can be appended.
pub(crate) fn ollama_base_url() -> String {
    let raw = std::env::var("NEURAL_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/generate".into());
    let trimmed = raw.trim_end_matches('/');
    // Match "/api" or "/api/..." (the common NEURAL_OLLAMA_URL suffix).
    let base = trimmed
        .find("/api")
        .map(|i| trimmed[..i].to_string())
        .unwrap_or_else(|| trimmed.to_string());
    if base.is_empty() { "http://127.0.0.1:11434".into() } else { base }
}

/// Locate the Hugging Face CLI (`hf` or the legacy `huggingface-cli`),
/// preferring NEURAL_HF_BIN, then well-known install locations, then PATH.
/// No machine-specific paths are assumed — the standard `huggingface_hub`
/// CLI is all that is required, on any OS.
fn resolve_hf_cli() -> Result<String, String> {
    for name in ["hf", "huggingface-cli"] {
        if let Ok(found) = resolve_binary(name, "NEURAL_HF_BIN") { return Ok(found); }
    }
    Err("Hugging Face CLI not found. Install it (pip install -U 'huggingface_hub[cli]') or set NEURAL_HF_BIN.".into())
}

/// Download a model into the local runtime's model store:
/// - runtime "ollama" pulls into the Ollama server (Ollama must be running).
/// - runtime "huggingface" downloads via the HF CLI into the HF cache
///   (respects NEURAL_HF_HOME / HF_HOME, otherwise the standard cache).
#[tauri::command]
fn download_model(runtime: String, model: String) -> Result<String, String> {
    log::info!("download_model: runtime={runtime} model={model}");
    let model = model.trim().to_string();
    if model.is_empty() { log::warn!("download_model: empty model name"); return Err("Enter a model name to download".into()); }
    match runtime.to_lowercase().as_str() {
        "ollama" => {
            let base = ollama_base_url();
            log::info!("download_model: pulling {model} from Ollama at {base}");
            let response = reqwest::blocking::Client::new()
                .post(format!("{base}/api/pull"))
                .json(&serde_json::json!({ "name": model, "stream": false }))
                .send()
                .map_err(|e| { log::error!("download_model: Ollama unreachable at {base}: {e}"); format!("Ollama is unreachable at {base}: {e}. Start Ollama first.") })?;
            let body: serde_json::Value = response.json().map_err(|e| { log::error!("download_model: bad Ollama response: {e}"); format!("Unexpected response from Ollama: {e}") })?;
            if let Some(error) = body["error"].as_str() {
                log::error!("download_model: Ollama pull failed for {model}: {error}");
                return Err(format!("Ollama pull failed: {error}"));
            }
            let status = body["status"].as_str().unwrap_or("success");
            log::info!("download_model: Ollama {status} for {model}");
            Ok(format!("Ollama {status}: {model}"))
        }
        "huggingface" => {
            let hf = match resolve_hf_cli() {
                Ok(cli) => cli,
                Err(error) => { log::error!("download_model: HF CLI unavailable: {error}"); return Err(error); }
            };
            log::info!("download_model: downloading {model} via {hf}");
            let mut command = std::process::Command::new(&hf);
            command.arg("download").arg(&model).env("HF_HUB_DISABLE_PROGRESS_BARS", "1");
            // Optional app-level cache override; otherwise the HF CLI uses its
            // standard resolution (HF_HOME env or the default cache dir).
            if let Ok(hf_home) = std::env::var("NEURAL_HF_HOME") {
                if !hf_home.trim().is_empty() {
                    command.env("HF_HOME", hf_home.trim())
                           .env("HF_HUB_CACHE", format!("{}/hub", hf_home.trim_end_matches('/')));
                }
            }
            let output = command.output().map_err(|e| { log::error!("download_model: could not run {hf}: {e}"); format!("Could not run {hf}: {e}") })?;
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let last = stdout.lines().rev().find(|line| !line.trim().is_empty())
                    .unwrap_or("download complete");
                log::info!("download_model: HF download ok for {model}: {last}");
                Ok(format!("Hugging Face: {last}"))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                log::error!("download_model: HF download failed for {model}: {}", stderr);
                Err(if stderr.is_empty() { format!("Hugging Face download failed for {model}") } else { stderr })
            }
        }
        other => { log::error!("download_model: unsupported runtime {other}"); Err(format!("Unsupported download runtime: {other}")) }
    }
}

// ── Notes ────────────────────────────────────────────────────────────────────

/// Insert a note and its FTS5 search row (shared by `create_note` and unit
/// tests so the SQL contract is verified against the real schema).
fn insert_note(connection: &Connection, id: &str, workspace_id: &str, title: &str, content: &str, now: &str) -> Result<(), String> {
    connection.execute("INSERT INTO notes (id, workspace_id, title, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![id, workspace_id, title, content, now]).map_err(|error| error.to_string())?;
    connection.execute("INSERT INTO note_search (title, content, note_id, workspace_id) VALUES (?1, ?2, ?3, ?4)", params![title, content, id, workspace_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn create_note(app: tauri::AppHandle, workspace_id: String, title: String, content: String) -> Result<NoteRecord, String> {
    if title.trim().is_empty() { return Err("Note title is required".into()); }
    let id = format!("note-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    let now = Utc::now().to_rfc3339();
    insert_note(&connection, &id, &workspace_id, title.trim(), &content, &now)?;
    Ok(NoteRecord { id, workspace_id, title: title.trim().into(), content, created_at: now.clone(), updated_at: now })
}

#[tauri::command]
fn update_note(app: tauri::AppHandle, note_id: String, title: String, content: String) -> Result<NoteRecord, String> {
    let connection = open_database(&app)?;
    let now = Utc::now().to_rfc3339();
    connection.execute("UPDATE notes SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4", params![title.trim(), content, now, note_id]).map_err(|error| error.to_string())?;
    connection.execute("UPDATE note_search SET title = ?1, content = ?2 WHERE note_id = ?3", params![title.trim(), content, note_id]).map_err(|error| error.to_string())?;
    let (workspace_id, created_at): (String, String) = connection.query_row("SELECT workspace_id, created_at FROM notes WHERE id = ?1", params![note_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|error| error.to_string())?;
    Ok(NoteRecord { id: note_id, workspace_id, title: title.trim().into(), content, created_at, updated_at: now })
}

#[tauri::command]
fn list_notes(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<NoteRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, title, content, created_at, updated_at FROM notes WHERE workspace_id = ?1 ORDER BY updated_at DESC LIMIT 100").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(NoteRecord { id: row.get(0)?, workspace_id: row.get(1)?, title: row.get(2)?, content: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_note(app: tauri::AppHandle, note_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    connection.execute("DELETE FROM note_search WHERE note_id = ?1", params![note_id]).map_err(|error| error.to_string())?;
    connection.execute("DELETE FROM notes WHERE id = ?1", params![note_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn search_notes(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<NoteRecord>, String> {
    if query.trim().is_empty() { return Ok(Vec::new()); }
    let connection = open_database(&app)?;
    let match_query = fts5_match_terms(&query);
    let mut statement = connection.prepare("SELECT n.id, n.workspace_id, n.title, n.content, n.created_at, n.updated_at FROM notes n JOIN note_search ns ON ns.note_id = n.id WHERE ns.workspace_id = ?1 AND note_search MATCH ?2 ORDER BY rank LIMIT 30").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id, match_query], |row| Ok(NoteRecord { id: row.get(0)?, workspace_id: row.get(1)?, title: row.get(2)?, content: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

// ── Web page import ─────────────────────────────────────────────────────────

#[tauri::command]
fn import_webpage(app: tauri::AppHandle, workspace_id: String, url: String) -> Result<WebPageRecord, String> {
    if url.trim().is_empty() { return Err("URL is required".into()); }
    let parsed = url.trim();
    let response = reqwest::blocking::get(parsed).map_err(|error| format!("Unable to fetch URL: {error}"))?.error_for_status().map_err(|error| error.to_string())?;
    let html = response.text().map_err(|error| error.to_string())?;
    let title = extract_html_title(&html).unwrap_or_else(|| parsed.into());
    let text = extract_html_text(&html);
    let connection = open_database(&app)?;
    let page_id = format!("webpage-{}", uuid::Uuid::new_v4());
    connection.execute("INSERT INTO web_pages (id, workspace_id, url, title) VALUES (?1, ?2, ?3, ?4)", params![page_id, workspace_id, parsed, title]).map_err(|error| error.to_string())?;
    if !text.trim().is_empty() {
        let source_id = format!("source-{}", uuid::Uuid::new_v4());
        connection.execute("INSERT INTO sources (id, workspace_id, kind, title, uri, status) VALUES (?1, ?2, 'webpage', ?3, ?4, 'pending')", params![source_id, workspace_id, title, parsed]).map_err(|error| error.to_string())?;
        let chunks: Vec<&str> = text.as_bytes().chunks(1800).filter_map(|bytes| std::str::from_utf8(bytes).ok()).map(str::trim).filter(|chunk| !chunk.is_empty()).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let chunk_id = format!("chunk-{}", uuid::Uuid::new_v4());
            connection.execute("INSERT INTO source_chunks (id, source_id, chunk_index, content) VALUES (?1, ?2, ?3, ?4)", params![chunk_id, source_id, index, chunk]).map_err(|error| error.to_string())?;
            connection.execute("INSERT INTO source_search (content, source_id, chunk_id) VALUES (?1, ?2, ?3)", params![chunk, source_id, chunk_id]).map_err(|error| error.to_string())?;
        }
        connection.execute("UPDATE sources SET status = 'indexed' WHERE id = ?1", params![source_id]).map_err(|error| error.to_string())?;
    }
    connection.execute("UPDATE workspaces SET source_count = (SELECT COUNT(*) FROM sources WHERE workspace_id = ?1) WHERE id = ?1", params![workspace_id]).map_err(|error| error.to_string())?;
    Ok(WebPageRecord { id: page_id, workspace_id, url: parsed.into(), title, fetched_at: Utc::now().to_rfc3339() })
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + 7;
    let end = lower[start..].find("</title>")?;
    Some(html[start..start + end].trim().to_string())
}

fn extract_html_text(html: &str) -> String {
    let mut text = html.to_string();
    loop { let Some(start) = text.find("<script") else { break }; let Some(end) = text[start..].find("</script>") else { break }; text.replace_range(start..start + end + 9, " "); }
    loop { let Some(start) = text.find("<style") else { break }; let Some(end) = text[start..].find("</style>") else { break }; text.replace_range(start..start + end + 8, " "); }
    let mut cleaned = String::new();
    let mut inside = false;
    for ch in text.chars() {
        if ch == '<' { inside = true; cleaned.push(' '); }
        else if ch == '>' { inside = false; }
        else if !inside { cleaned.push(ch); }
    }
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Email send ──────────────────────────────────────────────────────────────

#[tauri::command]
fn send_email(app: tauri::AppHandle, account_id: String, to: String, subject: String, body: String, agent_id: Option<String>) -> Result<EmailSendResult, String> {
    if to.trim().is_empty() || subject.trim().is_empty() { return Err("Recipient and subject are required".into()); }
    let connection = open_database(&app)?;
    // Get account info once
    let (provider, ws_id): (String, String) = connection.query_row(
        "SELECT provider, workspace_id FROM email_accounts WHERE id = ?1",
        params![account_id],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).map_err(|error| error.to_string())?;
    // Approval enforcement: if an agent is sending, check permissions and approval
    if let Some(ref agent) = agent_id {
        let perm = permissions::check_permission(&connection, agent, &ws_id, "send_email");
        if !perm.allowed {
            return Err(format!("Permission denied: {} — {}", agent, perm.reason));
        }
        let approval = approvals::check_and_request_approval(
            &connection, agent, &ws_id, "send_email",
            &format!("Send email to {}: {}", to, subject),
            &serde_json::json!({"to": to, "subject": subject, "body_preview": &body[..body.len().min(200)]}),
        )?;
        if let Some(pending) = approval {
            return Err(format!("Approval required. Request ID: {}. Approve in the Approvals view.", pending.id));
        }
    }
    // Enqueue sync for non-IMAP providers
    if provider != "imap" {
        let _ = sync::enqueue_sync(&connection, "email_send", &account_id, &provider, "send",
            &serde_json::json!({"to": to, "subject": subject, "body": body}));
    }
    // Audit log for outgoing email
    let _ = retention::record_audit(&connection, "email_sent", Some(&provider), Some("email"), Some(&ws_id),
        &format!("Sent email to {to}: {subject}"));
    smtp_send(&connection, &account_id, &to, &subject, &body, None)
}

/// Send an email over SMTP (TLS on 465) with an optional MIME attachment —
/// used for calendar invitations (.ics) and plain emails.
pub(crate) fn smtp_send(
    connection: &Connection,
    account_id: &str,
    to: &str,
    subject: &str,
    body: &str,
    attachment: Option<(&str, &str)>, // (filename, content)
) -> Result<EmailSendResult, String> {
    let (provider, smtp_host): (String, Option<String>) = connection.query_row(
        "SELECT provider, smtp_host FROM email_accounts WHERE id = ?1",
        params![account_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|error| error.to_string())?;
    let host = smtp_host.ok_or_else(|| -> String {
        match provider.as_str() { "gmail" => "smtp.gmail.com".to_string(), "outlook" => "smtp.office365.com".to_string(), _ => "SMTP host is required".to_string() }
    })?;
    let username = keyring::Entry::new("neural-agent-os/email", &format!("{account_id}:username")).map_err(|error| error.to_string())?.get_password().map_err(|error| error.to_string())?;
    let password = keyring::Entry::new("neural-agent-os/email", &format!("{account_id}:password")).map_err(|error| error.to_string())?.get_password().map_err(|error| error.to_string())?;
    let auth = base64_encode(&format!("\x00{username}\x00{password}"));
    let boundary = "neural-attachment-boundary";
    let message = match attachment {
        Some((filename, content)) => format!(
            "EHLO neural-agent-os\r\nAUTH PLAIN {auth}\r\nMAIL FROM:<{username}>\r\nRCPT TO:<{to}>\r\nDATA\r\nFrom: {username}\r\nTo: {to}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n--{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n--{boundary}\r\nContent-Type: text/calendar; method=REQUEST; charset=utf-8\r\nContent-Disposition: attachment; filename=\"{filename}\"\r\n\r\n{content}\r\n--{boundary}--\r\n.\r\nQUIT\r\n"
        ),
        None => format!("EHLO neural-agent-os\r\nAUTH PLAIN {auth}\r\nMAIL FROM:<{username}>\r\nRCPT TO:<{to}>\r\nDATA\r\nFrom: {username}\r\nTo: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n.\r\nQUIT\r\n"),
    };
    let tls = native_tls::TlsConnector::builder().build().map_err(|error| error.to_string())?;
    let stream = std::net::TcpStream::connect(format!("{}:465", host)).map_err(|error| error.to_string())?;
    let mut tls_stream = native_tls::TlsConnector::connect(&tls, &host, stream).map_err(|error| error.to_string())?;
    use std::io::{Read, Write};
    tls_stream.write_all(message.as_bytes()).map_err(|error| error.to_string())?;
    let mut response = String::new();
    let _ = tls_stream.read_to_string(&mut response);
    let _ = tls_stream.shutdown();
    let sent = response.contains("250");
    Ok(EmailSendResult { sent, message: if sent { format!("Email sent to {to} via {host}") } else { format!("SMTP response: {}", response.chars().take(200).collect::<String>()) } })
}

fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((combined >> 18) & 63) as usize] as char);
        result.push(CHARS[((combined >> 12) & 63) as usize] as char);
        if chunk.len() > 1 { result.push(CHARS[((combined >> 6) & 63) as usize] as char); } else { result.push('='); }
        if chunk.len() > 2 { result.push(CHARS[(combined & 63) as usize] as char); } else { result.push('='); }
    }
    result
}

// ── Voice profiles ──────────────────────────────────────────────────────────

#[tauri::command]
fn create_voice_profile(app: tauri::AppHandle, workspace_id: String, speaker_label: String) -> Result<VoiceProfileRecord, String> {
    if speaker_label.trim().is_empty() { return Err("Speaker label is required".into()); }
    let id = format!("voice-{}", uuid::Uuid::new_v4());
    let connection = open_database(&app)?;
    connection.execute("INSERT INTO voice_profiles (id, workspace_id, speaker_label) VALUES (?1, ?2, ?3)", params![id, workspace_id, speaker_label.trim()]).map_err(|error| error.to_string())?;
    Ok(VoiceProfileRecord { id, workspace_id, speaker_label: speaker_label.trim().into(), sample_count: 0 })
}

#[tauri::command]
fn list_voice_profiles(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<VoiceProfileRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT id, workspace_id, speaker_label, sample_count FROM voice_profiles WHERE workspace_id = ?1 ORDER BY created_at DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(VoiceProfileRecord { id: row.get(0)?, workspace_id: row.get(1)?, speaker_label: row.get(2)?, sample_count: row.get(3)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn conversation_search(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() { return Ok(Vec::new()); }
    let connection = open_database(&app)?;
    let pattern = format!("%{}%", query.trim().replace('%', "").replace('_', ""));
    let mut statement = connection.prepare("SELECT id, content, 'assistant conversation' FROM conversations WHERE workspace_id = ?1 AND content LIKE ?2 ORDER BY created_at DESC LIMIT 20").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id, pattern], |row| Ok(SearchResult { source_id: row.get(0)?, title: "Conversation".into(), uri: "assistant".into(), chunk_id: row.get(0)?, content: row.get::<_, String>(1)?.chars().take(500).collect() })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

// ── Calendar meeting detection ──────────────────────────────────────────────

#[derive(serde::Serialize)]
struct DetectedMeeting {
    event_id: String,
    title: String,
    starts_at: String,
    ends_at: String,
    meeting_url: Option<String>,
    workspace_id: String,
}

#[tauri::command]
fn detect_upcoming_meetings(app: tauri::AppHandle, workspace_id: String, hours_ahead: Option<i64>) -> Result<Vec<DetectedMeeting>, String> {
    let connection = open_database(&app)?;
    let hours = hours_ahead.unwrap_or(24);
    let now = Utc::now();
    let cutoff = now + chrono::Duration::hours(hours);
    let mut statement = connection.prepare("SELECT id, title, starts_at, ends_at, meeting_url, workspace_id FROM calendar_events WHERE workspace_id = ?1 AND starts_at >= ?2 AND starts_at <= ?3 ORDER BY starts_at LIMIT 50").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id, now.to_rfc3339(), cutoff.to_rfc3339()], |row| Ok(DetectedMeeting { event_id: row.get(0)?, title: row.get(1)?, starts_at: row.get(2)?, ends_at: row.get(3)?, meeting_url: row.get(4)?, workspace_id: row.get(5)? })).map_err(|error| error.to_string())?;
    let mut meetings = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    // Also check workspace-agnostic for calendar connections
    let mut cross = connection.prepare("SELECT id, title, starts_at, ends_at, meeting_url, workspace_id FROM calendar_events WHERE starts_at >= ?1 AND starts_at <= ?2 AND meeting_url IS NOT NULL ORDER BY starts_at LIMIT 50").map_err(|error| error.to_string())?;
    let cross_rows = cross.query_map(params![now.to_rfc3339(), cutoff.to_rfc3339()], |row| Ok(DetectedMeeting { event_id: row.get(0)?, title: row.get(1)?, starts_at: row.get(2)?, ends_at: row.get(3)?, meeting_url: row.get(4)?, workspace_id: row.get(5)? })).map_err(|error| error.to_string())?;
    meetings.extend(cross_rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?);
    Ok(meetings)
}

// ── Voice profile learning ─────────────────────────────────────────────────

#[tauri::command]
fn link_transcript_to_profile(app: tauri::AppHandle, segment_id: String, profile_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let speaker: String = connection.query_row("SELECT speaker_label FROM voice_profiles WHERE id = ?1", params![profile_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    connection.execute("UPDATE transcript_segments SET speaker = ?1 WHERE id = ?2", params![speaker, segment_id]).map_err(|error| error.to_string())?;
    connection.execute("UPDATE voice_profiles SET sample_count = sample_count + 1 WHERE id = ?1", params![profile_id]).map_err(|error| error.to_string())?;
    Ok(())
}

// ── OAuth URL generation ────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct OAuthUrl {
    url: String,
    state: String,
}

#[tauri::command]
fn generate_oauth_url(provider: String) -> Result<OAuthUrl, String> {
    let state = uuid::Uuid::new_v4().to_string();
    let url = match provider.as_str() {
        "google" => {
            let client_id = std::env::var("NEURAL_GOOGLE_CLIENT_ID").unwrap_or_default();
            if client_id.is_empty() { return Err("NEURAL_GOOGLE_CLIENT_ID is not set".into()); }
            format!("https://accounts.google.com/o/oauth2/v2/auth?client_id={client_id}&redirect_uri=http://127.0.0.1:8787/oauth/callback&response_type=code&scope=https://www.googleapis.com/auth/calendar.readonly+https://www.googleapis.com/auth/gmail.readonly+https://www.googleapis.com/auth/gmail.send&access_type=offline&prompt=consent&state={state}")
        }
        "outlook" => {
            let client_id = std::env::var("NEURAL_MICROSOFT_CLIENT_ID").unwrap_or_default();
            if client_id.is_empty() { return Err("NEURAL_MICROSOFT_CLIENT_ID is not set".into()); }
            format!("https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id={client_id}&redirect_uri=http://127.0.0.1:8787/oauth/callback&response_type=code&scope=Calendars.Read+Mail.Read+Mail.Send+offline_access&state={state}")
        }
        _ => return Err(format!("Unsupported OAuth provider: {provider}")),
    };
    Ok(OAuthUrl { url, state })
}

#[tauri::command]
fn exchange_oauth_code(_app: tauri::AppHandle, provider: String, code: String) -> Result<String, String> {
    let (client_id, client_secret, token_url) = match provider.as_str() {
        "google" => (std::env::var("NEURAL_GOOGLE_CLIENT_ID").unwrap_or_default(), std::env::var("NEURAL_GOOGLE_CLIENT_SECRET").unwrap_or_default(), "https://oauth2.googleapis.com/token"),
        "outlook" => (std::env::var("NEURAL_MICROSOFT_CLIENT_ID").unwrap_or_default(), std::env::var("NEURAL_MICROSOFT_CLIENT_SECRET").unwrap_or_default(), "https://login.microsoftonline.com/common/oauth2/v2.0/token"),
        _ => return Err(format!("Unsupported OAuth provider: {provider}")),
    };
    if client_id.is_empty() || client_secret.is_empty() { return Err("OAuth client credentials are not configured".into()); }
    let client = reqwest::blocking::Client::new();
    let response = client.post(token_url).form(&[("client_id", client_id.as_str()), ("client_secret", client_secret.as_str()), ("code", code.as_str()), ("grant_type", "authorization_code"), ("redirect_uri", "http://127.0.0.1:8787/oauth/callback")]).send().map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.json::<serde_json::Value>().map_err(|error| error.to_string())?;
    let refresh_token = response.get("refresh_token").and_then(|value| value.as_str()).unwrap_or("").to_string();
    if refresh_token.is_empty() { return Err("No refresh token received. Ensure access_type=offline and prompt=consent are set.".into()); }
    let entry = keyring::Entry::new("neural-agent-os/oauth", &format!("{provider}:refresh_token")).map_err(|error| error.to_string())?;
    entry.set_password(&refresh_token).map_err(|error| error.to_string())?;
    Ok(format!("OAuth exchange successful for {provider}"))
}

// ── Calendar sync ──────────────────────────────────────────────────────────

#[tauri::command]
fn sync_calendar_events(app: tauri::AppHandle, connection_id: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    let (provider, workspace_id): (String, String) = connection.query_row("SELECT provider, workspace_id FROM calendar_connections WHERE id = ?1", params![connection_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|error| error.to_string())?;
    let refresh_token = keyring::Entry::new("neural-agent-os/oauth", &format!("{provider}:refresh_token")).map_err(|error| error.to_string())?.get_password().map_err(|error| error.to_string())?;
    let (client_id, client_secret, token_url) = match provider.as_str() {
        "google" => (std::env::var("NEURAL_GOOGLE_CLIENT_ID").unwrap_or_default(), std::env::var("NEURAL_GOOGLE_CLIENT_SECRET").unwrap_or_default(), "https://oauth2.googleapis.com/token"),
        "outlook" => (std::env::var("NEURAL_MICROSOFT_CLIENT_ID").unwrap_or_default(), std::env::var("NEURAL_MICROSOFT_CLIENT_SECRET").unwrap_or_default(), "https://login.microsoftonline.com/common/oauth2/v2.0/token"),
        _ => return Err("Unsupported provider".into()),
    };
    let client = reqwest::blocking::Client::new();
    let token_resp = client.post(token_url).form(&[("client_id", client_id.as_str()), ("client_secret", client_secret.as_str()), ("refresh_token", refresh_token.as_str()), ("grant_type", "refresh_token")]).send().map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.json::<serde_json::Value>().map_err(|error| error.to_string())?;
    let access_token = token_resp.get("access_token").and_then(|value| value.as_str()).ok_or("No access token in response")?.to_string();
    let now = Utc::now();
    let time_min = now.to_rfc3339();
    let time_max = (now + chrono::Duration::days(30)).to_rfc3339();
    let events = match provider.as_str() {
        "google" => {
            let resp = client.get(format!("https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={time_min}&timeMax={time_max}&maxResults=100")).bearer_auth(&access_token).send().map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.json::<serde_json::Value>().map_err(|error| error.to_string())?;
            resp.get("items").cloned().unwrap_or_default()
        }
        "outlook" => {
            let resp = client.get(format!("https://graph.microsoft.com/v1.0/me/calendarview?startDateTime={time_min}&endDateTime={time_max}&$top=100")).bearer_auth(&access_token).send().map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.json::<serde_json::Value>().map_err(|error| error.to_string())?;
            resp.get("value").cloned().unwrap_or_default()
        }
        _ => return Err("Unsupported provider".into()),
    };
    let items = events.as_array().ok_or("Unexpected API response")?;
    let mut count = 0;
    for item in items {
        let (external_id, title, starts_at, ends_at, meeting_url) = match provider.as_str() {
            "google" => (item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(), item.get("summary").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string(), item.get("start").and_then(|v| v.get("dateTime")).and_then(|v| v.as_str()).unwrap_or("").to_string(), item.get("end").and_then(|v| v.get("dateTime")).and_then(|v| v.as_str()).unwrap_or("").to_string(), item.get("hangoutLink").and_then(|v| v.as_str()).map(String::from)),
            "outlook" => (item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(), item.get("subject").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string(), item.get("start").and_then(|v| v.get("dateTime")).and_then(|v| v.as_str()).unwrap_or("").to_string(), item.get("end").and_then(|v| v.get("dateTime")).and_then(|v| v.as_str()).unwrap_or("").to_string(), item.get("onlineMeeting").and_then(|v| v.get("joinUrl")).and_then(|v| v.as_str()).map(String::from)),
            _ => continue,
        };
        if external_id.is_empty() || title.is_empty() { continue; }
        connection.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, meeting_url, provider, external_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT DO NOTHING", params![format!("event-{}", uuid::Uuid::new_v4()), workspace_id, title, starts_at, ends_at, meeting_url, provider, external_id]).map_err(|error| error.to_string())?;
        count += 1;
    }
    connection.execute("UPDATE calendar_connections SET status = 'synced', last_synced_at = CURRENT_TIMESTAMP WHERE id = ?1", params![connection_id]).map_err(|error| error.to_string())?;
    Ok(format!("Synced {count} events from {provider}"))
}

// ── Email action extraction ─────────────────────────────────────────────────
// (extract_email_actions is defined in email_complete module below)

// ── Model routes (existing) ─────────────────────────────────────────────────

#[tauri::command]
fn list_model_routes(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<ModelRouteRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare("SELECT workspace_id, capability, provider, model FROM model_routes WHERE workspace_id = ?1 ORDER BY capability").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![workspace_id], |row| Ok(ModelRouteRecord { workspace_id: row.get(0)?, capability: row.get(1)?, provider: row.get(2)?, model: row.get(3)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

// ── Cloud transcription ────────────────────────────────────────────────────

#[tauri::command]
fn cloud_transcribe(app: tauri::AppHandle, meeting_id: String, recording_path: String, provider: String, model: String, language: Option<String>) -> Result<cloud::CloudTranscriptionResult, String> {
    let connection = open_database(&app)?;
    // Enforce cost limits
    let ws_id: String = connection.query_row("SELECT workspace_id FROM meetings WHERE id = ?1", params![meeting_id], |row| row.get(0)).map_err(|e| e.to_string())?;
    if let Ok(Some(limit)) = costs::get_cost_limit(&connection, &ws_id, "transcription") {
        if costs::check_cost_limit(&connection, &ws_id, limit)? {
            return Err(format!("Transcription cost limit of {} cents exceeded. Use local transcription instead.", limit));
        }
    }
    // Enforce cloud-data sharing policy
    let allow_cloud: bool = connection.query_row("SELECT value = 'true' FROM app_settings WHERE key = 'allowCloudAudio'", [], |row| row.get(0)).unwrap_or(false);
    if !allow_cloud {
        return Err("Cloud audio processing is disabled in settings. Enable allowCloudAudio to use cloud transcription.".into());
    }
    let api_key = keyring::Entry::new("neural-agent-os/provider", &format!("{}:api_key", provider)).map_err(|e| e.to_string())?.get_password().map_err(|e| e.to_string())?;
    let result = cloud::cloud_transcribe(&connection, &meeting_id, &recording_path, &provider, &model, &api_key, &ws_id, language.as_deref().unwrap_or(""))?;
    // Record token usage for cost tracking
    let _ = costs::record_token_usage(&connection, &ws_id, "transcription", &provider, &model, 0, 1000);
    // Comprehensive cloud egress audit
    let _ = retention::record_audit(&connection, "cloud_transcription_complete", Some(&provider), Some("transcript_text"), Some(&ws_id), &format!("Transcribed meeting {meeting_id} via {provider}/{model}"));
    Ok(result)
}

// ── Provider fallback ──────────────────────────────────────────────────────

#[tauri::command]
fn get_fallback_chain(app: tauri::AppHandle, workspace_id: String, capability: String) -> Result<fallback::FallbackChain, String> {
    let connection = open_database(&app)?;
    fallback::get_fallback_chain(&connection, &workspace_id, &capability)
}

#[tauri::command]
fn set_fallback_provider(app: tauri::AppHandle, workspace_id: String, capability: String, provider: String, model: String, priority: u32) -> Result<(), String> {
    let connection = open_database(&app)?;
    fallback::set_fallback_provider(&connection, &workspace_id, &capability, &provider, &model, priority)
}

#[tauri::command]
fn clear_fallback_chain(app: tauri::AppHandle, workspace_id: String, capability: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    fallback::clear_fallback_chain(&connection, &workspace_id, &capability)
}

// ── Cost tracking ──────────────────────────────────────────────────────────

#[tauri::command]
fn get_cost_summary(app: tauri::AppHandle, workspace_id: String) -> Result<costs::CostSummary, String> {
    let connection = open_database(&app)?;
    costs::get_cost_summary(&connection, &workspace_id)
}

#[tauri::command]
fn set_cost_limit(app: tauri::AppHandle, workspace_id: String, capability: String, limit_cents: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    costs::set_cost_limit(&connection, &workspace_id, &capability, limit_cents)
}

#[tauri::command]
fn get_cost_limit(app: tauri::AppHandle, workspace_id: String, capability: String) -> Result<Option<i64>, String> {
    let connection = open_database(&app)?;
    costs::get_cost_limit(&connection, &workspace_id, &capability)
}

// ── Reranking ──────────────────────────────────────────────────────────────

#[tauri::command]
fn rerank_search(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<reranking::RerankedResult>, String> {
    let connection = open_database(&app)?;
    let raw = search_sources(app.clone(), workspace_id.clone(), query.clone())?;
    reranking::rerank_results(&connection, &workspace_id, &query, &raw, true)
}

// ── Calendar extras ────────────────────────────────────────────────────────

#[tauri::command]
fn add_participant(app: tauri::AppHandle, event_id: String, name: String, email: Option<String>) -> Result<calendar_extras::CalendarParticipant, String> {
    let connection = open_database(&app)?;
    calendar_extras::add_participant(&connection, &event_id, &name, email.as_deref())
}

#[tauri::command]
fn list_participants(app: tauri::AppHandle, event_id: String) -> Result<Vec<calendar_extras::CalendarParticipant>, String> {
    let connection = open_database(&app)?;
    calendar_extras::list_participants(&connection, &event_id)
}

#[tauri::command]
fn update_participant_status(app: tauri::AppHandle, participant_id: String, status: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    calendar_extras::update_participant_status(&connection, &participant_id, &status)
}

#[tauri::command]
fn remove_participant(app: tauri::AppHandle, participant_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    calendar_extras::remove_participant(&connection, &participant_id)
}

#[tauri::command]
fn detect_conflicts(app: tauri::AppHandle, workspace_id: String, starts_at: String, ends_at: String, exclude_event_id: Option<String>) -> Result<Vec<calendar_extras::ConflictInfo>, String> {
    let connection = open_database(&app)?;
    calendar_extras::detect_conflicts(&connection, &workspace_id, &starts_at, &ends_at, exclude_event_id.as_deref())
}

#[tauri::command]
fn check_conflicts_for_event(app: tauri::AppHandle, workspace_id: String, event_id: String) -> Result<Vec<calendar_extras::ConflictInfo>, String> {
    let connection = open_database(&app)?;
    calendar_extras::check_conflicts_for_event(&connection, &workspace_id, &event_id)
}

// ── Folder monitoring ──────────────────────────────────────────────────────

#[tauri::command]
fn add_monitored_folder(app: tauri::AppHandle, workspace_id: String, path: String) -> Result<monitoring::MonitoredFolder, String> {
    let connection = open_database(&app)?;
    monitoring::add_monitored_folder(&connection, &workspace_id, &path)
}

#[tauri::command]
fn list_monitored_folders(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<monitoring::MonitoredFolder>, String> {
    let connection = open_database(&app)?;
    monitoring::list_monitored_folders(&connection, &workspace_id)
}

#[tauri::command]
fn toggle_monitored_folder(app: tauri::AppHandle, folder_id: String, enabled: bool) -> Result<(), String> {
    let connection = open_database(&app)?;
    monitoring::toggle_monitored_folder(&connection, &folder_id, enabled)
}

#[tauri::command]
fn remove_monitored_folder(app: tauri::AppHandle, folder_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    monitoring::remove_monitored_folder(&connection, &folder_id)
}

// ── Meeting preparation ────────────────────────────────────────────────────

#[tauri::command]
fn prepare_meeting(app: tauri::AppHandle, event_id: String) -> Result<meeting_prep::MeetingPrep, String> {
    log::info!("prepare_meeting: event={event_id}");
    let connection = open_database(&app)?;
    meeting_prep::prepare_meeting(&app, &connection, &event_id)
}

// ── Diarization ────────────────────────────────────────────────────────────

#[tauri::command]
fn diarize_transcript(app: tauri::AppHandle, meeting_id: String, workspace_id: String, embedding_model: Option<String>, similarity_threshold: Option<f32>) -> Result<diarization::DiarizationResult, String> {
    let connection = open_database(&app)?;
    let model = embedding_model.unwrap_or_else(|| "nomic-embed-text".into());
    let threshold = similarity_threshold.unwrap_or(0.7);
    diarization::diarize_transcript(&connection, &meeting_id, &workspace_id, &model, threshold)
}

#[tauri::command]
fn build_speaker_embedding(app: tauri::AppHandle, profile_id: String, model: Option<String>) -> Result<diarization::SpeakerEmbedding, String> {
    let connection = open_database(&app)?;
    let model = model.unwrap_or_else(|| "nomic-embed-text".into());
    diarization::build_speaker_embedding(&connection, &profile_id, &model)
}

// ── Job queue ──────────────────────────────────────────────────────────────

#[tauri::command]
fn enqueue_job(app: tauri::AppHandle, job_type: String, workspace_id: String, payload_json: String, max_retries: Option<i32>) -> Result<job_queue::BackgroundJob, String> {
    let connection = open_database(&app)?;
    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|e| e.to_string())?;
    job_queue::enqueue_job(&connection, &job_type, &workspace_id, &payload, max_retries.unwrap_or(3))
}

#[tauri::command]
fn list_jobs(app: tauri::AppHandle, workspace_id: String, status_filter: Option<String>, limit: Option<i64>) -> Result<Vec<job_queue::BackgroundJob>, String> {
    let connection = open_database(&app)?;
    job_queue::list_jobs(&connection, &workspace_id, status_filter.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
fn cancel_job(app: tauri::AppHandle, job_id: String) -> Result<bool, String> {
    let connection = open_database(&app)?;
    job_queue::cancel_job(&connection, &job_id)
}

#[tauri::command]
fn get_queue_status(app: tauri::AppHandle, workspace_id: String) -> Result<job_queue::JobQueueStatus, String> {
    let connection = open_database(&app)?;
    job_queue::get_queue_status(&connection, &workspace_id)
}

// ── Permissions ────────────────────────────────────────────────────────────

#[tauri::command]
fn check_permission(app: tauri::AppHandle, agent_id: String, workspace_id: String, capability: String) -> Result<permissions::PermissionCheck, String> {
    let connection = open_database(&app)?;
    Ok(permissions::check_permission(&connection, &agent_id, &workspace_id, &capability))
}

#[tauri::command]
fn check_all_permissions(app: tauri::AppHandle, agent_id: String, workspace_id: String) -> Result<Vec<permissions::PermissionCheck>, String> {
    let connection = open_database(&app)?;
    permissions::check_all_permissions(&connection, &agent_id, &workspace_id)
}

#[tauri::command]
fn list_agent_permissions(app: tauri::AppHandle, agent_id: String, workspace_id: String) -> Result<Vec<permissions::AgentPermission>, String> {
    let connection = open_database(&app)?;
    permissions::list_agent_permissions(&connection, &agent_id, &workspace_id)
}

#[tauri::command]
fn grant_permission(app: tauri::AppHandle, agent_id: String, workspace_id: String, capability: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    permissions::grant_permission(&connection, &agent_id, &workspace_id, &capability)
}

#[tauri::command]
fn revoke_permission(app: tauri::AppHandle, agent_id: String, workspace_id: String, capability: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    permissions::revoke_permission(&connection, &agent_id, &workspace_id, &capability)
}

#[tauri::command]
fn set_workspace_allowlist(app: tauri::AppHandle, workspace_id: String, capabilities: Vec<String>) -> Result<(), String> {
    let connection = open_database(&app)?;
    permissions::set_workspace_allowlist(&connection, &workspace_id, &capabilities)
}

#[tauri::command]
fn get_workspace_allowlist(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<String>, String> {
    let connection = open_database(&app)?;
    permissions::get_workspace_allowlist(&connection, &workspace_id)
}

// ── Meeting bot (Playwright browser automation) ─────────────────────────────

#[tauri::command]
fn launch_meeting_bot(platform: String, meeting_url: String, display_name: Option<String>, duration_minutes: Option<i64>) -> Result<String, String> {
    let name = display_name.unwrap_or_else(|| "Neural Assistant".into());
    let duration = duration_minutes.unwrap_or(60);
    let bot_script = std::env::current_dir()
        .unwrap_or_default()
        .join("teams-bot")
        .join("meeting_bot.py");

    let python = resolve_binary("python3", "NEURAL_PYTHON_BIN").unwrap_or_else(|_| "python3".into());
    let output = std::process::Command::new(&python)
        .arg(bot_script.to_string_lossy().as_ref())
        .arg(&platform)
        .arg(&meeting_url)
        .arg(&name)
        .arg(duration.to_string())
        .output()
        .map_err(|e| format!("Failed to launch meeting bot: {e}. Install: pip install playwright && playwright install chromium"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// ── Retention & diagnostics ────────────────────────────────────────────────

#[tauri::command]
fn get_storage_stats(app: tauri::AppHandle) -> Result<retention::StorageStats, String> {
    retention::get_storage_stats(&app)
}

#[tauri::command]
fn cleanup_old_recordings(app: tauri::AppHandle, older_than_days: i64, dry_run: Option<bool>) -> Result<Vec<String>, String> {
    retention::cleanup_old_recordings(&app, older_than_days, dry_run.unwrap_or(true))
}

#[tauri::command]
fn get_audit_log(app: tauri::AppHandle, workspace_id: Option<String>, limit: Option<i64>) -> Result<Vec<retention::AuditEntry>, String> {
    let connection = open_database(&app)?;
    retention::get_audit_log(&connection, workspace_id.as_deref(), limit.unwrap_or(50))
}


// ── Sync queue ─────────────────────────────────────────────────────────────

#[tauri::command]
fn enqueue_sync(app: tauri::AppHandle, entity_type: String, entity_id: String, provider: String, action: String, payload_json: String) -> Result<sync::SyncQueueItem, String> {
    let connection = open_database(&app)?;
    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|e| e.to_string())?;
    sync::enqueue_sync(&connection, &entity_type, &entity_id, &provider, &action, &payload)
}

#[tauri::command]
fn get_sync_status(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<sync::SyncStatus>, String> {
    let connection = open_database(&app)?;
    sync::get_sync_status(&connection, &workspace_id)
}

#[tauri::command]
fn resolve_sync_conflict(app: tauri::AppHandle, queue_id: String, resolution: String) -> Result<sync::ConflictResolution, String> {
    let connection = open_database(&app)?;
    sync::resolve_conflict(&connection, &queue_id, &resolution)
}

// ── Approvals ──────────────────────────────────────────────────────────────

#[tauri::command]
fn request_approval(app: tauri::AppHandle, agent_id: String, workspace_id: String, capability: String, description: String, payload_json: String) -> Result<approvals::ApprovalRequest, String> {
    let connection = open_database(&app)?;
    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|e| e.to_string())?;
    approvals::request_approval(&connection, &agent_id, &workspace_id, &capability, &description, &payload)
}

#[tauri::command]
fn approve_request(app: tauri::AppHandle, request_id: String, approved_by: String) -> Result<approvals::ApprovalRequest, String> {
    let connection = open_database(&app)?;
    approvals::approve_request(&connection, &request_id, &approved_by)
}

#[tauri::command]
fn deny_request(app: tauri::AppHandle, request_id: String, denied_by: String, reason: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    approvals::deny_request(&connection, &request_id, &denied_by, &reason)
}

#[tauri::command]
fn list_pending_approvals(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<approvals::ApprovalRequest>, String> {
    let connection = open_database(&app)?;
    approvals::list_pending_approvals(&connection, &workspace_id)
}

// ── OAuth ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_oauth_config(provider: String) -> Result<oauth::OAuthConfig, String> {
    oauth::get_oauth_config(&provider)
}

#[tauri::command]
fn generate_pkce() -> Result<oauth::PKCEParams, String> {
    Ok(oauth::generate_pkce())
}

#[tauri::command]
fn build_authorize_url(provider: String, pkce: oauth::PKCEParams) -> Result<String, String> {
    oauth::build_authorize_url(&provider, &pkce)
}

#[tauri::command]
fn exchange_code_with_pkce(provider: String, code: String, pkce: oauth::PKCEParams) -> Result<(String, String), String> {
    oauth::exchange_code_with_pkce(&provider, &code, &pkce)
}

#[tauri::command]
fn refresh_oauth_token(provider: String) -> Result<String, String> {
    oauth::refresh_access_token(&provider)
}

// ── Deletion preview ───────────────────────────────────────────────────────

#[tauri::command]
fn preview_source_deletion(app: tauri::AppHandle, source_id: String) -> Result<deletion::SourceDeletionPreview, String> {
    let connection = open_database(&app)?;
    deletion::preview_source_deletion(&connection, &source_id)
}

#[tauri::command]
fn preview_meeting_deletion(app: tauri::AppHandle, meeting_id: String) -> Result<deletion::MeetingDeletionPreview, String> {
    let connection = open_database(&app)?;
    deletion::preview_meeting_deletion(&connection, &meeting_id)
}

#[tauri::command]
fn full_deletion_preview(app: tauri::AppHandle, workspace_id: String) -> Result<deletion::FullDeletionPreview, String> {
    let connection = open_database(&app)?;
    deletion::full_deletion_preview(&connection, &workspace_id)
}
// ── Calendar complete ──────────────────────────────────────────────────────
#[tauri::command]
fn expand_recurring_event(app: tauri::AppHandle, event_id: String, from_date: String, to_date: String) -> Result<Vec<calendar_complete::ExpandedEvent>, String> {
    let connection = open_database(&app)?;
    calendar_complete::expand_recurring_event(&connection, &event_id, &from_date, &to_date)
}

#[tauri::command]
fn common_timezones() -> Result<Vec<calendar_complete::TimeZoneInfo>, String> {
    Ok(calendar_complete::common_timezones())
}

#[tauri::command]
fn evaluate_recording_rules(app: tauri::AppHandle, workspace_id: String, event_title: String, meeting_url: Option<String>, participants: Vec<String>) -> Result<String, String> {
    let connection = open_database(&app)?;
    calendar_complete::evaluate_recording_rules(&connection, &workspace_id, &event_title, meeting_url.as_deref(), &participants)
}

#[tauri::command]
fn set_recording_rule(app: tauri::AppHandle, workspace_id: String, rule_type: String, pattern: String, action: String, priority: i32) -> Result<(), String> {
    let connection = open_database(&app)?;
    calendar_complete::set_recording_rule(&connection, &workspace_id, &rule_type, &pattern, &action, priority)
}

#[tauri::command]
fn list_recording_rules(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<calendar_complete::RecordingRule>, String> {
    let connection = open_database(&app)?;
    calendar_complete::list_recording_rules(&connection, &workspace_id)
}

#[tauri::command]
fn delete_recording_rule(app: tauri::AppHandle, rule_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    calendar_complete::delete_recording_rule(&connection, &rule_id)
}

#[tauri::command]
fn convert_timezone(dt_str: String, from_offset_hours: f64, to_offset_hours: f64) -> Result<String, String> {
    calendar_complete::convert_timezone(&dt_str, from_offset_hours, to_offset_hours)
}

// ── Email complete ─────────────────────────────────────────────────────────
#[tauri::command]
fn rebuild_threads(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<email_complete::EmailThread>, String> {
    let connection = open_database(&app)?;
    email_complete::rebuild_threads(&connection, &workspace_id)
}

#[tauri::command]
fn create_label(app: tauri::AppHandle, account_id: String, name: String, color: Option<String>) -> Result<email_complete::EmailLabel, String> {
    let connection = open_database(&app)?;
    email_complete::create_label(&connection, &account_id, &name, color.as_deref())
}

#[tauri::command]
fn list_labels(app: tauri::AppHandle, account_id: String) -> Result<Vec<email_complete::EmailLabel>, String> {
    let connection = open_database(&app)?;
    email_complete::list_labels(&connection, &account_id)
}

#[tauri::command]
fn label_email(app: tauri::AppHandle, email_id: String, label_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    email_complete::label_email(&connection, &email_id, &label_id)
}

#[tauri::command]
fn list_labeled_emails(app: tauri::AppHandle, label_id: String) -> Result<Vec<String>, String> {
    let connection = open_database(&app)?;
    email_complete::get_labeled_emails(&connection, &label_id)
}

#[tauri::command]
fn index_attachment(app: tauri::AppHandle, email_id: String, filename: String, content_type: String, size_bytes: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    email_complete::index_attachment(&connection, &email_id, &filename, &content_type, size_bytes)
}

#[tauri::command]
fn generate_email_draft(app: tauri::AppHandle, workspace_id: String, instructions: String, context: String, model: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "chat", &model);
    email_complete::generate_email_draft(&connection, &workspace_id, &instructions, &context, &model)
}

#[tauri::command]
fn summarize_email_thread(app: tauri::AppHandle, thread_id: String, workspace_id: String, model: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "summarization", &model);
    email_complete::summarize_email_thread(&connection, &thread_id, &workspace_id, &model)
}

#[tauri::command]
fn extract_email_actions(app: tauri::AppHandle, email_id: String, workspace_id: String, model: String) -> Result<Vec<String>, String> {
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "chat", &model);
    email_complete::extract_email_actions(&connection, &email_id, &workspace_id, &model)
}

#[tauri::command]
fn extract_attachment_text(path: String, filename: String) -> Result<String, String> {
    email_complete::extract_attachment_text(&path, &filename)
}

#[tauri::command]
fn suggest_workspace_for_email(app: tauri::AppHandle, email_id: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    email_complete::suggest_workspace_for_email(&connection, &email_id)
}

// ── Agent activity visualization ──────────────────────────────────────────

#[derive(serde::Serialize)]
struct AgentActivitySummary {
    active_agents: usize,
    pending_approvals: usize,
    recent_runs: Vec<serde_json::Value>,
    next_scheduled: Vec<serde_json::Value>,
}

#[tauri::command]
fn get_agent_activity(app: tauri::AppHandle, workspace_id: String) -> Result<AgentActivitySummary, String> {
    let connection = open_database(&app)?;
    let active: usize = connection.query_row(
        "SELECT COUNT(*) FROM agents WHERE workspace_id = ?1 AND status = 'active'",
        params![workspace_id], |row| row.get(0)
    ).unwrap_or(0);
    let pending: usize = connection.query_row(
        "SELECT COUNT(*) FROM approval_requests WHERE workspace_id = ?1 AND status = 'pending'",
        params![workspace_id], |row| row.get(0)
    ).unwrap_or(0);
    let mut runs_stmt = connection.prepare(
        "SELECT ar.id, a.name, ar.status, ar.output, ar.error, ar.started_at FROM agent_runs ar JOIN agents a ON a.id = ar.agent_id WHERE a.workspace_id = ?1 ORDER BY ar.started_at DESC LIMIT 10"
    ).map_err(|e| e.to_string())?;
    let recent_runs: Vec<serde_json::Value> = runs_stmt.query_map(params![workspace_id], |row| Ok(serde_json::json!({"run_id": row.get::<_,String>(0)?, "agent_name": row.get::<_,String>(1)?, "status": row.get::<_,String>(2)?, "output": row.get::<_,Option<String>>(3)?, "error": row.get::<_,Option<String>>(4)?, "started_at": row.get::<_,String>(5)?}))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    let mut next_stmt = connection.prepare("SELECT id, name, schedule, permission_mode FROM agents WHERE workspace_id = ?1 AND status = 'active' ORDER BY name LIMIT 10").map_err(|e| e.to_string())?;
    let next_scheduled: Vec<serde_json::Value> = next_stmt.query_map(params![workspace_id], |row| Ok(serde_json::json!({"agent_id": row.get::<_,String>(0)?, "name": row.get::<_,String>(1)?, "schedule": row.get::<_,String>(2)?, "permission_mode": row.get::<_,String>(3)?}))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    Ok(AgentActivitySummary { active_agents: active, pending_approvals: pending, recent_runs, next_scheduled })
}

// ── Provider health ────────────────────────────────────────────────────────
#[tauri::command]
fn refresh_provider_health(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<health::ProviderHealth>, String> {
    let connection = open_database(&app)?;
    health::refresh_provider_health(&connection, &workspace_id)
}

#[tauri::command]
fn get_provider_health(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<health::ProviderHealth>, String> {
    let connection = open_database(&app)?;
    health::get_provider_health(&connection, &workspace_id)
}

// ── Local voice pipeline ───────────────────────────────────────────────────

#[tauri::command]
fn local_text_to_speech(text: String, language: Option<String>) -> Result<String, String> {
    let lang = language.unwrap_or_else(|| "en".into());
    // Try platform-native TTS engines. We BLOCK until speech finishes so the
    // caller knows exactly when the voice stopped (prevents the mic re-opening
    // during the tail of a reply, which caused echo loops in voice mode).
    #[cfg(target_os = "macos")]
    {
        let voice = match lang.as_str() {
            "hi" => "Lekha",
            "te" => "Chitra",
            _ => "Samantha",
        };
        let status = std::process::Command::new("say")
            .args(["-v", voice, &text])
            .status()
            .map_err(|e| format!("macOS say command unavailable: {e}"))?;
        let done = status.success();
        return Ok(format!("Speaking via macOS say ({voice}) done={done}"));
    }
    #[cfg(target_os = "linux")]
    {
        let espeak = resolve_binary("espeak-ng", "NEURAL_ESPEAK_BIN").unwrap_or_else(|_| "espeak-ng".into());
        let _status = std::process::Command::new(&espeak)
            .args(["-v", &format!("{lang}+f3"), &text])
            .status()
            .map_err(|e| format!("espeak-ng unavailable. Install with: sudo apt install espeak-ng. Error: {e}"))?;
        return Ok("Speaking via espeak-ng".into());
    }
    #[cfg(target_os = "windows")]
    {
        // Windows uses SAPI via PowerShell
        let _status = std::process::Command::new("powershell")
            .args(["-Command", &format!("Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Speak('{}')", text)])
            .status()
            .map_err(|e| format!("Windows TTS unavailable: {e}"))?;
        return Ok("Speaking via Windows SAPI".into());
    }
    #[allow(unreachable_code)]
    Err("No TTS engine available for this platform".into())
}

/// Detect the whisper CLI flavor once (cached): openai-whisper / faster-whisper
/// understand `--model/--beam_size/--fp16`; mlx-whisper and whisper.cpp do not
/// (mlx defaults to its fast `mlx-community/whisper-tiny`). Sniffing the help
/// text is more reliable than guessing from the binary name, since
/// `/opt/homebrew/bin/whisper` is often a wrapper script that execs mlx_whisper.
static WHISPER_CAPS: std::sync::OnceLock<(bool, bool)> = std::sync::OnceLock::new();
/// Returns (is_openai_like, supports_fp16_flag) for the resolved whisper CLI.
fn whisper_caps(bin: &str) -> (bool, bool) {
    *WHISPER_CAPS.get_or_init(|| {
        let help = std::process::Command::new(bin)
            .arg("--help")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let is_openai = help.contains("beam_size") || help.contains("openai") || help.contains("faster-whisper");
        let has_fp16 = help.contains("--fp16");
        (is_openai, has_fp16)
    })
}

#[tauri::command]
fn local_speech_to_text(_app: tauri::AppHandle, audio_path: String, language: Option<String>) -> Result<String, String> {
    let lang = language.unwrap_or_else(|| "en".into());
    let whisper = resolve_binary("whisper", "NEURAL_WHISPER_BIN")?;
    let lang_arg = match lang.as_str() {
        "hi" => "hi",
        "te" => "te",
        _ => "en",
    };
    let output_dir = data_subdir("transcripts");
    let mut command = std::process::Command::new(&whisper);
    command.args([&audio_path, "--language", lang_arg, "--output_format", "txt", "--output_dir", &output_dir.to_string_lossy()]);
    let (is_openai, has_fp16) = whisper_caps(&whisper);
    if is_openai {
        // openai-whisper: default `small` model takes ~60-75s per clip on CPU;
        // `base` + greedy decoding finishes in ~1s. Override the model with
        // NEURAL_WHISPER_MODEL (e.g. "tiny", "base", "small").
        let model = std::env::var("NEURAL_WHISPER_MODEL").unwrap_or_else(|_| "base".into());
        command.args(["--model", &model, "--beam_size", "1", "--fp16", "False"]);
    } else if has_fp16 {
        // mlx-whisper (Apple Silicon): its fast default model + fp16 GPU.
        command.args(["--fp16", "True"]);
    }
    let output = command.output()
        .map_err(|e| format!("Whisper unavailable: {e}. Install with: pip install openai-whisper"))?;
    if output.status.success() {
        let stem = std::path::Path::new(&audio_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let txt_path = output_dir.join(format!("{}.txt", stem));
        std::fs::read_to_string(&txt_path)
            .map_err(|e| format!("Could not read transcript: {e}"))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// ── Calendar sync pull ────────────────────────────────────────────────────

#[tauri::command]
fn pull_remote_calendar(app: tauri::AppHandle, connection_id: String) -> Result<(usize, usize, usize), String> {
    sync::pull_remote_calendar_events(&app, &connection_id)
}

// ── Cascading workspace delete ─────────────────────────────────────────────

#[tauri::command]
fn delete_workspace_complete(app: tauri::AppHandle, workspace_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if count <= 1 { return Err("The last workspace cannot be deleted".into()); }

    // Get all recording paths before deleting
    let mut stmt = connection.prepare(
        "SELECT recording_path FROM meetings WHERE workspace_id = ?1 AND recording_path IS NOT NULL"
    ).map_err(|e| e.to_string())?;
    let recordings: Vec<String> = stmt.query_map(
        params![workspace_id], |row| row.get::<_, String>(0)
    ).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Delete derived data in cascade order
    connection.execute(
        "DELETE FROM chunk_embeddings WHERE chunk_id IN (SELECT id FROM source_chunks WHERE source_id IN (SELECT id FROM sources WHERE workspace_id = ?1))",
        params![workspace_id]
    ).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM source_search WHERE source_id IN (SELECT id FROM sources WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM source_chunks WHERE source_id IN (SELECT id FROM sources WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM sources WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM transcript_segments WHERE meeting_id IN (SELECT id FROM meetings WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM meeting_actions WHERE meeting_id IN (SELECT id FROM meetings WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM meeting_summaries WHERE meeting_id IN (SELECT id FROM meetings WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM transcription_jobs WHERE meeting_id IN (SELECT id FROM meetings WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM background_jobs WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM meetings WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM token_usage WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM reminders WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM tasks WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM notes WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM agent_runs WHERE agent_id IN (SELECT id FROM agents WHERE workspace_id = ?1)", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM agents WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM model_routes WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM provider_fallback WHERE workspace_id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;
    connection.execute("DELETE FROM workspaces WHERE id = ?1", params![workspace_id]).map_err(|e| e.to_string())?;

    // Clean up recording files on disk
    for path in &recordings {
        let _ = std::fs::remove_file(path);
    }

    retention::record_audit(
        &connection, "workspace_deleted_complete", None, None, Some(&workspace_id),
        &format!("Cascading delete completed, {} recordings cleaned", recordings.len()),
    )?;

    Ok(())
}

// ── Calendar invitations & RSVP ─────────────────────────────────────────────

#[tauri::command]
fn send_calendar_invitation(app: tauri::AppHandle, event_id: String, participant_id: String) -> Result<calendar_extras::InvitationResult, String> {
    let connection = open_database(&app)?;
    calendar_extras::send_invitation(&connection, &event_id, &participant_id)
}

#[tauri::command]
fn process_rsvp_response(app: tauri::AppHandle, participant_id: String, response: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    calendar_extras::process_rsvp(&connection, &participant_id, &response)
}

#[tauri::command]
fn generate_ics(app: tauri::AppHandle, event_id: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    calendar_extras::generate_ics_content(&connection, &event_id)
}

// ── Differential backup & retention ─────────────────────────────────────────

#[tauri::command]
fn differential_backup(app: tauri::AppHandle, workspace_id: String, destination_path: String, since: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    // Resolve relative/~/ output paths against a writable location (the
    // packaged app runs from `/`, which is read-only).
    let resolved = resolve_output_path(&app, &destination_path)?;
    let resolved_str = resolved.to_string_lossy().into_owned();
    retention::differential_backup(&connection, &workspace_id, &resolved_str, &since)
}

#[tauri::command]
fn get_workspace_retention(app: tauri::AppHandle, workspace_id: String) -> Result<Option<i64>, String> {
    let connection = open_database(&app)?;
    retention::get_workspace_retention(&connection, &workspace_id)
}

#[tauri::command]
fn set_workspace_retention(app: tauri::AppHandle, workspace_id: String, retention_days: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    retention::set_workspace_retention(&connection, &workspace_id, retention_days)
}

// ── Centralized action authorization ────────────────────────────────────────

#[tauri::command]
fn delete_voice_profile(app: tauri::AppHandle, profile_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let changed = connection.execute("DELETE FROM voice_profiles WHERE id = ?1", params![profile_id]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Voice profile not found".into()); }
    Ok(())
}

#[tauri::command]
fn authorize_action(app: tauri::AppHandle, agent_id: String, workspace_id: String, capability: String, description: String, payload_json: String) -> Result<Option<approvals::ApprovalRequest>, String> {
    let connection = open_database(&app)?;
    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|e| e.to_string())?;
    permissions::authorize_action(&connection, &agent_id, &workspace_id, &capability, &description, &payload)
}

// ── API key authentication for MCP/REST ────────────────────────────────────

#[tauri::command]
fn generate_api_key(app: tauri::AppHandle, label: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    let key = format!("na-{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let hash = hash_api_key(&key);
    connection.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
        params![format!("api_key_{}", label.trim()), hash],
    ).map_err(|e| e.to_string())?;
    Ok(key)
}

#[tauri::command]
fn list_api_keys(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let connection = open_database(&app)?;
    let mut stmt = connection.prepare("SELECT key FROM app_settings WHERE key LIKE 'api_key_%'").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn revoke_api_key(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    connection.execute("DELETE FROM app_settings WHERE key = ?1", params![format!("api_key_{}", label.trim())]).map_err(|e| e.to_string())?;
    Ok(())
}

fn hash_api_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn api_key_valid(connection: &Connection, provided: &str) -> bool {
    let hash = hash_api_key(provided);
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM app_settings WHERE value = ?1 AND key LIKE 'api_key_%'",
        params![hash], |row| row.get(0),
    ).unwrap_or(0);
    count > 0
}

fn bearer_token(request: &tiny_http::Request) -> Option<String> {
    request.headers().iter().find_map(|h| {
        if h.field.equiv("Authorization") {
            let value = h.value.as_str();
            value.strip_prefix("Bearer ").map(|t| t.trim().to_string())
        } else { None }
    })
}

/// Read the Teams-bot shared-secret token from the `X-Teams-Bot-Token` header
/// and validate it. The bot and the local app share `NAO_TEAMS_BOT_SECRET`;
/// without a valid token the teams-bot endpoints reject the request.
fn teams_bot_auth(request: &tiny_http::Request) -> Result<(), String> {
    let token = request.headers().iter().find_map(|h| {
        if h.field.equiv("X-Teams-Bot-Token") {
            Some(h.value.as_str().trim().to_string())
        } else { None }
    }).ok_or("Missing X-Teams-Bot-Token header")?;
    teams_bot::validate_token(&token)
}

/// Check that a request is authenticated for write operations.
/// Returns Ok(()) if authorized, Err(message) otherwise.
fn require_api_auth(connection: &Connection, request: &tiny_http::Request) -> Result<(), String> {
    let token = bearer_token(request).ok_or("Missing Authorization: Bearer <api-key> header")?;
    if api_key_valid(connection, &token) {
        Ok(())
    } else {
        Err("Invalid API key".into())
    }
}

// ── Command/file execution with permission + approval gating ───────────────

#[derive(serde::Serialize)]
struct CommandResult {
    command: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[tauri::command]
fn execute_command(app: tauri::AppHandle, agent_id: String, workspace_id: String, command: String) -> Result<CommandResult, String> {    let connection = open_database(&app)?;
    // Permission check
    let perm = permissions::check_permission(&connection, &agent_id, &workspace_id, "execute_commands");
    if !perm.allowed {
        return Err(format!("Permission denied: execute_commands — {}", perm.reason));
    }
    // Approval required for command execution
    let approval = permissions::authorize_action(
        &connection, &agent_id, &workspace_id, "execute_commands",
        &format!("Execute command: {}", command),
        &serde_json::json!({ "command": command }),
    )?;
    if let Some(pending) = approval {
        return Err(format!("Approval required. Request ID: {}. Approve in the Approvals view.", pending.id));
    }

    // Execute the command
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .output()
        .map_err(|e| format!("Unable to execute command: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    retention::record_audit(
        &connection, "command_executed", None, None, Some(&workspace_id),
        &format!("Agent {agent_id} executed: {command} (exit {exit_code})"),
    )?;

    Ok(CommandResult { command, exit_code, stdout, stderr })
}

#[derive(serde::Serialize)]
struct FileEditResult {
    path: String,
    action: String,
    bytes_written: usize,
}

#[tauri::command]
fn modify_file(app: tauri::AppHandle, agent_id: String, workspace_id: String, path: String, content: String, append: bool) -> Result<FileEditResult, String> {
    let connection = open_database(&app)?;
    // Permission check
    let perm = permissions::check_permission(&connection, &agent_id, &workspace_id, "modify_files");
    if !perm.allowed {
        return Err(format!("Permission denied: modify_files — {}", perm.reason));
    }
    // Approval required for file modification
    let approval = permissions::authorize_action(
        &connection, &agent_id, &workspace_id, "modify_files",
        &format!("{} file: {}", if append { "Append to" } else { "Write" }, path),
        &serde_json::json!({ "path": path, "append": append, "content_preview": &content[..content.len().min(200)] }),
    )?;
    if let Some(pending) = approval {
        return Err(format!("Approval required. Request ID: {}. Approve in the Approvals view.", pending.id));
    }

    // Resolve ~/ and relative paths against the user's home directory so file
    // edits work regardless of the packaged app's working directory (/).
    let target = if let Some(rest) = path.trim().strip_prefix("~/") {
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest)
    } else {
        let p = std::path::PathBuf::from(path.trim());
        if p.is_absolute() { p } else { std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(p) }
    };
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let target_str = target.to_string_lossy().into_owned();
    let bytes_written = if append {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&target_str)
            .map_err(|e| format!("Unable to open file: {e}"))?;
        file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
        content.len()
    } else {
        std::fs::write(&target_str, &content).map_err(|e| format!("Unable to write file: {e}"))?;
        content.len()
    };

    retention::record_audit(
        &connection, "file_modified", None, None, Some(&workspace_id),
        &format!("Agent {agent_id} {} file {path} ({} bytes)", if append { "appended to" } else { "wrote" }, bytes_written),
    )?;

    Ok(FileEditResult { path, action: if append { "append".into() } else { "write".into() }, bytes_written })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(VoiceCaptureState(Mutex::new(None)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            // Resolve the unified data root: legacy marker file if present,
            // otherwise the default (~/Neural Agent OS / NEURAL_DATA_DIR).
            let root = {
                let marker = app.path().app_data_dir().ok()
                    .map(|d| d.join("data-directory.txt"))
                    .and_then(|m| std::fs::read_to_string(m).ok())
                    .map(|p| std::path::PathBuf::from(p.trim()))
                    .filter(|p| p.is_dir());
                marker.unwrap_or_else(data_root)
            };
            crate::set_data_root(root.clone());
            crate::ensure_data_layout();
            crate::migrate_legacy_layout(&handle, &root);
            // Application-wide logging: rotating file + stderr. Everything
            // from the job pipeline lands here for diagnostics.
            logging::init(&root.join("logs"));
            log::info!("Neural Agent OS starting (v{})", env!("CARGO_PKG_VERSION"));
            log::info!("data root: {}", root.display());
            log::info!("database: {}", data_subdir("database").join("neural-agent-os.sqlite").display());
            if let Some(log_file) = logging::log_path() {
                log::info!("log file: {}", log_file.display());
            }
            start_local_api(handle.clone());
            // Drag-and-drop recording import: drop audio/video files anywhere
            // on the window to import + queue transcription.
            let drop_app = handle.clone();
            if let Some(window) = handle.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                        let app = drop_app.clone();
                        let paths = paths.clone();
                        std::thread::spawn(move || {
                            const AUDIO_EXTS: &[&str] = &["mp3", "wav", "m4a", "mp4", "mov", "webm", "ogg", "flac"];
                            let mut imported = 0usize;
                            for path in &paths {
                                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                                if !AUDIO_EXTS.contains(&ext.as_str()) { continue; }
                                let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Recording").to_string();
                                if let Ok(meeting) = import_meeting_recording(app.clone(), "personal".to_string(), path.to_string_lossy().into_owned(), title) {
                                    if let Ok(conn) = open_database(&app) {
                                        let payload = serde_json::json!({ "meeting_id": meeting.id });
                                        let _ = job_queue::enqueue_job(&conn, "transcription", "personal", &payload, 2);
                                    }
                                    imported += 1;
                                }
                            }
                            let _ = app.emit("recording-dropped", imported);
                        });
                    }
                });
            }
            std::thread::spawn(move || loop {
                let _ = scheduler_tick(&handle);
                std::thread::sleep(std::time::Duration::from_secs(30));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![local_status, list_workspaces, create_workspace, save_setting, load_setting, set_data_directory, delete_workspace, preview_workspace_deletion, ingest_directory, index_sources, search_sources, list_sources, delete_source, export_workspace, import_workspace, import_meeting_recording, list_meetings, queue_transcription, list_transcription_jobs, process_transcription_job, list_transcript_segments, update_transcript_speaker, summarize_meeting, get_meeting_summary, promote_action_to_task, list_open_actions, create_task, delete_task, list_tasks, update_task_status, schedule_task_reminder, cancel_reminder, delete_reminder, list_reminders, create_calendar_event, update_calendar_event, delete_calendar_event, list_calendar_events, connect_calendar_provider, list_calendar_connections, connect_email_account, list_email_accounts, store_email_secret, store_provider_secret, delete_provider_secret, sync_email_account, list_emails, sync_email_oauth, embed_sources, semantic_search_sources, ask_assistant, create_agent, list_agents, update_agent, update_agent_status, list_agent_runs, run_agent_now, set_model_route, list_model_routes, download_model, start_voice_capture, stop_voice_capture, check_voice_silence, stream_voice_reply, start_local_recording, stop_local_recording, create_note, update_note, list_notes, delete_note, search_notes, import_webpage, send_email, create_voice_profile, list_voice_profiles, delete_voice_profile, conversation_search, detect_upcoming_meetings, link_transcript_to_profile, generate_oauth_url, exchange_oauth_code, sync_calendar_events, extract_email_actions, cloud_transcribe, get_fallback_chain, set_fallback_provider, clear_fallback_chain, get_cost_summary, set_cost_limit, get_cost_limit, rerank_search, add_participant, list_participants, update_participant_status, remove_participant, detect_conflicts, check_conflicts_for_event, add_monitored_folder, list_monitored_folders, toggle_monitored_folder, remove_monitored_folder, prepare_meeting, diarize_transcript, build_speaker_embedding, enqueue_job, list_jobs, cancel_job, get_queue_status, check_permission, check_all_permissions, list_agent_permissions, grant_permission, revoke_permission, set_workspace_allowlist, get_workspace_allowlist, get_storage_stats, cleanup_old_recordings, get_audit_log, enqueue_sync, get_sync_status, resolve_sync_conflict, request_approval, approve_request, deny_request, list_pending_approvals, get_oauth_config, generate_pkce, build_authorize_url, exchange_code_with_pkce, refresh_oauth_token, preview_source_deletion, preview_meeting_deletion, full_deletion_preview, expand_recurring_event, common_timezones, convert_timezone, evaluate_recording_rules, set_recording_rule, list_recording_rules, delete_recording_rule, rebuild_threads, create_label, list_labels, label_email, list_labeled_emails, index_attachment, refresh_provider_health, get_provider_health, generate_email_draft, summarize_email_thread, extract_attachment_text, suggest_workspace_for_email, get_agent_activity, local_text_to_speech, local_speech_to_text, pull_remote_calendar, delete_workspace_complete, send_calendar_invitation, process_rsvp_response, generate_ics, differential_backup, get_workspace_retention, set_workspace_retention, authorize_action, delete_voice_profile, check_recording_allowed, execute_command, modify_file, generate_api_key, list_api_keys, revoke_api_key, delete_source_embeddings, delete_transcript_segments, delete_meeting_summary, delete_meeting_actions, delete_meeting_recording, set_token_limit, get_token_limit, list_token_limits, create_memory, list_memories, search_memories, delete_memory, find_duplicates, hybrid_search, search_emails, semantic_search_emails, export_context, import_meeting_recordings_batch, update_transcript_segment_text, reprocess_transcription, email_triage, research_brief, schedule_event_reminder, list_bot_runs, launch_meeting_bot, test_provider_connection, verify_backup, read_app_log, app_log_path, log_event])
        .build(tauri::generate_context!())
        .expect("error while building Neural Agent OS")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                cleanup_capture_processes(app_handle);
            }
        });
}


// ═══════════════════════════════════════════════════════════════════════════
// Plan-completion additions (verified Aug 11). Core logic lives in
// plan_extras (connection-based, unit-tested); these commands are thin
// wrappers that open the database and delegate.
// ═══════════════════════════════════════════════════════════════════════════

fn ollama_embed(model: &str, text: &str) -> Result<Vec<f32>, String> {
    let endpoint = std::env::var("NEURAL_OLLAMA_EMBEDDINGS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/embeddings".into());
    let response = reqwest::blocking::Client::new()
        .post(&endpoint)
        .json(&serde_json::json!({ "model": model, "prompt": text }))
        .send().map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json::<OllamaEmbeddingResponse>().map_err(|e| e.to_string())?;
    Ok(response.embedding)
}

/// Default LM Studio chat model (configurable via NEURAL_OPENAI_COMPATIBLE_MODEL).
pub(crate) fn default_lmstudio_model() -> String {
    std::env::var("NEURAL_OPENAI_COMPATIBLE_MODEL")
        .unwrap_or_else(|_| "qwen/qwen3.5-9b".into())
}

/// Default LM Studio embedding model (configurable via
/// NEURAL_OPENAI_COMPATIBLE_EMBEDDING_MODEL).
pub(crate) fn default_lmstudio_embedding_model() -> String {
    std::env::var("NEURAL_OPENAI_COMPATIBLE_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-nomic-embed-text-v1.5@q8_0".into())
}

/// Embed a single text through the provider's embeddings endpoint.
/// LM Studio / any OpenAI-compatible runtime (provider "openai_compatible")
/// uses POST <base>/embeddings with `{model, input}` and reads
/// `data[0].embedding`; Ollama uses `{model, prompt}` and reads `embedding`.
pub(crate) fn embed_text(provider: &str, model: &str, text: &str) -> Result<Vec<f32>, String> {
    if provider == "openai_compatible" {
        let base = std::env::var("NEURAL_OPENAI_COMPATIBLE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:1234/v1".into());
        let base = base.trim_end_matches('/');
        let base = base.strip_suffix("/embeddings").unwrap_or(base);
        let url = format!("{base}/embeddings");
        let mut req = reqwest::blocking::Client::new().post(&url);
        if let Ok(key) = chat::provider_api_key("openai_compatible") {
            req = req.bearer_auth(&key);
        }
        let response = req
            .json(&serde_json::json!({ "model": model, "input": text }))
            .send()
            .map_err(|e| format!("Embeddings endpoint {url} unreachable: {e}. Start LM Studio (or set NEURAL_OPENAI_COMPATIBLE_URL)."))?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<serde_json::Value>()
            .map_err(|e| e.to_string())?;
        let embedding = response["data"][0]["embedding"].as_array()
            .ok_or_else(|| format!("Embeddings response from {url} missing data[0].embedding: {}", response))?;
        embedding.iter().map(|v| v.as_f64().map(|f| f as f32)
            .ok_or_else(|| "Non-numeric embedding value".to_string())).collect()
    } else {
        ollama_embed(model, text)
    }
}

// ── Token limits (§10) ──────────────────────────────────────────────────────
#[tauri::command]
fn set_token_limit(app: tauri::AppHandle, workspace_id: String, capability: String, limit_tokens: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    plan_extras::set_token_limit(&connection, &workspace_id, &capability, limit_tokens)
}

#[tauri::command]
fn get_token_limit(app: tauri::AppHandle, workspace_id: String, capability: String) -> Result<Option<i64>, String> {
    let connection = open_database(&app)?;
    plan_extras::get_token_limit(&connection, &workspace_id, &capability)
}

#[tauri::command]
fn list_token_limits(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<serde_json::Value>, String> {
    let connection = open_database(&app)?;
    plan_extras::list_token_limits(&connection, &workspace_id)
}

// ── Memories (§4/§5) ────────────────────────────────────────────────────────
#[tauri::command]
fn create_memory(app: tauri::AppHandle, workspace_id: String, content: String, source_kind: Option<String>, source_id: Option<String>) -> Result<plan_extras::MemoryRecord, String> {
    let connection = open_database(&app)?;
    plan_extras::create_memory(&connection, &workspace_id, &content, source_kind.as_deref(), source_id.as_deref())
}

#[tauri::command]
fn list_memories(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<plan_extras::MemoryRecord>, String> {
    let connection = open_database(&app)?;
    plan_extras::list_memories(&connection, &workspace_id)
}

#[tauri::command]
fn search_memories(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<plan_extras::MemoryRecord>, String> {
    let connection = open_database(&app)?;
    plan_extras::search_memories(&connection, &workspace_id, &query)
}

#[tauri::command]
fn delete_memory(app: tauri::AppHandle, memory_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    plan_extras::delete_memory(&connection, &memory_id)
}

// ── Duplicate detection (§5) ────────────────────────────────────────────────
#[tauri::command]
fn find_duplicates(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<plan_extras::DuplicateGroup>, String> {
    let connection = open_database(&app)?;
    plan_extras::find_duplicates(&connection, &workspace_id)
}

// ── Hybrid ranking (§5) ─────────────────────────────────────────────────────
#[tauri::command]
fn hybrid_search(app: tauri::AppHandle, workspace_id: String, query: String, model: String) -> Result<Vec<serde_json::Value>, String> {
    let connection = open_database(&app)?;
    let match_query = fts5_match_terms(&query);
    let mut keyword: Vec<(String, f32, serde_json::Value)> = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT s.id, s.title, s.uri, sc.id, sc.content FROM source_chunks sc JOIN sources s ON s.id = sc.source_id JOIN source_search ss ON ss.chunk_id = sc.id WHERE s.workspace_id = ?1 AND source_search MATCH ?2 ORDER BY rank LIMIT 10",
    ) {
        if let Ok(rows) = stmt.query_map(params![workspace_id, match_query], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?.unwrap_or_default(), row.get::<_, String>(3)?, row.get::<_, String>(4)?))) {
            for row in rows {
                if let Ok((sid, title, uri, cid, content)) = row {
                    keyword.push((cid.clone(), 1.0, serde_json::json!({ "source_id": sid, "title": title, "uri": uri, "chunk_id": cid, "content": content })));
                }
            }
        }
    }
    let mut semantic: Vec<(String, f32, serde_json::Value)> = Vec::new();
    let provider = chat::routed_provider(&connection, &workspace_id, "embeddings");
    if let Ok(query_embedding) = embed_text(&provider, &model, &query) {
        if let Ok(mut stmt) = connection.prepare(
            "SELECT ce.chunk_id, ce.vector_json, sc.source_id, sc.content, s.title, s.uri FROM chunk_embeddings ce JOIN source_chunks sc ON sc.id = ce.chunk_id JOIN sources s ON s.id = sc.source_id WHERE s.workspace_id = ?1",
        ) {
            if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, Option<String>>(5)?.unwrap_or_default()))) {
                for row in rows {
                    if let Ok((chunk_id, vector_json, source_id, content, title, uri)) = row {
                        if let Ok(vector) = serde_json::from_str::<Vec<f32>>(&vector_json) {
                            let score = cosine_similarity(&query_embedding, &vector);
                            if score > 0.3 {
                                semantic.push((chunk_id.clone(), score, serde_json::json!({ "source_id": source_id, "title": title, "uri": uri, "chunk_id": chunk_id, "content": content })));
                            }
                        }
                    }
                }
            }
        }
    }
    let mut merged: Vec<(String, f32, serde_json::Value)> = Vec::new();
    for (cid, score, item) in keyword.into_iter().chain(semantic) {
        if let Some(existing) = merged.iter_mut().find(|(eid, _, _)| *eid == cid) {
            if score > existing.1 { existing.1 = score; existing.2 = item; }
        } else {
            merged.push((cid, score, item));
        }
    }
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut results: Vec<serde_json::Value> = merged.into_iter().take(20).map(|(_, score, mut item)| {
        item["score"] = serde_json::json!(score);
        item
    }).collect();
    results.sort_by(|a, b| b["score"].as_f64().unwrap_or(0.0).partial_cmp(&a["score"].as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results)
}

// ── Email search: keyword + semantic (§9) ──────────────────────────────────
#[tauri::command]
fn search_emails(app: tauri::AppHandle, workspace_id: String, query: String) -> Result<Vec<plan_extras::EmailHit>, String> {
    let connection = open_database(&app)?;
    plan_extras::search_emails(&connection, &workspace_id, &query)
}

#[tauri::command]
fn semantic_search_emails(app: tauri::AppHandle, workspace_id: String, query: String, model: String) -> Result<Vec<plan_extras::EmailHit>, String> {
    let connection = open_database(&app)?;
    let model = routed_model(&connection, &workspace_id, "embeddings", &model);
    let provider = chat::routed_provider(&connection, &workspace_id, "embeddings");
    let query_embedding = embed_text(&provider, &model, &query)?;
    let mut stmt = connection.prepare(
        "SELECT e.id, e.account_id, e.subject, e.sender, e.recipients, e.body, e.received_at FROM emails e JOIN email_accounts ea ON ea.id = e.account_id JOIN email_search es ON es.email_id = e.id WHERE ea.workspace_id = ?1 AND email_search MATCH ?2 LIMIT 50",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![workspace_id, fts5_match_terms(&query)], |row| Ok((
        row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?,
        row.get::<_, Option<String>>(5)?, row.get::<_, Option<String>>(6)?,
    ))).map_err(|e| e.to_string())?;
    let mut ranked = Vec::new();
    for row in rows {
        let (id, account_id, subject, sender, recipients, body, received_at) = row.map_err(|e| e.to_string())?;
        let text = format!("{} {}", subject, body.clone().unwrap_or_default());
        if let Ok(embedding) = embed_text(&provider, &model, &text) {
            let score = cosine_similarity(&query_embedding, &embedding);
            if score > 0.3 {
                ranked.push((score, plan_extras::EmailHit { id, account_id, subject, sender, recipients, body, received_at, score: None }));
            }
        }
    }
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(ranked.into_iter().take(10).map(|(score, mut hit)| { hit.score = Some(score); hit }).collect())
}

// ── Context export (§13) ────────────────────────────────────────────────────
#[tauri::command]
fn export_context(app: tauri::AppHandle, workspace_id: String) -> Result<String, String> {
    let connection = open_database(&app)?;
    plan_extras::export_context(&connection, &workspace_id)
}

// ── Batch recording import (§6) ─────────────────────────────────────────────
#[tauri::command]
fn import_meeting_recordings_batch(app: tauri::AppHandle, workspace_id: String, paths: Vec<String>) -> Result<Vec<MeetingRecord>, String> {
    let mut imported = Vec::new();
    for path in &paths {
        let title = std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("Recording").to_string();
        match import_meeting_recording(app.clone(), workspace_id.clone(), path.clone(), title) {
            Ok(meeting) => imported.push(meeting),
            Err(error) => return Err(format!("Failed to import {path}: {error}")),
        }
    }
    Ok(imported)
}

// ── Transcript editing + reprocessing (§7) ──────────────────────────────────
#[tauri::command]
fn update_transcript_segment_text(app: tauri::AppHandle, segment_id: String, text: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    plan_extras::update_transcript_segment_text(&connection, &segment_id, &text)
}

#[tauri::command]
fn reprocess_transcription(app: tauri::AppHandle, meeting_id: String, provider: Option<String>, language: Option<String>) -> Result<TranscriptionJob, String> {
    log::info!("reprocess_transcription: meeting={meeting_id} provider={provider:?} language={language:?}");
    let connection = open_database(&app)?;
    // Fail fast on a missing meeting (no dangling queue entry).
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM meetings WHERE id = ?1)",
        params![meeting_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !exists { return Err(format!("Meeting not found: {meeting_id}")); }
    let workspace_id: String = connection.query_row(
        "SELECT workspace_id FROM meetings WHERE id = ?1",
        params![meeting_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let provider = provider.unwrap_or_else(|| "local-whisper".into());
    // Route through the background queue (same path as auto-imported
    // recordings) so the scheduler runs it with retries + logging. The old
    // implementation inserted into transcription_jobs, which nothing ever
    // processes — Retry/Reprocess appeared to do nothing.
    if let Some(lang) = language.as_deref().filter(|l| !l.trim().is_empty()) {
        let _ = connection.execute("UPDATE meetings SET language = ?1 WHERE id = ?2", params![lang, meeting_id]);
    }
    let payload = serde_json::json!({ "meeting_id": meeting_id, "language": language });
    let job = job_queue::enqueue_job(&connection, "transcription", &workspace_id, &payload, 2)?;
    Ok(TranscriptionJob { id: job.id, meeting_id, provider, status: "queued".into(), error: None })
}

// ── Email triage + research briefs (§11) ────────────────────────────────────
#[tauri::command]
fn email_triage(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<plan_extras::TriageItem>, String> {
    let connection = open_database(&app)?;
    plan_extras::email_triage(&connection, &workspace_id)
}

#[tauri::command]
fn research_brief(app: tauri::AppHandle, workspace_id: String, query: String, model: Option<String>) -> Result<String, String> {
    let connection = open_database(&app)?;
    let mut context = String::new();
    if let Ok(results) = search_sources(app.clone(), workspace_id.clone(), query.clone()) {
        context.push_str("## Matching sources\\n");
        for r in results {
            context.push_str(&format!("- {}: {}\\n", r.title, r.content.chars().take(200).collect::<String>()));
        }
    }
    if let Ok(notes) = list_notes(app.clone(), workspace_id.clone()) {
        context.push_str("\\n## Notes\\n");
        for n in notes {
            context.push_str(&format!("- {}: {}\\n", n.title, n.content.chars().take(160).collect::<String>()));
        }
    }
    if let Ok(transcripts) = conversation_search(app.clone(), workspace_id.clone(), query.clone()) {
        context.push_str("\\n## Transcripts\\n");
        for t in transcripts {
            context.push_str(&format!("- {}: {}\\n", t.title, t.content.chars().take(160).collect::<String>()));
        }
    }
    if let Ok(tasks) = list_tasks(app.clone(), workspace_id.clone()) {
        context.push_str("\\n## Tasks\\n");
        for t in tasks {
            context.push_str(&format!("- {} ({})\\n", t.title, t.status));
        }
    }
    if context.trim().is_empty() {
        return Ok(format!("# Research Brief: {query}\\n\\nNo matching context found in this workspace."));
    }
    let model = routed_model(&connection, &workspace_id, "chat", &model.unwrap_or_else(default_lmstudio_model));
    let prompt = format!("Summarize the following context into a concise research brief answering: {query}\n\nCONTEXT:\n{context}");
    match chat::chat_completion(&connection, &workspace_id, "chat", &model, &prompt) {
        Ok(text) => Ok(format!("# Research Brief: {query}\n\n{}\n\n---\nCompiled from workspace context.", text)),
        Err(_) => Ok(format!("# Research Brief: {query}\n\n{}", context)),
    }
}

// ── Calendar-event reminders (§8) ───────────────────────────────────────────
#[tauri::command]
fn schedule_event_reminder(app: tauri::AppHandle, workspace_id: String, event_id: String, title: String, scheduled_at: String) -> Result<ReminderRecord, String> {
    let connection = open_database(&app)?;
    plan_extras::schedule_event_reminder(&connection, &workspace_id, &event_id, &title, &scheduled_at)
}

// ── Teams-bot run history (§6/§13) ──────────────────────────────────────────
#[tauri::command]
fn list_bot_runs(app: tauri::AppHandle, workspace_id: String) -> Result<Vec<plan_extras::BotRunRecord>, String> {
    let connection = open_database(&app)?;
    plan_extras::list_bot_runs(&connection, &workspace_id)
}

// ── Provider connectivity test (P2) ─────────────────────────────────────────
#[tauri::command]
fn test_provider_connection(provider: String) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();
    let result = match provider.as_str() {
        "openai" => {
            let key = chat::provider_api_key("openai")?;
            client.get("https://api.openai.com/v1/models").bearer_auth(&key).send()
                .map_err(|e| format!("OpenAI unreachable: {e}"))?
        }
        "anthropic" => {
            let key = chat::provider_api_key("anthropic")?;
            client.get("https://api.anthropic.com/v1/models").header("x-api-key", &key)
                .header("anthropic-version", "2023-06-01").send()
                .map_err(|e| format!("Anthropic unreachable: {e}"))?
        }
        "google" => {
            let key = chat::provider_api_key("google")?;
            client.get(format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}")).send()
                .map_err(|e| format!("Google unreachable: {e}"))?
        }
        "openai_compatible" | "lmstudio" | "llamacpp" | "vllm" => {
            let base = std::env::var("NEURAL_OPENAI_COMPATIBLE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:1234/v1".into());
            let url = if base.ends_with('/') { format!("{base}models") } else { format!("{base}/models") };
            client.get(&url).send()
                .map_err(|e| format!("OpenAI-compatible endpoint unreachable: {e}"))?
        }
        "ollama" | "local" => {
            let url = std::env::var("NEURAL_OLLAMA_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434/api/tags".into());
            client.get(&url).send()
                .map_err(|e| format!("Ollama unreachable: {e}"))?
        }
        other => return Err(format!("Unknown provider: {other}")),
    };
    let status = result.status();
    Ok(serde_json::json!({
        "provider": provider,
        "ok": status.is_success(),
        "http_status": status.as_u16(),
    }))
}

// ── Backup verification (P2) ────────────────────────────────────────────────
#[tauri::command]
fn verify_backup(app: tauri::AppHandle, path: String) -> Result<bool, String> {
    let resolved = resolve_output_path(&app, &path)?;
    retention::validate_backup(resolved.to_string_lossy().as_ref())
}

#[cfg(test)]
mod tests {
    use super::{apply_workspace_import, build_workspace_export, cosine_similarity, default_imap_host, export_hash_valid, fts5_match_terms, hosts_for_address, insert_note, percent_decode, plan_auto_recordings, resolve_output_path_inner, terminate_capture_child, AutoRecordPlan, Schedule, Utc, ollama_base_url};

    /// Serializes tests that mutate process-global env vars (whisper/diarizer
    /// binaries, data dir, secrets). Recoverable so a panicking test does not
    /// poison every later env-touching test.
    pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    use rusqlite::params;
    use std::process::Command;
    use std::str::FromStr;
    use rusqlite::Connection;

    /// Open an in-memory database with the full schema applied. Used by
    /// unit tests in every backend module.
    pub(crate) fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory db");
        crate::init_schema(&connection).expect("schema");
        connection
    }

    /// Seed a workspace with rows in every export domain.
    fn seed_export_workspace(connection: &Connection) -> String {
        connection.execute("INSERT INTO workspaces (id, name, source_count) VALUES ('w1', 'Export Test', 2)", []).unwrap();
        connection.execute("INSERT INTO sources (id, workspace_id, kind, title, uri, status) VALUES ('s1', 'w1', 'file', 'Doc A', 'file:///a.txt', 'indexed')", []).unwrap();
        connection.execute("INSERT INTO sources (id, workspace_id, kind, title, uri, status) VALUES ('s2', 'w1', 'file', 'Doc B', 'file:///b.txt', 'indexed')", []).unwrap();
        connection.execute("INSERT INTO source_chunks (id, source_id, chunk_index, content) VALUES ('c1', 's1', 0, 'alpha content')", []).unwrap();
        connection.execute("INSERT INTO source_chunks (id, source_id, chunk_index, content) VALUES ('c2', 's1', 1, 'beta content')", []).unwrap();
        connection.execute("INSERT INTO source_chunks (id, source_id, chunk_index, content) VALUES ('c3', 's2', 0, 'gamma content')", []).unwrap();
        connection.execute("INSERT INTO meetings (id, workspace_id, title, status, language) VALUES ('m1', 'w1', 'Sync', 'transcribed', 'en')", []).unwrap();
        connection.execute("INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text, confidence) VALUES ('t1', 'm1', 0.0, 2.0, 'hello', 0.9)", []).unwrap();
        connection.execute("INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text) VALUES ('t2', 'm1', 2.0, 4.0, 'world')", []).unwrap();
        connection.execute("INSERT INTO meeting_summaries (meeting_id, model, summary) VALUES ('m1', 'mock-model', '# Summary\nDecided to ship.')", []).unwrap();
        connection.execute("INSERT INTO meeting_actions (id, meeting_id, title, owner, status) VALUES ('act1', 'm1', 'Ship the release', 'Alice', 'open')", []).unwrap();
        connection.execute("INSERT INTO transcription_jobs (id, meeting_id, provider, status, language) VALUES ('j1', 'm1', 'local-whisper', 'completed', 'en')", []).unwrap();
        connection.execute("INSERT INTO tasks (id, workspace_id, title, status) VALUES ('k1', 'w1', 'Ship it', 'open')", []).unwrap();
        connection.execute("INSERT INTO reminders (id, workspace_id, task_id, title, scheduled_at, status) VALUES ('r1', 'w1', 'k1', 'Remind me', '2026-08-15T09:00:00Z', 'scheduled')", []).unwrap();
        connection.execute("INSERT INTO email_accounts (id, workspace_id, provider, account_label, status) VALUES ('e1', 'w1', 'imap', 'Inbox', 'connected')", []).unwrap();
        connection.execute("INSERT INTO emails (id, account_id, subject, sender, recipients, body, received_at) VALUES ('em1', 'e1', 'Hi', 'a@x.com', 'b@x.com', 'body', '2026-08-10T10:00:00Z')", []).unwrap();
        connection.execute("INSERT INTO chunk_embeddings (chunk_id, model, vector_json) VALUES ('c1', 'nomic-embed-text', '[0.1,0.2]')", []).unwrap();
        connection.execute("INSERT INTO voice_profiles (id, workspace_id, speaker_label, sample_count) VALUES ('v1', 'w1', 'Alice', 3)", []).unwrap();
        connection.execute("INSERT INTO speaker_embeddings (profile_id, model, vector_json, sample_count) VALUES ('v1', 'nomic-embed-text', '[0.5]', 3)", []).unwrap();
        connection.execute("INSERT INTO agents (id, workspace_id, name, prompt, schedule, permission_mode, status) VALUES ('a1', 'w1', 'Bot', 'do it', '0 * * * *', 'full', 'active')", []).unwrap();
        connection.execute("INSERT INTO agent_runs (id, agent_id, status, retry_count) VALUES ('ar1', 'a1', 'completed', 0)", []).unwrap();
        connection.execute("INSERT INTO agent_permissions (agent_id, workspace_id, capability, granted) VALUES ('a1', 'w1', 'create_notes', 1)", []).unwrap();
        connection.execute("INSERT INTO workspace_allowlists (workspace_id, capability) VALUES ('w1', 'read_knowledge')", []).unwrap();
        connection.execute("INSERT INTO notes (id, workspace_id, title, content) VALUES ('n1', 'w1', 'Note', 'content')", []).unwrap();
        connection.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider) VALUES ('ce1', 'w1', 'Event', '2026-08-16T09:00:00Z', '2026-08-16T10:00:00Z', 'local')", []).unwrap();
        connection.execute("INSERT INTO calendar_participants (id, event_id, name, status) VALUES ('cp1', 'ce1', 'Alice', 'accepted')", []).unwrap();
        connection.execute("INSERT INTO model_routes (workspace_id, capability, provider, model) VALUES ('w1', 'chat', 'openai_compatible', 'qwen')", []).unwrap();
        connection.execute("INSERT INTO memories (id, workspace_id, content) VALUES ('mem1', 'w1', 'remember this')", []).unwrap();
        connection.execute("INSERT INTO web_pages (id, workspace_id, url, title) VALUES ('wp1', 'w1', 'https://example.com', 'Example')", []).unwrap();
        connection.execute("INSERT INTO bot_runs (id, workspace_id, bot_id, status, platform) VALUES ('br1', 'w1', 'bot-1', 'running', 'teams')", []).unwrap();
        "w1".to_string()
    }

    #[test]
    fn workspace_export_roundtrip_preserves_all_domains() {
        let source = test_connection();
        let workspace_id = seed_export_workspace(&source);
        let export = build_workspace_export(&source, &workspace_id).unwrap();

        assert_eq!(export.version, 2);
        assert_eq!(export.schema_version, 2);
        assert!(export_hash_valid(&export), "fresh export must hash-verify");
        assert_eq!(export.row_counts.get("sources"), Some(&2));
        assert_eq!(export.row_counts.get("source_chunks"), Some(&3));
        assert_eq!(export.row_counts.get("transcript_segments"), Some(&2));
        assert_eq!(export.row_counts.get("meeting_summaries"), Some(&1));
        assert_eq!(export.row_counts.get("meeting_actions"), Some(&1));
        assert_eq!(export.row_counts.get("calendar_participants"), Some(&1));
        assert_eq!(export.row_counts.get("notes"), Some(&1));
        assert_eq!(export.row_counts.get("agent_permissions"), Some(&1));

        // Restore into a fresh database and compare every domain count.
        let target = test_connection();
        let restored = apply_workspace_import(&target, &export).unwrap();
        for (domain, expected) in &export.row_counts {
            assert_eq!(restored.get(domain), Some(expected), "domain {domain} count mismatch");
        }
        // Spot-check row fidelity.
        let title: String = target.query_row("SELECT title FROM meetings WHERE id = 'm1'", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "Sync");
        let lang: String = target.query_row("SELECT language FROM meetings WHERE id = 'm1'", [], |r| r.get(0)).unwrap();
        assert_eq!(lang, "en");
        let granted: i64 = target.query_row("SELECT granted FROM agent_permissions WHERE agent_id = 'a1'", [], |r| r.get(0)).unwrap();
        assert_eq!(granted, 1);
        let conf: Option<f64> = target.query_row("SELECT confidence FROM transcript_segments WHERE id = 't1'", [], |r| r.get(0)).unwrap();
        assert_eq!(conf, Some(0.9));
        // Meeting summary + extracted actions survive the roundtrip.
        let summary: String = target.query_row("SELECT summary FROM meeting_summaries WHERE meeting_id = 'm1'", [], |r| r.get(0)).unwrap();
        assert!(summary.contains("Decided to ship"));
        let action_title: String = target.query_row("SELECT title FROM meeting_actions WHERE id = 'act1'", [], |r| r.get(0)).unwrap();
        assert_eq!(action_title, "Ship the release");
    }

    #[test]
    fn workspace_export_detects_tampering() {
        let source = test_connection();
        let workspace_id = seed_export_workspace(&source);
        let export = build_workspace_export(&source, &workspace_id).unwrap();

        // Tamper with a note's content: hash must no longer verify.
        let mut tampered = export.clone();
        tampered.notes[0]["content"] = serde_json::Value::String("MALICIOUS".into());
        assert!(!export_hash_valid(&tampered), "tampered export must fail hash");
        assert!(export_hash_valid(&export), "original export still verifies");

        // validate_backup (path-based) must reject the tampered file and accept the clean one.
        let dir = std::env::temp_dir().join(format!("nao-export-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let clean_path = dir.join("clean.json");
        let tampered_path = dir.join("tampered.json");
        std::fs::write(&clean_path, serde_json::to_vec_pretty(&export).unwrap()).unwrap();
        std::fs::write(&tampered_path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
        assert!(crate::retention::validate_backup(clean_path.to_str().unwrap()).unwrap());
        assert!(!crate::retention::validate_backup(tampered_path.to_str().unwrap()).unwrap());
        // A v1-style file (no hash) still validates as long as fields exist.
        let legacy = serde_json::json!({ "version": 1, "workspace_id": "w1", "exported_at": "2026-08-14T00:00:00Z", "sources": [] });
        let legacy_path = dir.join("legacy.json");
        std::fs::write(&legacy_path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
        assert!(crate::retention::validate_backup(legacy_path.to_str().unwrap()).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn workspace_export_empty_hash_is_invalid() {
        let source = test_connection();
        let workspace_id = seed_export_workspace(&source);
        let mut export = build_workspace_export(&source, &workspace_id).unwrap();
        export.content_hash.clear();
        assert!(!export_hash_valid(&export));
    }

    /// #3: the scheduler's auto-record planner finds in-window events that
    /// match rules, skips already-auto-recorded events, and reports the rule
    /// action per event.
    #[test]
    fn auto_record_planner_windows_rules_and_dedup() {
        use chrono::Duration;
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        let now = Utc::now();
        let fmt = |d: chrono::DateTime<chrono::Utc>| d.to_rfc3339();
        // Event starting now → planned.
        conn.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider) VALUES ('e-now', 'w1', 'Daily Standup', ?1, ?2, 'local')", params![fmt(now), fmt(now + Duration::minutes(30))]).unwrap();
        // Event starting in 3 hours → outside the 2-min window, not planned.
        conn.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider) VALUES ('e-later', 'w1', 'Daily Standup', ?1, ?2, 'local')", params![fmt(now + Duration::hours(3)), fmt(now + Duration::hours(4))]).unwrap();
        // Event already auto-recorded → skipped even though in-window.
        conn.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider, recording_auto_started) VALUES ('e-done', 'w1', 'Daily Standup', ?1, ?2, 'local', ?3)", params![fmt(now + Duration::seconds(30)), fmt(now + Duration::minutes(30)), fmt(now)]).unwrap();
        // An event with no matching rule falls back to the global mode.
        conn.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider) VALUES ('e-plain', 'w1', 'Unrelated Chat', ?1, ?2, 'local')", params![fmt(now + Duration::seconds(10)), fmt(now + Duration::minutes(20))]).unwrap();

        // Rule: calendar titles containing "standup" → automatic (priority 1);
        // global default = confirm.
        crate::calendar_complete::set_recording_rule(&conn, "w1", "calendar", "standup", "automatic", 1).unwrap();
        conn.execute("INSERT INTO app_settings (key, value) VALUES ('recordingMode', 'confirm')", []).unwrap();

        let plans = plan_auto_recordings(&conn).unwrap();
        let by_event: std::collections::HashMap<&str, &AutoRecordPlan> = plans.iter().map(|p| (p.event_id.as_str(), p)).collect();
        assert_eq!(by_event.len(), 2, "only e-now and e-plain should be planned: {plans:?}");
        assert_eq!(by_event["e-now"].action, "automatic");
        assert_eq!(by_event["e-now"].ends_at.as_deref(), Some(fmt(now + Duration::minutes(30)).as_str()));
        assert_eq!(by_event["e-plain"].action, "confirm");
        assert!(!by_event.contains_key("e-later"), "outside window");
        assert!(!by_event.contains_key("e-done"), "already auto-recorded");
    }

    /// #Knowledge-tab fix: user queries containing FTS5 syntax characters
    /// (commas, quotes, parens) must match literally instead of raising
    /// `fts5: syntax error near ","`.
    #[test]
    fn fts5_match_terms_never_raises_syntax_errors() {
        let conn = test_connection();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS fts_probe USING fts5(content);
             INSERT INTO fts_probe (content) VALUES ('hello, world notes are here');
             INSERT INTO fts_probe (content) VALUES ('unrelated item');",
        ).unwrap();

        let count_for = |q: &str| -> i64 {
            let expr = fts5_match_terms(q);
            conn.query_row(
                "SELECT COUNT(*) FROM fts_probe WHERE fts_probe MATCH ?1",
                params![expr], |r| r.get(0),
            ).unwrap()
        };
        // The reported bug: a comma in the query.
        assert_eq!(count_for("hello, world"), 1, "comma query must match literally");
        // Quotes, parens, and apostrophes are literal too (bound params keep
        // the SQL string safe).
        assert_eq!(count_for("say \"hi\" (now)"), 0, "no such row -> no syntax error, 0 hits");
        assert_eq!(count_for("hello notes"), 1, "plain multi-term OR still works");
        assert_eq!(count_for("user's data"), 0, "apostrophe is safe via bound param");
        assert_eq!(fts5_match_terms("   "), "", "whitespace-only -> empty expression");
    }

    /// Gmail/Outlook accounts get provider-default IMAP hosts automatically
    /// (fixes "Set Gmail IMAP host to imap.gmail.com" when the field was left
    /// empty at connect time).
    #[test]
    fn imap_host_defaults_for_managed_providers() {
        assert_eq!(default_imap_host("gmail"), Some("imap.gmail.com"));
        assert_eq!(default_imap_host("outlook"), Some("outlook.office365.com"));
        assert_eq!(default_imap_host("imap"), None, "custom providers must supply a host");
    }

    /// Hosts are inferred from the account address domain, not just the
    /// provider dropdown (e.g. a Yahoo address under the generic "imap"
    /// provider still gets imap.mail.yahoo.com).
    #[test]
    fn hosts_are_mapped_from_account_address() {
        assert_eq!(hosts_for_address("me@gmail.com"), (Some("imap.gmail.com"), Some("smtp.gmail.com")));
        assert_eq!(hosts_for_address("me@hotmail.com"), (Some("outlook.office365.com"), Some("smtp-mail.outlook.com")));
        assert_eq!(hosts_for_address("Me@Outlook.COM"), (Some("outlook.office365.com"), Some("smtp-mail.outlook.com")), "case-insensitive");
        assert_eq!(hosts_for_address("me@yahoo.com"), (Some("imap.mail.yahoo.com"), Some("smtp.mail.yahoo.com")));
        assert_eq!(hosts_for_address("me@icloud.com"), (Some("imap.mail.me.com"), Some("smtp.mail.me.com")));
        assert_eq!(hosts_for_address("me@aol.com"), (Some("imap.aol.com"), Some("smtp.aol.com")));
        assert_eq!(hosts_for_address("me@zoho.com"), (Some("imap.zoho.com"), Some("smtp.zoho.com")));
        assert_eq!(hosts_for_address("me@mycompany.com"), (None, None), "unknown domains need manual hosts");
        assert_eq!(hosts_for_address("no-at-sign"), (None, None));
    }

    #[test]
    fn percent_decode_decodes_query_values() {
        assert_eq!(percent_decode("API%20Teams%20Upload"), "API Teams Upload");
        assert_eq!(percent_decode("meet-1"), "meet-1");
        assert_eq!(percent_decode("a%2Fb"), "a/b");
        assert_eq!(percent_decode("100%25"), "100%");
        assert_eq!(percent_decode("bad%zz"), "bad%zz"); // invalid escapes stay literal
        assert_eq!(percent_decode(""), "");
    }

    #[test]
    fn ollama_base_url_strips_api_suffix() {
        // NEURAL_OLLAMA_URL commonly points at /api/generate.
        std::env::set_var("NEURAL_OLLAMA_URL", "http://127.0.0.1:11434/api/generate");
        assert_eq!(ollama_base_url(), "http://127.0.0.1:11434");
        std::env::set_var("NEURAL_OLLAMA_URL", "http://localhost:11434/api/");
        assert_eq!(ollama_base_url(), "http://localhost:11434");
        // A bare root URL passes through untouched.
        std::env::set_var("NEURAL_OLLAMA_URL", "http://192.168.1.10:11434");
        assert_eq!(ollama_base_url(), "http://192.168.1.10:11434");
        std::env::remove_var("NEURAL_OLLAMA_URL");
        assert_eq!(ollama_base_url(), "http://127.0.0.1:11434");
    }

    #[test]
    fn output_paths_resolve_relative_and_tilde_paths() {
        let data_dir = std::path::Path::new("/data/nao");
        // ~/ paths expand to the home directory (writable even though the
        // packaged app runs with CWD = /).
        let home = resolve_output_path_inner("/Users/test", data_dir, "~/Recordings/meeting-2026-08-11_12-34-56.m4a").unwrap();
        assert_eq!(home, std::path::PathBuf::from("/Users/test/Recordings/meeting-2026-08-11_12-34-56.m4a"));
        // Relative paths resolve against the app data directory instead of
        // the read-only working directory.
        let rel = resolve_output_path_inner("/Users/test", data_dir, "recordings/x.wav").unwrap();
        assert_eq!(rel, std::path::PathBuf::from("/data/nao/recordings/x.wav"));
        // Absolute paths are untouched.
        let abs = resolve_output_path_inner("/Users/test", data_dir, "/tmp/x.wav").unwrap();
        assert_eq!(abs, std::path::PathBuf::from("/tmp/x.wav"));
        // Empty paths are rejected.
        assert!(resolve_output_path_inner("/Users/test", data_dir, "  ").is_err());
    }

    #[test]
    fn identical_vectors_have_maximum_similarity() {
        assert!((cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn orthogonal_vectors_have_zero_similarity() {
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < f32::EPSILON);
    }

    #[test]
    fn zero_vector_does_not_produce_nan() {
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn cron_expression_has_a_next_occurrence() {
        let schedule = Schedule::from_str("0 * * * * *").expect("valid cron expression");
        assert!(schedule.upcoming(Utc).next().is_some());
    }

    #[test]
    fn capture_child_is_terminated_on_cleanup() {
        let child = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", "ping", "127.0.0.1", "-n", "30"]).spawn().unwrap()
        } else {
            Command::new("sleep").arg("30").spawn().unwrap()
        };
        assert!(terminate_capture_child(child).is_ok());
    }

    /// Regression: `create_note` previously passed 4 params to a 5-slot
    /// INSERT (the `now` timestamp was missing), so the Notes view could never
    /// save a note. The shared helper must succeed against the real schema.
    #[test]
    fn create_note_insert_matches_schema() {
        let connection = test_connection();
        // `open_database` seeds the default workspaces; unit tests must too
        // (notes.workspace_id has an FK to workspaces.id).
        connection.execute(
            "INSERT OR IGNORE INTO workspaces (id, name, source_count) VALUES (?1, ?2, ?3)",
            params!["personal", "Personal", 0],
        ).unwrap();
        let now = crate::Utc::now().to_rfc3339();
        insert_note(&connection, "note-1", "personal", "Title", "Body", &now).expect("note insert should succeed");
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM notes WHERE id = 'note-1'", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1, "note row must exist");
        let search_count: i64 = connection.query_row("SELECT COUNT(*) FROM note_search WHERE note_id = 'note-1'", [], |row| row.get(0)).unwrap();
        assert_eq!(search_count, 1, "note_search FTS row must exist");
        // The same schema must also accept an update path used by update_note
        connection.execute("UPDATE notes SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4", params!["T2", "C2", now, "note-1"]).unwrap();
        connection.execute("UPDATE note_search SET title = ?1, content = ?2 WHERE note_id = ?3", params!["T2", "C2", "note-1"]).unwrap();
    }

}

#[cfg(test)]
mod voice_silence_tests {
    use super::analyze_wav_silence;

    /// Build a 16-bit PCM mono WAVE file in memory (16 kHz).
    fn synth_wav(seconds: f32, amplitude: f32, sample_rate: u32) -> Vec<u8> {
        let channels: u16 = 1;
        let bits: u16 = 16;
        let frame_count = (seconds * sample_rate as f32) as usize;
        let data_len = frame_count * (bits as usize / 8) * channels as usize;
        let mut bytes = Vec::with_capacity(44 + data_len);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * channels as u32 * (bits as u32 / 8)).to_le_bytes());
        bytes.extend_from_slice(&(channels * (bits / 8)).to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
        for i in 0..frame_count {
            let sample = ((i as f32 / sample_rate as f32 * 220.0 * std::f32::consts::TAU).sin() * amplitude * 32767.0) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn silence_detection_quiet_clip() {
        let dir = std::env::temp_dir();
        let path = dir.join("nao-silence-quiet.wav");
        std::fs::write(&path, synth_wav(2.0, 0.001, 16000)).unwrap();
        let (rms, has_speech, duration) = analyze_wav_silence(&path, 1.2).expect("decodes");
        assert!(rms < 0.01, "quiet clip rms {rms} should be near zero");
        assert!(!has_speech);
        assert!((duration - 2.0).abs() < 0.01);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn silence_detection_loud_clip() {
        let dir = std::env::temp_dir();
        let path = dir.join("nao-silence-loud.wav");
        std::fs::write(&path, synth_wav(2.0, 0.25, 16000)).unwrap();
        let (rms, has_speech, _) = analyze_wav_silence(&path, 1.2).expect("decodes");
        assert!(rms > 0.1, "loud clip rms {rms} should be well above threshold");
        assert!(has_speech);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn silence_detection_missing_file() {
        assert!(analyze_wav_silence(&std::path::PathBuf::from("/nonexistent/x.wav"), 1.2).is_none());
    }
}

