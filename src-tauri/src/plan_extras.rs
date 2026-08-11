// ── Plan-completion extras: connection-based implementations for the features
// added in the Aug 11 verification pass (token limits, memories, duplicates,
// email search, transcript editing, triage, event reminders, bot runs,
// context export). Kept separate from lib.rs so each piece is unit-testable
// against an in-memory database.
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct MemoryRecord {
    pub id: String,
    pub workspace_id: String,
    pub content: String,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct DuplicateGroup {
    pub content_hash: String,
    pub source_count: usize,
    pub sources: Vec<serde_json::Value>,
}

#[derive(Serialize, Clone)]
pub struct EmailHit {
    pub id: String,
    pub account_id: String,
    pub subject: String,
    pub sender: Option<String>,
    pub recipients: Option<String>,
    pub body: Option<String>,
    pub received_at: Option<String>,
    pub score: Option<f32>,
}

#[derive(Serialize)]
pub struct TriageItem {
    pub email_id: String,
    pub subject: String,
    pub sender: String,
    pub received_at: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct BotRunRecord {
    pub id: String,
    pub workspace_id: String,
    pub bot_id: String,
    pub status: String,
    pub message: Option<String>,
    pub meeting_id: Option<String>,
    pub platform: String,
    pub reported_at: String,
}

// ── Token limits ────────────────────────────────────────────────────────────
pub fn set_token_limit(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    limit_tokens: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO token_limits (workspace_id, capability, limit_tokens) VALUES (?1, ?2, ?3)
             ON CONFLICT(workspace_id, capability) DO UPDATE SET limit_tokens = excluded.limit_tokens",
            params![workspace_id, capability, limit_tokens],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_token_limit(connection: &Connection, workspace_id: &str, capability: &str) -> Result<Option<i64>, String> {
    connection
        .query_row(
            "SELECT limit_tokens FROM token_limits WHERE workspace_id = ?1 AND capability = ?2",
            params![workspace_id, capability],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.to_string()),
        })
}

pub fn list_token_limits(connection: &Connection, workspace_id: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = connection
        .prepare("SELECT capability, limit_tokens FROM token_limits WHERE workspace_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![workspace_id], |row| {
            Ok(serde_json::json!({ "capability": row.get::<_, String>(0)?, "limit_tokens": row.get::<_, i64>(1)? }))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Memories ────────────────────────────────────────────────────────────────
fn query_memory(connection: &Connection, id: &str) -> Result<MemoryRecord, String> {
    connection.query_row(
        "SELECT id, workspace_id, content, source_kind, source_id, created_at, updated_at FROM memories WHERE id = ?1",
        params![id],
        |row| Ok(MemoryRecord {
            id: row.get(0)?, workspace_id: row.get(1)?, content: row.get(2)?,
            source_kind: row.get(3)?, source_id: row.get(4)?,
            created_at: row.get(5)?, updated_at: row.get(6)?,
        }),
    ).map_err(|e| e.to_string())
}

pub fn create_memory(
    connection: &Connection,
    workspace_id: &str,
    content: &str,
    source_kind: Option<&str>,
    source_id: Option<&str>,
) -> Result<MemoryRecord, String> {
    if content.trim().is_empty() {
        return Err("Memory content cannot be empty".into());
    }
    let id = format!("memory-{}", uuid::Uuid::new_v4());
    connection.execute(
        "INSERT INTO memories (id, workspace_id, content, source_kind, source_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, workspace_id, content, source_kind, source_id],
    ).map_err(|e| e.to_string())?;
    query_memory(connection, &id)
}

pub fn list_memories(connection: &Connection, workspace_id: &str) -> Result<Vec<MemoryRecord>, String> {
    let mut stmt = connection.prepare(
        "SELECT id, workspace_id, content, source_kind, source_id, created_at, updated_at FROM memories WHERE workspace_id = ?1 ORDER BY created_at DESC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![workspace_id], |row| Ok(MemoryRecord {
        id: row.get(0)?, workspace_id: row.get(1)?, content: row.get(2)?,
        source_kind: row.get(3)?, source_id: row.get(4)?, created_at: row.get(5)?, updated_at: row.get(6)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn search_memories(connection: &Connection, workspace_id: &str, query: &str) -> Result<Vec<MemoryRecord>, String> {
    let pattern = format!("%{}%", query);
    let mut stmt = connection.prepare(
        "SELECT id, workspace_id, content, source_kind, source_id, created_at, updated_at FROM memories WHERE workspace_id = ?1 AND content LIKE ?2 ORDER BY created_at DESC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![workspace_id, pattern], |row| Ok(MemoryRecord {
        id: row.get(0)?, workspace_id: row.get(1)?, content: row.get(2)?,
        source_kind: row.get(3)?, source_id: row.get(4)?, created_at: row.get(5)?, updated_at: row.get(6)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn delete_memory(connection: &Connection, memory_id: &str) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM memories WHERE id = ?1", params![memory_id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Memory not found".into());
    }
    Ok(())
}

// ── Duplicate detection ─────────────────────────────────────────────────────
pub fn find_duplicates(connection: &Connection, workspace_id: &str) -> Result<Vec<DuplicateGroup>, String> {
    let mut stmt = connection.prepare(
        "SELECT content_hash FROM sources WHERE workspace_id = ?1 AND content_hash IS NOT NULL GROUP BY content_hash HAVING COUNT(*) > 1",
    ).map_err(|e| e.to_string())?;
    let hashes: Vec<String> = stmt
        .query_map(params![workspace_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut groups = Vec::new();
    for hash in hashes {
        let mut stmt = connection.prepare(
            "SELECT id, title, uri, created_at FROM sources WHERE workspace_id = ?1 AND content_hash = ?2 ORDER BY created_at ASC",
        ).map_err(|e| e.to_string())?;
        let sources: Vec<serde_json::Value> = stmt
            .query_map(params![workspace_id, hash], |row| Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "uri": row.get::<_, Option<String>>(2)?,
                "created_at": row.get::<_, String>(3)?,
            })))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        groups.push(DuplicateGroup { content_hash: hash, source_count: sources.len(), sources });
    }
    Ok(groups)
}

// ── Email keyword search ────────────────────────────────────────────────────
pub fn search_emails(connection: &Connection, workspace_id: &str, query: &str) -> Result<Vec<EmailHit>, String> {
    let match_query = query.split_whitespace().collect::<Vec<_>>().join(" OR ");
    let mut stmt = connection.prepare(
        "SELECT e.id, e.account_id, e.subject, e.sender, e.recipients, e.body, e.received_at
         FROM emails e JOIN email_accounts ea ON ea.id = e.account_id
         JOIN email_search es ON es.email_id = e.id
         WHERE ea.workspace_id = ?1 AND es.workspace_id = ?1 AND email_search MATCH ?2
         ORDER BY rank LIMIT 25",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![workspace_id, match_query], |row| Ok(EmailHit {
        id: row.get(0)?, account_id: row.get(1)?, subject: row.get(2)?,
        sender: row.get(3)?, recipients: row.get(4)?, body: row.get(5)?,
        received_at: row.get(6)?, score: None,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Transcript editing ──────────────────────────────────────────────────────
pub fn update_transcript_segment_text(connection: &Connection, segment_id: &str, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Segment text cannot be empty".into());
    }
    let changed = connection
        .execute("UPDATE transcript_segments SET text = ?1 WHERE id = ?2", params![text, segment_id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Transcript segment not found".into());
    }
    Ok(())
}

// ── Email triage ────────────────────────────────────────────────────────────
pub fn email_triage(connection: &Connection, workspace_id: &str) -> Result<Vec<TriageItem>, String> {
    let mut stmt = connection.prepare(
        "SELECT e.id, e.subject, e.sender, e.received_at FROM emails e
         JOIN email_accounts ea ON ea.id = e.account_id
         WHERE ea.workspace_id = ?1 AND e.received_at >= datetime('now', '-7 days')
         ORDER BY e.received_at DESC LIMIT 50",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![workspace_id], |row| Ok((
        row.get::<_, String>(0)?, row.get::<_, String>(1)?,
        row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?,
    ))).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows {
        let (email_id, subject, sender, received_at) = row.map_err(|e| e.to_string())?;
        let lower = subject.to_lowercase();
        let reason = if lower.contains("invoice") || lower.contains("payment") { "Review payment".to_string() }
            else if lower.contains("meeting") || lower.contains("calendar") || lower.contains("invite") { "Meeting-related".to_string() }
            else if lower.contains("deadline") || lower.contains("due") || lower.contains("urgent") { "Time-sensitive".to_string() }
            else if lower.contains("report") || lower.contains("update") || lower.contains("follow") { "Needs follow-up".to_string() }
            else { "Inbox review".to_string() };
        items.push(TriageItem { email_id, subject, sender: sender.unwrap_or_default(), received_at: received_at.unwrap_or_default(), reason });
    }
    Ok(items)
}

// ── Calendar-event reminders ────────────────────────────────────────────────
pub fn schedule_event_reminder(
    connection: &Connection,
    workspace_id: &str,
    event_id: &str,
    title: &str,
    scheduled_at: &str,
) -> Result<crate::ReminderRecord, String> {
    if chrono::DateTime::parse_from_rfc3339(scheduled_at).is_err() {
        return Err("scheduled_at must be an RFC3339 timestamp".into());
    }
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE id = ?1 AND workspace_id = ?2)",
        params![event_id, workspace_id], |row| row.get(0),
    ).unwrap_or(false);
    if !exists {
        return Err("Calendar event not found in this workspace".into());
    }
    let id = format!("reminder-{}", uuid::Uuid::new_v4());
    connection.execute(
        "INSERT INTO reminders (id, workspace_id, calendar_event_id, title, scheduled_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, workspace_id, event_id, title, scheduled_at],
    ).map_err(|e| e.to_string())?;
    connection.query_row(
        "SELECT id, workspace_id, task_id, calendar_event_id, title, scheduled_at, status FROM reminders WHERE id = ?1",
        params![id],
        |row| Ok(crate::ReminderRecord {
            id: row.get(0)?, workspace_id: row.get(1)?, task_id: row.get(2)?,
            calendar_event_id: row.get(3)?, title: row.get(4)?, scheduled_at: row.get(5)?, status: row.get(6)?,
        }),
    ).map_err(|e| e.to_string())
}

// ── Teams-bot run history ───────────────────────────────────────────────────
pub fn record_bot_run(
    connection: &Connection,
    workspace_id: &str,
    bot_id: &str,
    status: &str,
    message: Option<&str>,
    meeting_id: Option<&str>,
) -> Result<(), String> {
    connection.execute(
        "INSERT INTO bot_runs (id, workspace_id, bot_id, status, message, meeting_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![format!("bot-run-{}", uuid::Uuid::new_v4()), workspace_id, bot_id, status, message, meeting_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_bot_runs(connection: &Connection, workspace_id: &str) -> Result<Vec<BotRunRecord>, String> {
    let mut stmt = connection.prepare(
        "SELECT id, workspace_id, bot_id, status, message, meeting_id, platform, reported_at FROM bot_runs WHERE workspace_id = ?1 ORDER BY reported_at DESC LIMIT 50",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![workspace_id], |row| Ok(BotRunRecord {
        id: row.get(0)?, workspace_id: row.get(1)?, bot_id: row.get(2)?, status: row.get(3)?,
        message: row.get(4)?, meeting_id: row.get(5)?, platform: row.get(6)?, reported_at: row.get(7)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Context export ──────────────────────────────────────────────────────────
pub fn export_context(connection: &Connection, workspace_id: &str) -> Result<String, String> {
    let ws_name: String = connection.query_row("SELECT name FROM workspaces WHERE id = ?1", params![workspace_id], |row| row.get(0)).unwrap_or_else(|_| workspace_id.to_string());
    let mut out = format!("# Workspace Context: {ws_name}\n\n");
    out.push_str("## Sources\n");
    if let Ok(mut stmt) = connection.prepare("SELECT id, kind, title, uri, status FROM sources WHERE workspace_id = ?1") {
        if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, String>(4)?))) {
            for row in rows {
                if let Ok((id, kind, title, uri, status)) = row {
                    out.push_str(&format!("- [{kind}] {title} ({status}) {} [{id}]\n", uri.unwrap_or_default()));
                }
            }
        }
    }
    out.push_str("\n## Notes\n");
    if let Ok(mut stmt) = connection.prepare("SELECT id, title, content FROM notes WHERE workspace_id = ?1") {
        if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))) {
            for row in rows {
                if let Ok((id, title, content)) = row {
                    out.push_str(&format!("- **{title}** [{id}]: {}\n", content.chars().take(240).collect::<String>()));
                }
            }
        }
    }
    out.push_str("\n## Tasks\n");
    if let Ok(mut stmt) = connection.prepare("SELECT id, title, status, due_at FROM tasks WHERE workspace_id = ?1") {
        if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?))) {
            for row in rows {
                if let Ok((id, title, status, due_at)) = row {
                    out.push_str(&format!("- [ ] {title} ({status}{}) [{id}]\n", due_at.map(|d| format!(", due {d}")).unwrap_or_default()));
                }
            }
        }
    }
    out.push_str("\n## Meetings\n");
    if let Ok(mut stmt) = connection.prepare("SELECT m.id, m.title, m.started_at, (SELECT COUNT(*) FROM transcript_segments ts WHERE ts.meeting_id = m.id) FROM meetings m WHERE m.workspace_id = ?1") {
        if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, i64>(3)?))) {
            for row in rows {
                if let Ok((id, title, started_at, segs)) = row {
                    out.push_str(&format!("- {title} ({} segments) [{id}] {}\n", segs, started_at.unwrap_or_default()));
                }
            }
        }
    }
    out.push_str("\n## Memories\n");
    if let Ok(mut stmt) = connection.prepare("SELECT id, content FROM memories WHERE workspace_id = ?1") {
        if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))) {
            for row in rows {
                if let Ok((id, content)) = row {
                    out.push_str(&format!("- {content} [{id}]\n"));
                }
            }
        }
    }
    out.push_str("\n## Agents\n");
    if let Ok(mut stmt) = connection.prepare("SELECT id, name, schedule, status FROM agents WHERE workspace_id = ?1") {
        if let Ok(rows) = stmt.query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))) {
            for row in rows {
                if let Ok((id, name, schedule, status)) = row {
                    out.push_str(&format!("- {name} [{id}] schedule={schedule} status={status}\n"));
                }
            }
        }
    }
    Ok(out)
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

    fn insert_email(conn: &Connection, id: &str, subject: &str, body: &str, received_at: &str) {
        conn.execute(
            "INSERT INTO email_accounts (id, workspace_id, provider, account_label, status) VALUES ('acct-1', 'w1', 'imap', 'Work', 'connected')",
            [],
        ).unwrap_or_default();
        conn.execute(
            "INSERT INTO emails (id, account_id, subject, body, received_at) VALUES (?1, 'acct-1', ?2, ?3, ?4)",
            params![id, subject, body, received_at],
        ).unwrap();
        conn.execute(
            "INSERT INTO email_search (subject, sender, body, email_id, workspace_id) VALUES (?1, '', ?2, ?3, 'w1')",
            params![subject, body, id],
        ).unwrap();
    }

    #[test]
    fn token_limit_roundtrip_and_upsert() {
        let conn = setup();
        assert!(get_token_limit(&conn, "w1", "chat").unwrap().is_none());
        set_token_limit(&conn, "w1", "chat", 100_000).unwrap();
        assert_eq!(get_token_limit(&conn, "w1", "chat").unwrap(), Some(100_000));
        set_token_limit(&conn, "w1", "chat", 200_000).unwrap(); // upsert
        assert_eq!(get_token_limit(&conn, "w1", "chat").unwrap(), Some(200_000));
        let all = list_token_limits(&conn, "w1").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["capability"], "chat");
        assert_eq!(all[0]["limit_tokens"], 200_000);
    }

    #[test]
    fn memories_crud_and_search() {
        let conn = setup();
        let mem = create_memory(&conn, "w1", "User prefers concise replies", Some("note"), Some("n-1")).unwrap();
        assert_eq!(mem.content, "User prefers concise replies");
        assert_eq!(mem.source_kind.as_deref(), Some("note"));

        let mem2 = create_memory(&conn, "w1", "Prefers local models", None, None).unwrap();
        assert_ne!(mem.id, mem2.id);

        let all = list_memories(&conn, "w1").unwrap();
        assert_eq!(all.len(), 2);

        let hits = search_memories(&conn, "w1", "concise").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, mem.id);

        delete_memory(&conn, &mem.id).unwrap();
        assert_eq!(list_memories(&conn, "w1").unwrap().len(), 1);
        assert!(delete_memory(&conn, &mem.id).is_err()); // already gone

        // Empty content rejected
        assert!(create_memory(&conn, "w1", "   ", None, None).is_err());
    }

    #[test]
    fn duplicate_detection_groups_same_hash() {
        let conn = setup();
        conn.execute(
            "INSERT INTO sources (id, workspace_id, kind, title, content_hash) VALUES ('s1', 'w1', 'note', 'A', 'hash-1'), ('s2', 'w1', 'note', 'A copy', 'hash-1'), ('s3', 'w1', 'note', 'B', 'hash-2')",
            [],
        ).unwrap();

        let groups = find_duplicates(&conn, "w1").unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].content_hash, "hash-1");
        assert_eq!(groups[0].source_count, 2);
        assert_eq!(groups[0].sources.len(), 2);
        assert_eq!(groups[0].sources[0]["title"], "A"); // ordered by created_at
    }

    #[test]
    fn email_keyword_search_finds_matches() {
        let conn = setup();
        insert_email(&conn, "e1", "Project Sync", "meeting agenda for sync", "2026-08-10T09:00:00Z");
        insert_email(&conn, "e2", "Invoice", "payment due", "2026-08-10T10:00:00Z");

        let hits = search_emails(&conn, "w1", "invoice").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subject, "Invoice");

        let hits2 = search_emails(&conn, "w1", "sync").unwrap();
        assert_eq!(hits2.len(), 1);
        assert_eq!(hits2[0].id, "e1");
    }

    #[test]
    fn transcript_segment_text_can_be_edited() {
        let conn = setup();
        conn.execute("INSERT INTO meetings (id, workspace_id, title) VALUES ('m1', 'w1', 'Standup')", []).unwrap();
        conn.execute("INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text) VALUES ('seg-1', 'm1', 0, 5, 'old text')", []).unwrap();

        update_transcript_segment_text(&conn, "seg-1", "corrected text").unwrap();
        let text: String = conn.query_row("SELECT text FROM transcript_segments WHERE id = 'seg-1'", [], |r| r.get(0)).unwrap();
        assert_eq!(text, "corrected text");

        assert!(update_transcript_segment_text(&conn, "seg-999", "x").is_err());
        assert!(update_transcript_segment_text(&conn, "seg-1", "  ").is_err());
    }

    #[test]
    fn email_triage_classifies_by_keywords() {
        let conn = setup();
        insert_email(&conn, "e1", "Invoice #42", "payment due", "2026-08-10T09:00:00Z");
        insert_email(&conn, "e2", "Meeting invite", "calendar", "2026-08-10T10:00:00Z");
        insert_email(&conn, "e3", "Old email", "nothing special", "2026-01-01T00:00:00Z"); // older than 7 days

        let triage = email_triage(&conn, "w1").unwrap();
        assert_eq!(triage.len(), 2); // old email excluded
        assert!(triage.iter().any(|t| t.email_id == "e1" && t.reason == "Review payment"));
        assert!(triage.iter().any(|t| t.email_id == "e2" && t.reason == "Meeting-related"));
    }

    #[test]
    fn event_reminder_requires_existing_event() {
        let conn = setup();
        conn.execute(
            "INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at) VALUES ('ev-1', 'w1', 'Standup', '2026-08-10T09:00:00Z', '2026-08-10T09:30:00Z')",
            [],
        ).unwrap();

        // Missing event rejected
        assert!(schedule_event_reminder(&conn, "w1", "ev-999", "Reminder", "2026-08-11T09:00:00Z").is_err());
        // Invalid timestamp rejected
        assert!(schedule_event_reminder(&conn, "w1", "ev-1", "Reminder", "not-a-date").is_err());

        let reminder = schedule_event_reminder(&conn, "w1", "ev-1", "Prep for standup", "2026-08-11T08:30:00Z").unwrap();
        assert_eq!(reminder.calendar_event_id.as_deref(), Some("ev-1"));
        assert_eq!(reminder.title, "Prep for standup");
        assert_eq!(reminder.task_id, None);
    }

    #[test]
    fn bot_runs_are_recorded_and_listed() {
        let conn = setup();
        record_bot_run(&conn, "w1", "bot-1", "joined", Some("Joined meeting"), Some("m-1")).unwrap();
        record_bot_run(&conn, "w1", "bot-1", "recording", Some("Recording started"), Some("m-1")).unwrap();

        let runs = list_bot_runs(&conn, "w1").unwrap();
        assert_eq!(runs.len(), 2);
        let mut statuses: Vec<&str> = runs.iter().map(|r| r.status.as_str()).collect();
        statuses.sort();
        assert_eq!(statuses, vec!["joined", "recording"]);
        assert!(runs.iter().all(|r| r.bot_id == "bot-1"));
        assert!(runs.iter().all(|r| r.meeting_id.as_deref() == Some("m-1")));
    }

    #[test]
    fn context_export_contains_all_sections() {
        let conn = setup();
        conn.execute("INSERT INTO notes (id, workspace_id, title, content) VALUES ('n1', 'w1', 'Note', 'content here')", []).unwrap();
        conn.execute("INSERT INTO tasks (id, workspace_id, title, status) VALUES ('t1', 'w1', 'Ship', 'open')", []).unwrap();
        create_memory(&conn, "w1", "A durable fact", None, None).unwrap();
        conn.execute("INSERT INTO meetings (id, workspace_id, title) VALUES ('m1', 'w1', 'Standup')", []).unwrap();

        let context = export_context(&conn, "w1").unwrap();
        assert!(context.contains("## Sources"));
        assert!(context.contains("## Notes"));
        assert!(context.contains("Note"));
        assert!(context.contains("## Tasks"));
        assert!(context.contains("Ship"));
        assert!(context.contains("## Meetings"));
        assert!(context.contains("Standup"));
        assert!(context.contains("## Memories"));
        assert!(context.contains("A durable fact"));
        assert!(context.contains("## Agents"));
    }
}
