use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
pub struct StorageStats {
    pub database_path: String,
    pub database_size_bytes: u64,
    pub recordings_count: usize,
    pub recordings_size_bytes: u64,
    pub recordings_paths: Vec<RecordingInfo>,
    pub total_sources: usize,
    pub total_embeddings: usize,
    pub total_emails: usize,
    pub oldest_recording_date: Option<String>,
    pub storage_warning: Option<String>,
}

#[derive(Serialize)]
pub struct RecordingInfo {
    pub meeting_id: String,
    pub title: String,
    pub path: String,
    pub size_bytes: u64,
    pub created_at: Option<String>,
    pub status: String,
}

#[derive(Serialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: String,
    pub action: String,
    pub provider: Option<String>,
    pub data_type: Option<String>,
    pub workspace_id: Option<String>,
    pub details: String,
}

/// Get comprehensive storage statistics
pub fn get_storage_stats(app: &tauri::AppHandle) -> Result<StorageStats, String> {
    let connection = super::open_database(app)?;
    let db_path = super::database_path(app)?;
    let db_size = std::fs::metadata(&db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Gather recording info
    let mut stmt = connection
        .prepare(
            "SELECT id, title, recording_path, created_at, status
             FROM meetings WHERE recording_path IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;

    let recordings: Vec<RecordingInfo> = stmt
        .query_map([], |row| {
            let path: String = row.get(2)?;
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(RecordingInfo {
                meeting_id: row.get(0)?,
                title: row.get(1)?,
                path,
                size_bytes: size,
                created_at: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let recordings_size: u64 = recordings.iter().map(|r| r.size_bytes).sum();
    let oldest = recordings
        .iter()
        .filter_map(|r| r.created_at.clone())
        .min();

    let total_sources: usize = connection
        .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))
        .unwrap_or(0);

    let total_embeddings: usize = connection
        .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |row| row.get(0))
        .unwrap_or(0);

    let total_emails: usize = connection
        .query_row("SELECT COUNT(*) FROM emails", [], |row| row.get(0))
        .unwrap_or(0);

    // Storage warning at 10GB
    let warning = if db_size + recordings_size > 10_000_000_000 {
        Some("Storage exceeds 10GB. Consider cleaning up old recordings or moving the data directory.".into())
    } else if recordings_size > 5_000_000_000 {
        Some("Recording storage exceeds 5GB. Review old recordings for cleanup.".into())
    } else {
        None
    };

    Ok(StorageStats {
        database_path: db_path.to_string_lossy().into_owned(),
        database_size_bytes: db_size,
        recordings_count: recordings.len(),
        recordings_size_bytes: recordings_size,
        recordings_paths: recordings,
        total_sources,
        total_embeddings,
        total_emails,
        oldest_recording_date: oldest,
        storage_warning: warning,
    })
}

/// Delete old recordings older than specified days, with preview
pub fn cleanup_old_recordings(
    app: &tauri::AppHandle,
    older_than_days: i64,
    dry_run: bool,
) -> Result<Vec<String>, String> {
    let connection = super::open_database(app)?;
    let cutoff = chrono::Utc::now() - chrono::Duration::days(older_than_days);

    let mut stmt = connection
        .prepare(
            "SELECT id, title, recording_path FROM meetings
             WHERE recording_path IS NOT NULL AND created_at < ?1",
        )
        .map_err(|e| e.to_string())?;

    let to_clean: Vec<(String, String, String)> = stmt
        .query_map(params![cutoff.to_rfc3339()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut deleted = Vec::new();

    for (id, title, path) in &to_clean {
        if !dry_run {
            if !path.is_empty() {
                let _ = std::fs::remove_file(path);
            }
            connection
                .execute(
                    "UPDATE meetings SET recording_path = NULL WHERE id = ?1",
                    params![id],
                )
                .map_err(|e| e.to_string())?;
        }
        deleted.push(format!("{title} ({path})"));
    }

    Ok(deleted)
}

/// Record an audit entry for cloud data transfers
pub fn record_audit(
    connection: &Connection,
    action: &str,
    provider: Option<&str>,
    data_type: Option<&str>,
    workspace_id: Option<&str>,
    details: &str,
) -> Result<(), String> {
    let id = format!("audit-{}", uuid::Uuid::new_v4());
    connection
        .execute(
            "INSERT INTO audit_log (id, action, provider, data_type, workspace_id, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)",
            params![id, action, provider, data_type, workspace_id, details],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get audit log entries
pub fn get_audit_log(
    connection: &Connection,
    workspace_id: Option<&str>,
    limit: i64,
) -> Result<Vec<AuditEntry>, String> {
    let sql = match workspace_id {
        Some(_) => "SELECT id, created_at, action, provider, data_type, workspace_id, details
                    FROM audit_log WHERE workspace_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        None => "SELECT id, created_at, action, provider, data_type, workspace_id, details
                 FROM audit_log ORDER BY created_at DESC LIMIT ?1",
    };

    let mut stmt = connection.prepare(sql).map_err(|e| e.to_string())?;

    let rows: Vec<AuditEntry> = if let Some(ws) = workspace_id {
        stmt.query_map(params![ws, limit], |row| {
            Ok(AuditEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                action: row.get(2)?,
                provider: row.get(3)?,
                data_type: row.get(4)?,
                workspace_id: row.get(5)?,
                details: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    } else {
        stmt.query_map(params![limit], |row| {
            Ok(AuditEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                action: row.get(2)?,
                provider: row.get(3)?,
                data_type: row.get(4)?,
                workspace_id: row.get(5)?,
                details: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    };

    Ok(rows)
}

/// Schedule backup
pub fn schedule_backup(
    app: &tauri::AppHandle,
    connection: &Connection,
    workspace_id: &str,
    destination_path: &str,
) -> Result<String, String> {
    let destination = std::path::PathBuf::from(destination_path);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Export workspace as backup
    let export = super::export_workspace(
        app.clone(),
        workspace_id.to_string(),
        destination_path.to_string(),
    )?;

    // Validate backup integrity before reporting success
    if !validate_backup(destination_path)? {
        return Err(format!("Backup validation failed for {destination_path}"));
    }

    record_audit(
        connection,
        "backup_created",
        None,
        Some("workspace_export"),
        Some(workspace_id),
        &format!("Backup saved to {destination_path}"),
    )?;

    Ok(export)
}

/// Validate backup integrity
pub fn validate_backup(path: &str) -> Result<bool, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&content);
    match parsed {
        Ok(json) => {
            let valid = json.get("version").is_some()
                && json.get("workspace_id").is_some()
                && json.get("exported_at").is_some();
            Ok(valid)
        }
        Err(_) => Ok(false),
    }
}

/// Differential backup: only export data modified since last backup.
/// Returns the path to the differential backup file.
pub fn differential_backup(
    connection: &Connection,
    workspace_id: &str,
    destination_path: &str,
    since: &str,
) -> Result<String, String> {
    let destination = std::path::PathBuf::from(destination_path);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Export only items modified since the given timestamp
    let mut sources = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT id, workspace_id, kind, title, uri, status, created_at FROM sources WHERE workspace_id = ?1 AND created_at >= ?2"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![workspace_id, since], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "workspace_id": row.get::<_, String>(1)?,
                "kind": row.get::<_, String>(2)?,
                "title": row.get::<_, String>(3)?,
                "uri": row.get::<_, Option<String>>(4)?,
                "status": row.get::<_, String>(5)?,
                "created_at": row.get::<_, String>(6)?,
            }))
        }) {
            sources = rows.filter_map(|r| r.ok()).collect();
        }
    }

    let mut meetings = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT id, workspace_id, title, started_at, duration_seconds, recording_path, status, created_at FROM meetings WHERE workspace_id = ?1 AND created_at >= ?2"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![workspace_id, since], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "workspace_id": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "started_at": row.get::<_, Option<String>>(3)?,
                "duration_seconds": row.get::<_, Option<i64>>(4)?,
                "recording_path": row.get::<_, Option<String>>(5)?,
                "status": row.get::<_, String>(6)?,
                "created_at": row.get::<_, String>(7)?,
            }))
        }) {
            meetings = rows.filter_map(|r| r.ok()).collect();
        }
    }

    let mut notes = Vec::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT id, workspace_id, title, content, created_at, updated_at FROM notes WHERE workspace_id = ?1 AND updated_at >= ?2"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![workspace_id, since], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "workspace_id": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "content": row.get::<_, String>(3)?,
                "created_at": row.get::<_, String>(4)?,
                "updated_at": row.get::<_, String>(5)?,
            }))
        }) {
            notes = rows.filter_map(|r| r.ok()).collect();
        }
    }

    let export = serde_json::json!({
        "version": 2,
        "backup_type": "differential",
        "since": since,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "workspace_id": workspace_id,
        "sources": sources,
        "meetings": meetings,
        "notes": notes,
    });

    std::fs::write(destination_path, serde_json::to_string_pretty(&export).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    record_audit(
        connection,
        "differential_backup_created",
        None,
        Some("workspace_export"),
        Some(workspace_id),
        &format!("Differential backup since {since} saved to {destination_path} ({} sources, {} meetings, {} notes)", sources.len(), meetings.len(), notes.len()),
    )?;

    Ok(destination_path.to_string())
}

