use rusqlite::{params, Connection};
use serde::Serialize;
use chrono::Utc;

#[derive(Serialize, Clone)]
pub struct ApprovalRequest {
    pub id: String,
    pub agent_id: String,
    pub workspace_id: String,
    pub capability: String, // send_email, schedule_events, delete_data, execute_command, edit_files
    pub action_description: String,
    pub payload_json: String,
    pub status: String, // pending, approved, denied, expired
    pub requested_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
}

/// Create an approval request for a dangerous action
pub fn request_approval(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
    description: &str,
    payload: &serde_json::Value,
) -> Result<ApprovalRequest, String> {
    log::info!("approvals::request_approval: enter");
    let id = format!("approval-{}", uuid::Uuid::new_v4());
    let now = Utc::now().to_rfc3339();

    connection.execute(
        "INSERT INTO approval_requests (id, agent_id, workspace_id, capability, action_description,
         payload_json, status, requested_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)",
        params![
            id, agent_id, workspace_id, capability, description,
            serde_json::to_string(payload).map_err(|e| e.to_string())?,
            now,
        ],
    ).map_err(|e| e.to_string())?;

    // Record in audit log
    super::retention::record_audit(
        connection,
        "approval_requested",
        None,
        Some(capability),
        Some(workspace_id),
        &format!("Agent {agent_id} requests {capability}: {description}"),
    )?;

    Ok(ApprovalRequest {
        id,
        agent_id: agent_id.to_string(),
        workspace_id: workspace_id.to_string(),
        capability: capability.to_string(),
        action_description: description.to_string(),
        payload_json: serde_json::to_string(payload).map_err(|e| e.to_string())?,
        status: "pending".into(),
        requested_at: now,
        resolved_at: None,
        resolved_by: None,
    })
}

/// Approve a pending request
pub fn approve_request(
    connection: &Connection,
    request_id: &str,
    approved_by: &str,
) -> Result<ApprovalRequest, String> {
    log::info!("approvals::approve_request: enter");
    let now = Utc::now().to_rfc3339();
    let changed = connection.execute(
        "UPDATE approval_requests SET status = 'approved', resolved_at = ?1, resolved_by = ?2
         WHERE id = ?3 AND status = 'pending'",
        params![now, approved_by, request_id],
    ).map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("Approval request not found or already resolved".into());
    }

    let request: ApprovalRequest = connection.query_row(
        "SELECT id, agent_id, workspace_id, capability, action_description, payload_json, status,
         requested_at, resolved_at, resolved_by
         FROM approval_requests WHERE id = ?1",
        params![request_id],
        |row| Ok(ApprovalRequest {
            id: row.get(0)?, agent_id: row.get(1)?, workspace_id: row.get(2)?,
            capability: row.get(3)?, action_description: row.get(4)?,
            payload_json: row.get(5)?, status: row.get(6)?,
            requested_at: row.get(7)?, resolved_at: row.get(8)?,
            resolved_by: row.get(9)?,
        }),
    ).map_err(|e| e.to_string())?;

    super::retention::record_audit(
        connection, "approval_granted", None, Some(&request.capability),
        Some(&request.workspace_id),
        &format!("Approved {request_id} by {approved_by}"),
    )?;

    Ok(request)
}

/// Deny a pending request
pub fn deny_request(
    connection: &Connection,
    request_id: &str,
    denied_by: &str,
    reason: &str,
) -> Result<(), String> {
    log::info!("approvals::deny_request: enter");
    let now = Utc::now().to_rfc3339();
    let changed = connection.execute(
        "UPDATE approval_requests SET status = 'denied', resolved_at = ?1, resolved_by = ?2
         WHERE id = ?3 AND status = 'pending'",
        params![now, denied_by, request_id],
    ).map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("Approval request not found or already resolved".into());
    }

    super::retention::record_audit(
        connection, "approval_denied", None, None, None,
        &format!("Denied {request_id}: {reason}"),
    )?;

    Ok(())
}

