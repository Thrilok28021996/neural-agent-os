use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
pub struct SourceDeletionPreview {
    pub source_id: String,
    pub source_title: String,
    pub source_kind: String,
    pub chunks: usize,
    pub embeddings: usize,
    pub total_size_estimate: u64,
}

#[derive(Serialize)]
pub struct MeetingDeletionPreview {
    pub meeting_id: String,
    pub meeting_title: String,
    pub transcript_segments: usize,
    pub actions: usize,
    pub summary_exists: bool,
    pub recording_path: Option<String>,
    pub recording_size: u64,
}

#[derive(Serialize)]
pub struct FullDeletionPreview {
    pub workspace_id: String,
    pub workspace_name: String,
    pub sources: Vec<SourceDeletionPreview>,
    pub meetings: Vec<MeetingDeletionPreview>,
    pub notes_count: usize,
    pub emails_count: usize,
    pub total_derived_items: usize,
    pub total_storage_impact_bytes: u64,
}

/// Preview what would be deleted when removing a single source
pub fn preview_source_deletion(
    connection: &Connection,
    source_id: &str,
) -> Result<SourceDeletionPreview, String> {
    log::debug!("deletion::preview_source_deletion: enter");
    let (title, kind): (String, String) = connection
        .query_row(
            "SELECT title, kind FROM sources WHERE id = ?1",
            params![source_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let chunks: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM source_chunks WHERE source_id = ?1",
            params![source_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let embeddings: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM chunk_embeddings WHERE chunk_id IN (SELECT id FROM source_chunks WHERE source_id = ?1)",
            params![source_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Estimate storage impact from chunk content length
    let total_size: u64 = connection
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM source_chunks WHERE source_id = ?1",
            params![source_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(SourceDeletionPreview {
        source_id: source_id.to_string(),
        source_title: title,
        source_kind: kind,
        chunks,
        embeddings,
        total_size_estimate: total_size,
    })
}

/// Preview what would be deleted when removing a meeting
pub fn preview_meeting_deletion(
    connection: &Connection,
    meeting_id: &str,
) -> Result<MeetingDeletionPreview, String> {
    log::debug!("deletion::preview_meeting_deletion: enter");
    let (title, recording_path): (String, Option<String>) = connection
        .query_row(
            "SELECT title, recording_path FROM meetings WHERE id = ?1",
            params![meeting_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let segments: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM transcript_segments WHERE meeting_id = ?1",
            params![meeting_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let actions: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM meeting_actions WHERE meeting_id = ?1",
            params![meeting_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let summary_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM meeting_summaries WHERE meeting_id = ?1)",
            params![meeting_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    let recording_size = recording_path
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(MeetingDeletionPreview {
        meeting_id: meeting_id.to_string(),
        meeting_title: title,
        transcript_segments: segments,
        actions,
        summary_exists,
        recording_path,
        recording_size,
    })
}

/// Full preview for a workspace including all individual items
pub fn full_deletion_preview(
    connection: &Connection,
    workspace_id: &str,
) -> Result<FullDeletionPreview, String> {
    log::info!("deletion::full_deletion_preview: enter");
    let workspace_name: String = connection
        .query_row(
            "SELECT name FROM workspaces WHERE id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Sources
    let mut src_stmt = connection
        .prepare("SELECT id FROM sources WHERE workspace_id = ?1")
        .map_err(|e| e.to_string())?;
    let source_ids: Vec<String> = src_stmt
        .query_map(params![workspace_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut sources = Vec::new();
    for sid in &source_ids {
        if let Ok(preview) = preview_source_deletion(&connection, sid) {
            sources.push(preview);
        }
    }

    // Meetings
    let mut meet_stmt = connection
        .prepare("SELECT id FROM meetings WHERE workspace_id = ?1")
        .map_err(|e| e.to_string())?;
    let meeting_ids: Vec<String> = meet_stmt
        .query_map(params![workspace_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut meetings = Vec::new();
    for mid in &meeting_ids {
        if let Ok(preview) = preview_meeting_deletion(&connection, mid) {
            meetings.push(preview);
        }
    }

    let notes_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let emails_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM emails WHERE account_id IN (SELECT id FROM email_accounts WHERE workspace_id = ?1)",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_derived = sources.iter().map(|s| s.chunks + s.embeddings).sum::<usize>()
        + meetings.iter().map(|m| m.transcript_segments + m.actions).sum::<usize>()
        + notes_count
        + emails_count;

    let total_storage: u64 = sources.iter().map(|s| s.total_size_estimate).sum::<u64>()
        + meetings.iter().map(|m| m.recording_size).sum::<u64>();

    Ok(FullDeletionPreview {
        workspace_id: workspace_id.to_string(),
        workspace_name,
        sources,
        meetings,
        notes_count,
        emails_count,
        total_derived_items: total_derived,
        total_storage_impact_bytes: total_storage,
    })
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
    fn source_deletion_preview_counts_derived_data() {
        let conn = setup();
        conn.execute("INSERT INTO sources (id, workspace_id, kind, title) VALUES ('s1', 'w1', 'document', 'Doc')", []).unwrap();
        conn.execute("INSERT INTO source_chunks (id, source_id, chunk_index, content) VALUES ('c1', 's1', 0, 'hello world'), ('c2', 's1', 1, 'foo bar baz')", []).unwrap();
        conn.execute("INSERT INTO chunk_embeddings (chunk_id, model, vector_json) VALUES ('c1', 'm', '[1.0]'), ('c2', 'm', '[2.0]')", []).unwrap();

        let preview = preview_source_deletion(&conn, "s1").unwrap();
        assert_eq!(preview.source_title, "Doc");
        assert_eq!(preview.chunks, 2);
        assert_eq!(preview.embeddings, 2);
        assert!(preview.total_size_estimate > 0);
    }

    #[test]
    fn meeting_deletion_preview_reports_related_items() {
        let conn = setup();
        conn.execute("INSERT INTO meetings (id, workspace_id, title) VALUES ('m1', 'w1', 'Standup')", []).unwrap();
        conn.execute("INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text) VALUES ('t1', 'm1', 0, 1, 'hi')", []).unwrap();
        conn.execute("INSERT INTO meeting_actions (id, meeting_id, title) VALUES ('a1', 'm1', 'Follow up')", []).unwrap();
        conn.execute("INSERT INTO meeting_summaries (meeting_id, model, summary) VALUES ('m1', 'm', 'sum')", []).unwrap();

        let preview = preview_meeting_deletion(&conn, "m1").unwrap();
        assert_eq!(preview.transcript_segments, 1);
        assert_eq!(preview.actions, 1);
        assert!(preview.summary_exists);
    }

    #[test]
    fn full_deletion_preview_aggregates_workspace() {
        let conn = setup();
        conn.execute("INSERT INTO sources (id, workspace_id, kind, title) VALUES ('s1', 'w1', 'document', 'Doc')", []).unwrap();
        conn.execute("INSERT INTO meetings (id, workspace_id, title) VALUES ('m1', 'w1', 'Standup')", []).unwrap();
        conn.execute("INSERT INTO notes (id, workspace_id, title, content) VALUES ('n1', 'w1', 'Note', 'content')", []).unwrap();

        let preview = full_deletion_preview(&conn, "w1").unwrap();
        assert_eq!(preview.sources.len(), 1);
        assert_eq!(preview.meetings.len(), 1);
        assert_eq!(preview.notes_count, 1);
        assert_eq!(preview.total_derived_items, 1);
    }
}
