use rusqlite::{params, Connection};
use serde::Serialize;
use chrono::Utc;

#[derive(Serialize)]
pub struct MeetingPrep {
    pub meeting_id: String,
    pub title: String,
    pub starts_at: String,
    pub participants: Vec<String>,
    pub recent_related_emails: Vec<EmailBrief>,
    pub open_action_items: Vec<String>,
    pub suggested_agenda: String,
    pub knowledge_preview: Vec<KnowledgeSnippet>,
}

#[derive(Serialize)]
pub struct EmailBrief {
    pub subject: String,
    pub sender: Option<String>,
    pub snippet: String,
}

#[derive(Serialize)]
pub struct KnowledgeSnippet {
    pub title: String,
    pub content: String,
}

/// Generate a meeting preparation briefing.
/// Gathers related emails, open action items, and relevant knowledge.
pub fn prepare_meeting(
    app: &tauri::AppHandle,
    connection: &Connection,
    event_id: &str,
) -> Result<MeetingPrep, String> {
    let (workspace_id, title, starts_at): (String, String, String) = connection
        .query_row(
            "SELECT workspace_id, title, starts_at FROM calendar_events WHERE id = ?1",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    // Get participants
    let mut part_stmt = connection
        .prepare("SELECT name FROM calendar_participants WHERE event_id = ?1")
        .map_err(|e| e.to_string())?;
    let participants: Vec<String> = part_stmt
        .query_map(params![event_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Get recent related emails (last 7 days)
    let seven_days_ago = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let mut email_stmt = connection
        .prepare(
            "SELECT subject, sender, COALESCE(body, '') FROM emails
             JOIN email_accounts ON email_accounts.id = emails.account_id
             WHERE email_accounts.workspace_id = ?1
             AND emails.received_at >= ?2
             ORDER BY emails.received_at DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let recent_emails: Vec<EmailBrief> = email_stmt
        .query_map(params![workspace_id, seven_days_ago], |row| {
            Ok(EmailBrief {
                subject: row.get(0)?,
                sender: row.get(1)?,
                snippet: row.get::<_, String>(2)
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Get open action items from recent meetings
    let mut action_stmt = connection
        .prepare(
            "SELECT title FROM meeting_actions
             WHERE status = 'open'
             ORDER BY rowid DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let open_actions: Vec<String> = action_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Get relevant knowledge from the workspace
    let mut knowledge_stmt = connection
        .prepare(
            "SELECT sources.title, source_chunks.content
             FROM source_chunks
             JOIN sources ON sources.id = source_chunks.source_id
             WHERE sources.workspace_id = ?1
             ORDER BY source_chunks.rowid DESC LIMIT 5",
        )
        .map_err(|e| e.to_string())?;

    let knowledge: Vec<KnowledgeSnippet> = knowledge_stmt
        .query_map(params![workspace_id], |row| {
            Ok(KnowledgeSnippet {
                title: row.get(0)?,
                content: row.get::<_, String>(1)
                    .unwrap_or_default()
                    .chars()
                    .take(300)
                    .collect(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Build suggested agenda
    let agenda = build_agenda(&title, &participants, &open_actions);

    Ok(MeetingPrep {
        meeting_id: event_id.to_string(),
        title,
        starts_at,
        participants,
        recent_related_emails: recent_emails,
        open_action_items: open_actions,
        suggested_agenda: agenda,
        knowledge_preview: knowledge,
    })
}

fn build_agenda(title: &str, participants: &[String], actions: &[String]) -> String {
    let mut agenda = format!("# {title}\n\n");
    agenda.push_str("## Suggested Agenda\n\n");
    agenda.push_str("1. Opening & check-in (5 min)\n");

    if !participants.is_empty() {
        agenda.push_str(&format!(
            "2. Updates from {} (10 min)\n",
            participants.join(", ")
        ));
    }

    if !actions.is_empty() {
        agenda.push_str("3. Review of open action items (10 min)\n");
        for action in actions.iter().take(5) {
            agenda.push_str(&format!("   - {action}\n"));
        }
    }

    agenda.push_str("4. Key discussion topics (20 min)\n");
    agenda.push_str("5. Decisions & next steps (10 min)\n");
    agenda.push_str("6. Wrap-up (5 min)\n");

    agenda
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agenda_includes_participants_and_actions() {
        let agenda = build_agenda("Weekly Sync", &["Alice".to_string(), "Bob".to_string()], &["Send update".to_string()]);
        assert!(agenda.contains("# Weekly Sync"));
        assert!(agenda.contains("Updates from Alice, Bob"));
        assert!(agenda.contains("Send update"));
        assert!(agenda.contains("Key discussion topics"));
    }

    #[test]
    fn agenda_works_without_optional_sections() {
        let agenda = build_agenda("Standup", &[], &[]);
        assert!(agenda.contains("# Standup"));
        assert!(!agenda.contains("Updates from"));
        assert!(!agenda.contains("open action items"));
    }
}
