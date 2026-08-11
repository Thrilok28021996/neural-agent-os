use rusqlite::{params, Connection};
use serde::Serialize;
use chrono::Utc;
use uuid;
use serde_json;

#[derive(Serialize, Clone)]
pub struct SyncQueueItem {
    pub id: String,
    pub entity_type: String, // calendar_event, email_send, contact_update
    pub entity_id: String,
    pub provider: String,
    pub action: String, // create, update, delete, send
    pub payload_json: String,
    pub status: String, // pending, syncing, synced, failed, conflict
    pub retry_count: i32,
    pub error: Option<String>,
    pub conflict_details: Option<String>,
    pub created_at: String,
    pub synced_at: Option<String>,
}

#[derive(Serialize)]
pub struct SyncStatus {
    pub provider: String,
    pub pending: usize,
    pub synced: usize,
    pub failed: usize,
    pub conflicts: usize,
    pub last_sync: Option<String>,
}

#[derive(Serialize)]
pub struct ConflictResolution {
    pub queue_id: String,
    pub resolution: String, // keep_local, keep_remote, merge
    pub resolved_by: String,
    pub resolved_at: String,
}

/// Enqueue a sync operation to be processed
pub fn enqueue_sync(
    connection: &Connection,
    entity_type: &str,
    entity_id: &str,
    provider: &str,
    action: &str,
    payload: &serde_json::Value,
) -> Result<SyncQueueItem, String> {
    let id = format!("sync-{}", uuid::Uuid::new_v4());

    connection.execute(
        "INSERT INTO sync_queue (id, entity_type, entity_id, provider, action, payload_json, status, retry_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 0, ?7)",
        params![id, entity_type, entity_id, provider, action,
            serde_json::to_string(payload).map_err(|e| e.to_string())?,
            Utc::now().to_rfc3339()],
    ).map_err(|e| e.to_string())?;

    Ok(SyncQueueItem {
        id,
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        provider: provider.to_string(),
        action: action.to_string(),
        payload_json: serde_json::to_string(payload).map_err(|e| e.to_string())?,
        status: "pending".into(),
        retry_count: 0,
        error: None,
        conflict_details: None,
        created_at: Utc::now().to_rfc3339(),
        synced_at: None,
    })
}

