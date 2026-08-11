use rusqlite::{params, Connection};
use serde::Serialize;
use std::io::Read;

#[derive(Serialize)]
pub struct EmailThread {
    pub thread_id: String,
    pub subject: String,
    pub message_count: usize,
    pub participants: Vec<String>,
    pub oldest_at: Option<String>,
    pub newest_at: Option<String>,
    pub messages: Vec<ThreadMessage>,
}

#[derive(Serialize)]
pub struct ThreadMessage {
    pub email_id: String,
    pub subject: String,
    pub sender: Option<String>,
    pub body_preview: String,
    pub received_at: Option<String>,
    pub position: usize, // 0 = oldest
}

#[derive(Serialize)]
pub struct EmailLabel {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub provider_label_id: Option<String>,
    pub color: Option<String>,
}

/// Reconstruct email threads by grouping on cleaned subject
pub fn rebuild_threads(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<EmailThread>, String> {
    // Get all emails in workspace, ordered by subject and date
    let mut stmt = connection.prepare(
        "SELECT e.id, e.subject, e.sender, COALESCE(e.body, ''), e.received_at, e.provider_id
         FROM emails e
         JOIN email_accounts ea ON ea.id = e.account_id
         WHERE ea.workspace_id = ?1
         ORDER BY clean_subject(e.subject), e.received_at ASC"
    ).map_err(|e| e.to_string())?;

    #[derive(Clone)]
    struct RawEmail {
        id: String, subject: String, sender: Option<String>,
        body: String, received_at: Option<String>, provider_id: Option<String>,
    }

    let emails: Vec<RawEmail> = stmt.query_map(params![workspace_id], |row| {
        Ok(RawEmail {
            id: row.get(0)?, subject: row.get(1)?, sender: row.get(2)?,
            body: row.get(3)?, received_at: row.get(4)?, provider_id: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    // Group by cleaned subject (remove Re:, Fwd:, etc.)
    let mut threads: Vec<EmailThread> = Vec::new();
    let mut current_thread: Option<EmailThread> = None;
    let mut last_clean_subject = String::new();

    for email in &emails {
        let clean = clean_subject(&email.subject);
        if clean != last_clean_subject || current_thread.is_none() {
            if let Some(thread) = current_thread.take() {
                if thread.message_count > 0 {
                    threads.push(thread);
                }
            }
            current_thread = Some(EmailThread {
                thread_id: format!("thread-{}", uuid::Uuid::new_v4()),
                subject: email.subject.clone(),
                message_count: 0,
                participants: Vec::new(),
                oldest_at: email.received_at.clone(),
                newest_at: email.received_at.clone(),
                messages: Vec::new(),
            });
            last_clean_subject = clean;
        }

        if let Some(ref mut thread) = current_thread {
            thread.message_count += 1;
            if let Some(ref sender) = email.sender {
                if !thread.participants.contains(sender) {
                    thread.participants.push(sender.clone());
                }
            }
            thread.newest_at = email.received_at.clone();
            thread.messages.push(ThreadMessage {
                email_id: email.id.clone(),
                subject: email.subject.clone(),
                sender: email.sender.clone(),
                body_preview: email.body.chars().take(150).collect(),
                received_at: email.received_at.clone(),
                position: thread.messages.len(),
            });
        }
    }

    if let Some(thread) = current_thread {
        if thread.message_count > 0 {
            threads.push(thread);
        }
    }

    Ok(threads)
}

pub(crate) fn clean_subject(subject: &str) -> String {
    let mut s = subject.to_lowercase().trim().to_string();
    // Remove common prefixes
    for prefix in &["re:", "fwd:", "fw:", "aw:", "wg:", "sv:"] {
        if s.starts_with(prefix) {
            s = s[prefix.len()..].trim().to_string();
        }
    }
    // Remove bracketed prefixes like [External], [Internal]
    while s.starts_with('[') {
        if let Some(end) = s.find(']') {
            s = s[end + 1..].trim().to_string();
        } else {
            break;
        }
    }
    s.trim().to_string()
}

/// Add a label to an email account
pub fn create_label(
    connection: &Connection,
    account_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<EmailLabel, String> {
    let id = format!("label-{}", uuid::Uuid::new_v4());
    connection.execute(
        "INSERT INTO email_labels (id, account_id, name, color) VALUES (?1, ?2, ?3, ?4)",
        params![id, account_id, name.trim(), color],
    ).map_err(|e| e.to_string())?;
    Ok(EmailLabel { id, account_id: account_id.to_string(), name: name.trim().into(), provider_label_id: None, color: color.map(String::from) })
}

/// List labels for an account
pub fn list_labels(connection: &Connection, account_id: &str) -> Result<Vec<EmailLabel>, String> {
    let mut stmt = connection.prepare(
        "SELECT id, account_id, name, provider_label_id, color FROM email_labels WHERE account_id = ?1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![account_id], |row| Ok(EmailLabel {
        id: row.get(0)?, account_id: row.get(1)?, name: row.get(2)?,
        provider_label_id: row.get(3)?, color: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Assign a label to an email
pub fn label_email(connection: &Connection, email_id: &str, label_id: &str) -> Result<(), String> {
    connection.execute(
        "INSERT OR IGNORE INTO email_label_assignments (email_id, label_id) VALUES (?1, ?2)",
        params![email_id, label_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get emails by label
pub fn get_labeled_emails(connection: &Connection, label_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = connection.prepare(
        "SELECT email_id FROM email_label_assignments WHERE label_id = ?1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![label_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Index email attachment content for search (record and optionally extract text)
pub fn index_attachment(
    connection: &Connection,
    email_id: &str,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
) -> Result<(), String> {
    let id = format!("att-{}", uuid::Uuid::new_v4());
    connection.execute(
        "INSERT INTO email_attachments (id, email_id, filename, content_type, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, email_id, filename, content_type, size_bytes],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Extract text content from an attachment for search indexing.
/// Supports PDF, DOCX, TXT, HTML, JSON, CSV.
/// Returns the extracted text content.
pub fn extract_attachment_text(
    path: &str,
    filename: &str,
) -> Result<String, String> {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "pdf" => pdf_extract::extract_text(path).map_err(|e| format!("PDF extraction failed: {e}")),
        "docx" => {
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            let mut document = archive.by_name("word/document.xml").map_err(|e| e.to_string())?;
            let mut xml = String::new();
            std::io::Read::read_to_string(&mut document, &mut xml).map_err(|e| e.to_string())?;
            let mut reader = quick_xml::Reader::from_str(&xml);
            let mut text = String::new();
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Text(e)) => {
                        text.push_str(&e.unescape().map_err(|e| e.to_string())?);
                    }
                    Ok(quick_xml::events::Event::End(e)) if e.name().as_ref() == b"w:p" => {
                        text.push('\n');
                    }
                    Ok(quick_xml::events::Event::Eof) => break,
                    Err(e) => return Err(e.to_string()),
                    _ => {}
                }
            }
            Ok(text)
        }
        "txt" | "md" | "markdown" | "html" | "htm" | "json" | "csv" | "xml" => {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unsupported attachment format: .{extension}")),
    }
}

/// Auto-assign workspace for an email based on content analysis.
/// Returns the suggested workspace_id or empty string if unsure.
pub fn suggest_workspace_for_email(
    connection: &Connection,
    email_id: &str,
) -> Result<String, String> {
    let (subject, body, sender): (String, Option<String>, Option<String>) = connection.query_row(
        "SELECT subject, body, sender FROM emails WHERE id = ?1",
        params![email_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| e.to_string())?;

    let sender_clone = sender.clone();
    let content = format!("{} {} {}", subject, body.unwrap_or_default(), sender.unwrap_or_default()).to_lowercase();

    // Get all workspace names for matching
    let mut stmt = connection.prepare(
        "SELECT id, name FROM workspaces"
    ).map_err(|e| e.to_string())?;

    let workspaces: Vec<(String, String)> = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Simple keyword matching against workspace names
    for (ws_id, ws_name) in &workspaces {
        if content.contains(&ws_name.to_lowercase()) {
            return Ok(ws_id.clone());
        }
    }

    // Check for domain-based assignment
    if let Some(ref sender_addr) = sender_clone {
        let domain = sender_addr.split('@').nth(1).unwrap_or("");
        // Business domains → work workspace
        if domain.contains("company") || domain.contains("corp") || domain.contains("work") {
            return Ok(workspaces.iter()
                .find(|(_, n)| n.to_lowercase().contains("work"))
                .map(|(id, _)| id.clone())
                .unwrap_or_default());
        }
    }

    Ok(String::new()) // No suggestion
}

/// Generate an email draft using a local or cloud model
pub fn generate_email_draft(
    connection: &Connection,
    workspace_id: &str,
    instructions: &str,
    context: &str,
    model: &str,
) -> Result<String, String> {
    let prompt = format!(
        "You are an email drafting assistant. Write a professional email based on the instructions and context provided.\\n\\
        Return only the email body text, no explanations.\\n\\
        INSTRUCTIONS: {instructions}\\n\\
        CONTEXT: {context}\\n\\
        EMAIL BODY:"
    );
    Ok(crate::chat::chat_completion(connection, workspace_id, "chat", model, &prompt)?)
}

/// Summarize an email thread
pub fn summarize_email_thread(
    connection: &Connection,
    thread_id: &str,
    workspace_id: &str,
    model: &str,
) -> Result<String, String> {
    // Get all messages in the thread
    let mut stmt = connection.prepare(
        "SELECT e.subject, e.sender, COALESCE(e.body, ''), e.received_at
         FROM emails e
         WHERE e.thread_id = ?1
         ORDER BY e.received_at ASC"
    ).map_err(|e| e.to_string())?;

    let messages: Vec<(String, Option<String>, String, Option<String>)> = stmt
        .query_map(params![thread_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if messages.is_empty() {
        return Err("No emails found in this thread".into());
    }

    let thread_text: String = messages
        .iter()
        .enumerate()
        .map(|(i, (subject, sender, body, _date))| {
            format!(
                "[{i}] From: {} | Subject: {} | {}",
                sender.as_deref().unwrap_or("Unknown"),
                subject,
                &body[..body.len().min(300)]
            )
        })
        .collect::<Vec<_>>()
        .join("\\n\\
");

    let prompt = format!(
        "Summarize this email thread concisely. Include: key topics, decisions made, and any action items.\\n\\
        THREAD:\\n{thread_text}"
    );
    crate::chat::chat_completion(connection, workspace_id, "chat", model, &prompt)
}

/// Extract action items from emails using AI
pub fn extract_email_actions(
    connection: &Connection,
    email_id: &str,
    workspace_id: &str,
    model: &str,
) -> Result<Vec<String>, String> {
    let (subject, body): (String, Option<String>) = connection.query_row(
        "SELECT subject, body FROM emails WHERE id = ?1",
        params![email_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;

    let content = format!("Subject: {}\n\n{}", subject, body.unwrap_or_default());
    let prompt = format!(
        "Extract action items from the following email. Return a JSON array of strings, each a concise action item. If there are none, return an empty array.\\n\\nEMAIL:\\n{content}"
    );
    let raw = crate::chat::chat_completion(connection, workspace_id, "chat", model, &prompt)?;
    serde_json::from_str::<Vec<String>>(&raw).map_err(|_| raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        conn.execute(
            "INSERT INTO email_accounts (id, workspace_id, provider, account_label, status) VALUES ('acct-1', 'w1', 'imap', 'Work', 'connected')",
            [],
        ).unwrap();
        conn
    }

    fn insert_email(conn: &Connection, id: &str, subject: &str, sender: &str, body: &str, received_at: &str) {
        conn.execute(
            "INSERT INTO emails (id, account_id, subject, sender, body, received_at) VALUES (?1, 'acct-1', ?2, ?3, ?4, ?5)",
            params![id, subject, sender, body, received_at],
        ).unwrap();
    }

    #[test]
    fn clean_subject_strips_prefixes_and_brackets() {
        assert_eq!(clean_subject("Re: Fwd: Hello"), "hello");
        assert_eq!(clean_subject("[External] Status Update"), "status update");
        assert_eq!(clean_subject("No prefix"), "no prefix");
    }

    #[test]
    fn rebuild_threads_groups_related_emails() {
        let conn = setup();
        insert_email(&conn, "e1", "Project Sync", "alice@corp.com", "first", "2026-08-10T09:00:00Z");
        insert_email(&conn, "e2", "Re: Project Sync", "bob@corp.com", "reply", "2026-08-10T10:00:00Z");
        insert_email(&conn, "e3", "Fwd: Project Sync", "carol@corp.com", "fwd", "2026-08-10T11:00:00Z");
        insert_email(&conn, "e4", "Lunch Plans", "dave@corp.com", "other", "2026-08-10T12:00:00Z");

        let threads = rebuild_threads(&conn, "w1").unwrap();
        assert_eq!(threads.len(), 2);
        let sync = threads.iter().find(|t| t.subject == "Project Sync").unwrap();
        assert_eq!(sync.message_count, 3);
        assert_eq!(sync.participants.len(), 3);
        assert!(sync.messages.iter().any(|m| m.subject.starts_with("Re:")));
    }

    #[test]
    fn labels_crud_and_assignment() {
        let conn = setup();
        insert_email(&conn, "e1", "Project Sync", "a@b.com", "x", "2026-08-10T09:00:00Z");
        let label = create_label(&conn, "acct-1", "Important", Some("#ff0000")).unwrap();
        assert_eq!(label.name, "Important");

        let labels = list_labels(&conn, "acct-1").unwrap();
        assert_eq!(labels.len(), 1);

        label_email(&conn, "e1", &label.id).unwrap();
        let labeled = get_labeled_emails(&conn, &label.id).unwrap();
        assert_eq!(labeled, vec!["e1"]);
    }

    #[test]
    fn suggest_workspace_matches_keywords_and_domains() {
        let conn = setup();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('research', 'Research')", []).unwrap();
        insert_email(&conn, "e1", "Q3 Research Notes", "a@b.com", "research draft", "2026-08-10T09:00:00Z");
        let ws = suggest_workspace_for_email(&conn, "e1").unwrap();
        assert_eq!(ws, "research");

        insert_email(&conn, "e2", "Contract", "lawyer@company.com", "terms", "2026-08-10T09:00:00Z");
        let ws2 = suggest_workspace_for_email(&conn, "e2").unwrap();
        assert_eq!(ws2, "w1"); // domain company -> work workspace (first workspace name containing "work")
    }

    #[test]
    fn extract_attachment_text_supports_txt() {
        let conn = setup();
        let dir = std::env::temp_dir().join(format!("nao-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("notes.txt");
        std::fs::write(&file, "hello attachment world").unwrap();
        let text = extract_attachment_text(file.to_str().unwrap(), "notes.txt").unwrap();
        assert!(text.contains("hello attachment world"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn index_attachment_records_metadata() {
        let conn = setup();
        insert_email(&conn, "e1", "With attachment", "a@b.com", "x", "2026-08-10T09:00:00Z");
        index_attachment(&conn, "e1", "report.pdf", "application/pdf", 1024).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_attachments WHERE email_id = 'e1'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }
}