/// List pending approvals for a workspace
pub fn list_pending_approvals(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<ApprovalRequest>, String> {
    log::debug!("approvals::list_pending_approvals: enter");
    let mut stmt = connection.prepare(
        "SELECT id, agent_id, workspace_id, capability, action_description, payload_json, status,
         requested_at, resolved_at, resolved_by
         FROM approval_requests
         WHERE workspace_id = ?1 AND status = 'pending'
         ORDER BY requested_at DESC LIMIT 50",
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![workspace_id], |row| {
        Ok(ApprovalRequest {
            id: row.get(0)?, agent_id: row.get(1)?, workspace_id: row.get(2)?,
            capability: row.get(3)?, action_description: row.get(4)?,
            payload_json: row.get(5)?, status: row.get(6)?,
            requested_at: row.get(7)?, resolved_at: row.get(8)?,
            resolved_by: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Check if an agent action requires approval and create request if needed
pub fn check_and_request_approval(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
    description: &str,
    payload: &serde_json::Value,
) -> Result<Option<ApprovalRequest>, String> {
    log::debug!("approvals::check_and_request_approval: enter");
    // Check if the agent has explicit permission
    let perm = super::permissions::check_permission(connection, agent_id, workspace_id, capability);

    if perm.allowed {
        // No approval needed - agent has explicit permission
        return Ok(None);
    }

    // Check if this is a capability that always requires approval
    let always_requires = matches!(
        capability,
        "send_email" | "schedule_events" | "delete_data" | "execute_commands" | "modify_files" | "draft_email"
    );

    if !always_requires && super::permissions::check_permission(connection, agent_id, workspace_id, "read_knowledge").allowed {
        // Read-only capabilities don't need approval for read-knowledge agents
        return Ok(None);
    }

    // Create approval request
    let request = request_approval(connection, agent_id, workspace_id, capability, description, payload)?;
    Ok(Some(request))
}

/// Check if an approved request exists for an action
pub fn has_approval(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
) -> Result<bool, String> {
    log::debug!("approvals::has_approval: enter");
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM approval_requests
         WHERE agent_id = ?1 AND workspace_id = ?2 AND capability = ?3
         AND status = 'approved' AND resolved_at >= datetime('now', '-1 hour')",
        params![agent_id, workspace_id, capability],
        |row| row.get(0),
    ).unwrap_or(0);
    Ok(count > 0)
}

/// Expire old pending approvals
pub fn expire_old_approvals(connection: &Connection) -> Result<usize, String> {
    log::info!("approvals::expire_old_approvals: enter");
    let changed = connection.execute(
        "UPDATE approval_requests SET status = 'expired', resolved_at = CURRENT_TIMESTAMP
         WHERE status = 'pending' AND requested_at < datetime('now', '-24 hours')",
        [],
    ).map_err(|e| e.to_string())?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;
    use serde_json::json;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        conn.execute(
            "INSERT INTO agents (id, workspace_id, name, prompt, schedule) VALUES ('agent-1', 'w1', 'Agent', 'prompt', '0 * * * * *')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn request_approval_creates_pending_request() {
        let conn = setup();
        let req = request_approval(&conn, "agent-1", "w1", "send_email", "Send report", &json!({"to": "a@b.com"}))
            .unwrap();
        assert_eq!(req.status, "pending");
        assert_eq!(req.capability, "send_email");
        assert!(req.id.starts_with("approval-"));

        let pending = list_pending_approvals(&conn, "w1").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, req.id);
    }

    #[test]
    fn approve_and_deny_flow() {
        let conn = setup();
        let req = request_approval(&conn, "agent-1", "w1", "send_email", "Send report", &json!({})).unwrap();

        let approved = approve_request(&conn, &req.id, "user").unwrap();
        assert_eq!(approved.status, "approved");
        assert_eq!(approved.resolved_by.as_deref(), Some("user"));

        // Approving again fails (already resolved)
        assert!(approve_request(&conn, &req.id, "user").is_err());

        let req2 = request_approval(&conn, "agent-1", "w1", "modify_files", "Edit file", &json!({})).unwrap();
        deny_request(&conn, &req2.id, "user", "Not needed").unwrap();
        assert!(list_pending_approvals(&conn, "w1").unwrap().is_empty());

        // Denying again fails
        assert!(deny_request(&conn, &req2.id, "user", "again").is_err());
    }

    #[test]
    fn expire_old_approvals_marks_old_requests() {
        let conn = setup();
        let req = request_approval(&conn, "agent-1", "w1", "send_email", "Send report", &json!({})).unwrap();
        // Backdate the request beyond the 24h window
        conn.execute(
            "UPDATE approval_requests SET requested_at = datetime('now', '-25 hours') WHERE id = ?1",
            params![req.id],
        ).unwrap();

        let expired = expire_old_approvals(&conn).unwrap();
        assert_eq!(expired, 1);

        // Fresh request is not expired
        let req2 = request_approval(&conn, "agent-1", "w1", "send_email", "Send report", &json!({})).unwrap();
        let status: String = conn.query_row("SELECT status FROM approval_requests WHERE id = ?1", params![req2.id], |r| r.get(0)).unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn has_approval_only_within_one_hour() {
        let conn = setup();
        assert!(!has_approval(&conn, "agent-1", "w1", "send_email").unwrap());

        let req = request_approval(&conn, "agent-1", "w1", "send_email", "Send", &json!({})).unwrap();
        assert!(!has_approval(&conn, "agent-1", "w1", "send_email").unwrap()); // pending, not approved

        approve_request(&conn, &req.id, "user").unwrap();
        assert!(has_approval(&conn, "agent-1", "w1", "send_email").unwrap());
    }

    #[test]
    fn check_and_request_approval_skips_when_permitted() {
        let conn = setup();
        super::super::permissions::grant_permission(&conn, "agent-1", "w1", "send_email").unwrap();
        let result = check_and_request_approval(&conn, "agent-1", "w1", "send_email", "Send", &json!({})).unwrap();
        assert!(result.is_none());
    }
}