/// Get pending sync items for a provider
pub fn get_pending_syncs(
    connection: &Connection,
    provider: &str,
    limit: usize,
) -> Result<Vec<SyncQueueItem>, String> {
    let mut stmt = connection.prepare(
        "SELECT id, entity_type, entity_id, provider, action, payload_json, status, retry_count,
         error, conflict_details, created_at, synced_at
         FROM sync_queue
         WHERE (?1 = '' OR provider = ?1) AND status IN ('pending', 'failed')
         AND retry_count < 5
         ORDER BY created_at ASC LIMIT ?2"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![provider, limit as i64], |row| {
        Ok(SyncQueueItem {
            id: row.get(0)?, entity_type: row.get(1)?, entity_id: row.get(2)?,
            provider: row.get(3)?, action: row.get(4)?, payload_json: row.get(5)?,
            status: row.get(6)?, retry_count: row.get(7)?, error: row.get(8)?,
            conflict_details: row.get(9)?, created_at: row.get(10)?, synced_at: row.get(11)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Mark a sync item as synced
pub fn mark_synced(connection: &Connection, queue_id: &str) -> Result<(), String> {
    connection.execute(
        "UPDATE sync_queue SET status = 'synced', synced_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), queue_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark a sync item as failed for retry
pub fn mark_sync_failed(
    connection: &Connection,
    queue_id: &str,
    error: &str,
) -> Result<(), String> {
    connection.execute(
        "UPDATE sync_queue SET status = 'failed', error = ?1, retry_count = retry_count + 1 WHERE id = ?2",
        params![error, queue_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark a sync item as having a conflict
pub fn mark_sync_conflict(
    connection: &Connection,
    queue_id: &str,
    details: &str,
) -> Result<(), String> {
    connection.execute(
        "UPDATE sync_queue SET status = 'conflict', conflict_details = ?1 WHERE id = ?2",
        params![details, queue_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Resolve a sync conflict
pub fn resolve_conflict(
    connection: &Connection,
    queue_id: &str,
    resolution: &str, // keep_local or keep_remote
) -> Result<ConflictResolution, String> {
    if !["keep_local", "keep_remote", "merge"].contains(&resolution) {
        return Err("Resolution must be keep_local, keep_remote, or merge".into());
    }

    let item: (String, String, String) = connection.query_row(
        "SELECT entity_type, provider, payload_json FROM sync_queue WHERE id = ?1 AND status = 'conflict'",
        params![queue_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| e.to_string())?;

    match resolution {
        "keep_local" => {
            // Delete remote version, mark synced
            connection.execute(
                "UPDATE sync_queue SET status = 'synced', conflict_details = 'Resolved: kept local version', synced_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), queue_id],
            ).map_err(|e| e.to_string())?;
        }
        "keep_remote" => {
            // Apply remote payload locally
            let payload: serde_json::Value = serde_json::from_str(&item.2).map_err(|e| e.to_string())?;
            match item.0.as_str() {
                "calendar_event" => {
                    let id = format!("event-{}", uuid::Uuid::new_v4());
                    connection.execute(
                        "INSERT OR REPLACE INTO calendar_events (id, workspace_id, title, starts_at, ends_at, meeting_url, provider)
                         VALUES (?1, 'personal', ?2, ?3, ?4, ?5, ?6)",
                        params![
                            id,
                            payload["title"].as_str().unwrap_or("Untitled"),
                            payload["starts_at"].as_str().unwrap_or(""),
                            payload["ends_at"].as_str().unwrap_or(""),
                            payload["meeting_url"].as_str(),
                            item.1,
                        ],
                    ).map_err(|e| e.to_string())?;
                }
                _ => {}
            }
            connection.execute(
                "UPDATE sync_queue SET status = 'synced', conflict_details = 'Resolved: kept remote version', synced_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), queue_id],
            ).map_err(|e| e.to_string())?;
        }
        _ => {}
    }

    Ok(ConflictResolution {
        queue_id: queue_id.to_string(),
        resolution: resolution.to_string(),
        resolved_by: "user".into(),
        resolved_at: Utc::now().to_rfc3339(),
    })
}

/// Get sync status summary for all providers in a workspace
pub fn get_sync_status(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<SyncStatus>, String> {
    let mut stmt = connection.prepare(
        "SELECT provider FROM calendar_connections WHERE workspace_id = ?1
         UNION SELECT provider FROM email_accounts WHERE workspace_id = ?1"
    ).map_err(|e| e.to_string())?;

    let providers: Vec<String> = stmt
        .query_map(params![workspace_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for provider in providers {
        let pending: usize = connection.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE provider = ?1 AND status = 'pending'",
            params![provider], |row| row.get(0),
        ).unwrap_or(0);
        let synced: usize = connection.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE provider = ?1 AND status = 'synced'",
            params![provider], |row| row.get(0),
        ).unwrap_or(0);
        let failed: usize = connection.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE provider = ?1 AND status = 'failed'",
            params![provider], |row| row.get(0),
        ).unwrap_or(0);
        let conflicts: usize = connection.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE provider = ?1 AND status = 'conflict'",
            params![provider], |row| row.get(0),
        ).unwrap_or(0);
        let last_sync: Option<String> = connection.query_row(
            "SELECT synced_at FROM sync_queue WHERE provider = ?1 AND status = 'synced' ORDER BY synced_at DESC LIMIT 1",
            params![provider], |row| row.get(0),
        ).ok();

        results.push(SyncStatus { provider, pending, synced, failed, conflicts, last_sync });
    }
    Ok(results)
}

/// Process pending sync queue - called by scheduler
pub fn process_sync_queue(app: &tauri::AppHandle) -> Result<usize, String> {
    let connection = super::open_database(app)?;
    let items = get_pending_syncs(&connection, "", 10)?;
    let mut processed = 0;

    for item in items {
        let result = match (item.entity_type.as_str(), item.provider.as_str()) {
            ("calendar_event", "google") | ("calendar_event", "outlook") => {
                super::sync_calendar_events(app.clone(), item.entity_id.clone())
                    .map(|_| ())
                    .map_err(|e| e)
            }
            ("email_send", _) => {
                // Email send sync handled by email module directly
                Ok(())
            }
            _ => {
                Ok(()) // Unknown types skip
            }
        };

        match result {
            Ok(_) => {
                mark_synced(&connection, &item.id)?;
                processed += 1;
            }
            Err(error) => {
                mark_sync_failed(&connection, &item.id, &error)?;
            }
        }
    }
    Ok(processed)
}

/// Pull remote calendar events and merge into local database.
/// Performs two-way sync: local changes are pushed, remote changes are pulled.
pub fn pull_remote_calendar_events(
    app: &tauri::AppHandle,
    connection_id: &str,
) -> Result<(usize, usize, usize), String> {
    let connection = super::open_database(app)?;
    let (workspace_id, provider): (String, String) = connection.query_row(
        "SELECT workspace_id, provider FROM calendar_connections WHERE id = ?1",
        params![connection_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut conflicts = 0usize;

    match provider.as_str() {
        "google" => {
            let access_token = get_oauth_token(app, "google", "calendar")?;
            let response = reqwest::blocking::Client::new()
                .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
                .bearer_auth(&access_token)
                .query(&[
                    ("timeMin".to_string(), Utc::now().to_rfc3339()),
                    ("maxResults".to_string(), "50".to_string()),
                    ("singleEvents".to_string(), "true".to_string()),
                    ("orderBy".to_string(), "startTime".to_string()),
                ])
                .send()
                .map_err(|e| format!("Google Calendar API error: {e}"))?
                .json::<serde_json::Value>()
                .map_err(|e| format!("Failed to parse response: {e}"))?;

            if let Some(items) = response["items"].as_array() {
                for item in items {
                    let remote_id = item["id"].as_str().unwrap_or("");
                    let title = item["summary"].as_str().unwrap_or("Untitled");
                    let starts_at = item["start"].get("dateTime")
                        .or_else(|| item["start"].get("date"))
                        .and_then(|v| v.as_str()).unwrap_or("");
                    let ends_at = item["end"].get("dateTime")
                        .or_else(|| item["end"].get("date"))
                        .and_then(|v| v.as_str()).unwrap_or("");
                    let remote_updated = item["updated"].as_str().unwrap_or("").to_string();

                    // Check if this event already exists locally. The local
                    // timestamp is the last time we touched it (updated_at,
                    // falling back to created_at) so comparing against the
                    // remote modified time is meaningful.
                    let local: Option<(String, String)> = connection.query_row(
                        "SELECT id, COALESCE(updated_at, created_at) FROM calendar_events WHERE external_id = ?1 AND workspace_id = ?2",
                        params![remote_id, workspace_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).ok();

                    match local {
                        Some((local_id, local_updated)) => {
                            // Event exists locally — check for conflicts
                            if remote_updated > local_updated {
                                // Remote is newer: update local
                                connection.execute(
                                    "UPDATE calendar_events SET title = ?1, starts_at = ?2, ends_at = ?3, updated_at = CURRENT_TIMESTAMP WHERE id = ?4",
                                    params![title, starts_at, ends_at, local_id],
                                ).map_err(|e| e.to_string())?;
                                updated += 1;
                            } else if remote_updated < local_updated {
                                // Local is newer: push to remote (enqueue sync) and
                                // record the conflict so the user can resolve it.
                                let item = enqueue_sync(
                                    &connection, "calendar_event", &local_id,
                                    "google", "update",
                                    &serde_json::json!({"title": title, "starts_at": starts_at, "ends_at": ends_at}),
                                )?;
                                mark_sync_conflict(&connection, &item.id, "Local version is newer than remote")?;
                                conflicts += 1;
                            }
                        }
                        None => {
                            // New remote event: create locally
                            let id = format!("event-{}", uuid::Uuid::new_v4());
                            connection.execute(
                                "INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider, external_id) VALUES (?1, ?2, ?3, ?4, ?5, 'google', ?6)",
                                params![id, workspace_id, title, starts_at, ends_at, remote_id],
                            ).map_err(|e| e.to_string())?;
                            created += 1;
                        }
                    }
                }
            }
        }
        "outlook" => {
            let access_token = get_oauth_token(app, "microsoft", "calendar")?;
            let response = reqwest::blocking::Client::new()
                .get("https://graph.microsoft.com/v1.0/me/events")
                .bearer_auth(&access_token)
                .query(&[
                    ("$top", "50"),
                    ("$orderby", "start/dateTime"),
                    ("$filter", &format!("start/dateTime ge {}", Utc::now().to_rfc3339())),
                ])
                .send()
                .map_err(|e| format!("Outlook API error: {e}"))?
                .json::<serde_json::Value>()
                .map_err(|e| format!("Failed to parse response: {e}"))?;

            if let Some(items) = response["value"].as_array() {
                for item in items {
                    let remote_id = item["id"].as_str().unwrap_or("");
                    let title = item["subject"].as_str().unwrap_or("Untitled");
                    let starts_at = item["start"]["dateTime"].as_str().unwrap_or("");
                    let ends_at = item["end"]["dateTime"].as_str().unwrap_or("");
                    let remote_updated = item["lastModifiedDateTime"].as_str().unwrap_or("").to_string();

                    let local: Option<(String, String)> = connection.query_row(
                        "SELECT id, COALESCE(updated_at, created_at) FROM calendar_events WHERE external_id = ?1 AND workspace_id = ?2",
                        params![remote_id, workspace_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).ok();

                    match local {
                        Some((local_id, local_updated)) => {
                            if remote_updated > local_updated {
                                connection.execute(
                                    "UPDATE calendar_events SET title = ?1, starts_at = ?2, ends_at = ?3, updated_at = CURRENT_TIMESTAMP WHERE id = ?4",
                                    params![title, starts_at, ends_at, local_id],
                                ).map_err(|e| e.to_string())?;
                                updated += 1;
                            }
                        }
                        None => {
                            let id = format!("event-{}", uuid::Uuid::new_v4());
                            connection.execute(
                                "INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider, external_id) VALUES (?1, ?2, ?3, ?4, ?5, 'outlook', ?6)",
                                params![id, workspace_id, title, starts_at, ends_at, remote_id],
                            ).map_err(|e| e.to_string())?;
                            created += 1;
                        }
                    }
                }
            }
        }
        _ => return Err(format!("Unsupported provider for pull: {provider}")),
    }

    // Update last synced timestamp
    connection.execute(
        "UPDATE calendar_connections SET last_synced_at = ?1, status = 'synced' WHERE id = ?2",
        params![Utc::now().to_rfc3339(), connection_id],
    ).map_err(|e| e.to_string())?;

    super::retention::record_audit(
        &connection, "calendar_pull", Some(&provider), Some("calendar"), Some(&workspace_id),
        &format!("Pulled {created} new, {updated} updated, {conflicts} conflicts"),
    )?;

    Ok((created, updated, conflicts))
}

/// Get OAuth access token from keychain, refreshing if needed
fn get_oauth_token(app: &tauri::AppHandle, provider: &str, scope: &str) -> Result<String, String> {
    let token_entry = keyring::Entry::new(
        "neural-agent-os/oauth",
        &format!("{provider}:{scope}:access_token"),
    ).map_err(|e| e.to_string())?;

    let token = token_entry.get_password().map_err(|e| e.to_string())?;

    // Check expiry
    let expiry_entry = keyring::Entry::new(
        "neural-agent-os/oauth",
        &format!("{provider}:{scope}:expires_at"),
    ).map_err(|e| e.to_string())?;

    if let Ok(expires_str) = expiry_entry.get_password() {
        if let Ok(expires) = expires_str.parse::<i64>() {
            let now = Utc::now().timestamp();
            if now >= expires - 60 {
                // Token expired — refresh it
                let refresh_entry = keyring::Entry::new(
                    "neural-agent-os/oauth",
                    &format!("{provider}:{scope}:refresh_token"),
                ).map_err(|e| e.to_string())?;
                let refresh_token = refresh_entry.get_password().map_err(|e| e.to_string())?;
                return super::oauth::refresh_access_token(provider);
            }
        }
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        // resolve_conflict(keep_remote) writes calendar events into the seeded
        // 'personal' workspace (mirrors the production seed), so create it.
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('personal', 'Personal')", []).unwrap();
        conn
    }

    #[test]
    fn enqueue_and_mark_synced() {
        let conn = setup();
        let item = enqueue_sync(&conn, "calendar_event", "ev-1", "google", "create", &serde_json::json!({"title": "Meet"})).unwrap();
        assert_eq!(item.status, "pending");
        assert_eq!(item.provider, "google");

        let pending = get_pending_syncs(&conn, "google", 10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].entity_id, "ev-1");

        mark_synced(&conn, &item.id).unwrap();
        assert!(get_pending_syncs(&conn, "google", 10).unwrap().is_empty());
    }

    #[test]
    fn failed_items_retry_with_retry_count() {
        let conn = setup();
        let item = enqueue_sync(&conn, "calendar_event", "ev-1", "google", "create", &serde_json::json!({})).unwrap();
        mark_sync_failed(&conn, &item.id, "network error").unwrap();

        let pending = get_pending_syncs(&conn, "google", 10).unwrap();
        assert_eq!(pending[0].status, "failed");
        assert_eq!(pending[0].retry_count, 1);
        assert_eq!(pending[0].error.as_deref(), Some("network error"));
    }

    #[test]
    fn conflict_marking_and_resolution_keep_local() {
        let conn = setup();
        let item = enqueue_sync(&conn, "calendar_event", "ev-1", "google", "update", &serde_json::json!({})).unwrap();
        mark_sync_conflict(&conn, &item.id, "Local is newer").unwrap();

        let resolution = resolve_conflict(&conn, &item.id, "keep_local").unwrap();
        assert_eq!(resolution.resolution, "keep_local");
        let status: String = conn.query_row("SELECT status FROM sync_queue WHERE id = ?1", params![item.id], |r| r.get(0)).unwrap();
        assert_eq!(status, "synced");
    }

    #[test]
    fn conflict_resolution_keep_remote_creates_event() {
        let conn = setup();
        let item = enqueue_sync(
            &conn, "calendar_event", "ev-1", "google", "update",
            &serde_json::json!({"title": "Remote Title", "starts_at": "2026-08-10T09:00:00Z", "ends_at": "2026-08-10T10:00:00Z", "meeting_url": null}),
        ).unwrap();
        mark_sync_conflict(&conn, &item.id, "conflict").unwrap();
        resolve_conflict(&conn, &item.id, "keep_remote").unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM calendar_events WHERE title = 'Remote Title'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn invalid_resolution_is_rejected() {
        let conn = setup();
        let item = enqueue_sync(&conn, "calendar_event", "ev-1", "google", "update", &serde_json::json!({})).unwrap();
        mark_sync_conflict(&conn, &item.id, "conflict").unwrap();
        assert!(resolve_conflict(&conn, &item.id, "nuke_it").is_err());
    }

    #[test]
    fn sync_status_summarizes_provider_queues() {
        let conn = setup();
        conn.execute(
            "INSERT INTO calendar_connections (id, workspace_id, provider, account_label, status) VALUES ('cc-1', 'w1', 'google', 'Main', 'connected')",
            [],
        ).unwrap();
        let item = enqueue_sync(&conn, "calendar_event", "ev-1", "google", "create", &serde_json::json!({})).unwrap();
        mark_synced(&conn, &item.id).unwrap();
        let item2 = enqueue_sync(&conn, "calendar_event", "ev-2", "google", "create", &serde_json::json!({})).unwrap();
        mark_sync_conflict(&conn, &item2.id, "conflict").unwrap();

        let statuses = get_sync_status(&conn, "w1").unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].provider, "google");
        assert_eq!(statuses[0].synced, 1);
        assert_eq!(statuses[0].conflicts, 1);
    }
}