/// Get per-workspace recording retention policy (days to keep recordings).
/// Returns None if using the global default.
pub fn get_workspace_retention(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Option<i64>, String> {
    connection
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = ?1",
            params![format!("retention_{}", workspace_id)],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.to_string()),
        })
}

/// Set per-workspace recording retention policy (days to keep recordings).
pub fn set_workspace_retention(
    connection: &Connection,
    workspace_id: &str,
    retention_days: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![format!("retention_{}", workspace_id), retention_days.to_string()],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        conn
    }

    #[test]
    fn validate_backup_accepts_complete_export() {
        let dir = std::env::temp_dir().join(format!("nao-backup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("backup.json");
        std::fs::write(&path, r#"{"version": 1, "workspace_id": "w1", "exported_at": "2026-08-10T00:00:00Z"}"#).unwrap();
        assert!(validate_backup(path.to_str().unwrap()).unwrap());

        // Missing required keys -> invalid
        std::fs::write(&path, r#"{"version": 1}"#).unwrap();
        assert!(!validate_backup(path.to_str().unwrap()).unwrap());

        // Not JSON -> invalid
        std::fs::write(&path, "not json").unwrap();
        assert!(!validate_backup(path.to_str().unwrap()).unwrap());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn differential_backup_exports_only_recent_data() {
        let conn = setup();
        conn.execute(
            "INSERT INTO sources (id, workspace_id, kind, title, created_at) VALUES ('s1', 'w1', 'note', 'Old', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sources (id, workspace_id, kind, title, created_at) VALUES ('s2', 'w1', 'note', 'New', '2026-08-01T00:00:00Z')",
            [],
        ).unwrap();

        let dir = std::env::temp_dir().join(format!("nao-diff-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("diff.json");
        let out = differential_backup(&conn, "w1", dest.to_str().unwrap(), "2026-06-01T00:00:00Z").unwrap();
        assert_eq!(out, dest.to_str().unwrap());

        let content = std::fs::read_to_string(&dest).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["backup_type"], "differential");
        assert_eq!(json["sources"].as_array().unwrap().len(), 1);
        assert_eq!(json["sources"][0]["id"], "s2");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn workspace_retention_roundtrip() {
        let conn = setup();
        assert!(get_workspace_retention(&conn, "w1").unwrap().is_none());
        set_workspace_retention(&conn, "w1", 30).unwrap();
        assert_eq!(get_workspace_retention(&conn, "w1").unwrap(), Some(30));
        set_workspace_retention(&conn, "w1", 60).unwrap();
        assert_eq!(get_workspace_retention(&conn, "w1").unwrap(), Some(60));
    }
}
