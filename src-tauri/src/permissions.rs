use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentPermission {
    pub agent_id: Option<String>,
    pub capability: String, // read_knowledge, read_transcripts, create_notes, create_reminders, modify_tasks, schedule_events, draft_email, send_email, modify_files, execute_commands
    pub granted: bool,
    pub workspace_id: String,
}

#[derive(Serialize)]
pub struct PermissionCheck {
    pub capability: String,
    pub allowed: bool,
    pub reason: String,
}

/// Predefined permission capabilities for external agents
const CAPABILITIES: &[&str] = &[
    "read_knowledge",
    "read_transcripts",
    "create_notes",
    "create_reminders",
    "modify_tasks",
    "schedule_events",
    "draft_email",
    "send_email",
    "modify_files",
    "execute_commands",
];

/// Check if an agent has permission for a specific capability in a workspace
pub fn check_permission(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
) -> PermissionCheck {
    // Default: all external agents are read-only unless explicitly granted
    let default_allowed = is_read_capability(capability);

    // Workspace allowlist enforcement: if the workspace has an explicit
    // allowlist, capabilities outside it are denied regardless of agent grants.
    let allowlist_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM workspace_allowlists WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if allowlist_count > 0 {
        let in_list: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_allowlists WHERE workspace_id = ?1 AND capability = ?2",
                params![workspace_id, capability],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if in_list == 0 {
            return PermissionCheck {
                capability: capability.to_string(),
                allowed: false,
                reason: "Not in workspace allowlist".into(),
            };
        }
    }

    let result = connection.query_row(
        "SELECT granted FROM agent_permissions
         WHERE agent_id = ?1 AND workspace_id = ?2 AND capability = ?3",
        params![agent_id, workspace_id, capability],
        |row| row.get::<_, bool>(0),
    );

    match result {
        Ok(granted) => PermissionCheck {
            capability: capability.to_string(),
            allowed: granted,
            reason: if granted {
                "Explicitly granted".into()
            } else {
                "Explicitly denied".into()
            },
        },
        Err(rusqlite::Error::QueryReturnedNoRows) => PermissionCheck {
            capability: capability.to_string(),
            allowed: default_allowed,
            reason: if default_allowed {
                "Allowed by default (read-only)".into()
            } else {
                "Denied by default (requires explicit grant)".into()
            },
        },
        Err(e) => PermissionCheck {
            capability: capability.to_string(),
            allowed: false,
            reason: format!("Permission check error: {e}"),
        },
    }
}

fn is_read_capability(capability: &str) -> bool {
    matches!(capability, "read_knowledge" | "read_transcripts")
}

/// Grant a permission to an agent
pub fn grant_permission(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
) -> Result<(), String> {
    if !CAPABILITIES.contains(&capability) {
        return Err(format!("Unknown capability: {capability}"));
    }

    connection
        .execute(
            "INSERT INTO agent_permissions (agent_id, workspace_id, capability, granted)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(agent_id, workspace_id, capability) DO UPDATE SET granted = 1",
            params![agent_id, workspace_id, capability],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Revoke a permission from an agent
pub fn revoke_permission(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE agent_permissions SET granted = 0
             WHERE agent_id = ?1 AND workspace_id = ?2 AND capability = ?3",
            params![agent_id, workspace_id, capability],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// List all permissions for an agent in a workspace
pub fn list_agent_permissions(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
) -> Result<Vec<AgentPermission>, String> {
    let mut stmt = connection
        .prepare(
            "SELECT agent_id, capability, granted, workspace_id
             FROM agent_permissions
             WHERE agent_id = ?1 AND workspace_id = ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![agent_id, workspace_id], |row| {
            Ok(AgentPermission {
                agent_id: Some(row.get(0)?),
                capability: row.get(1)?,
                granted: row.get(2)?,
                workspace_id: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut permissions: Vec<AgentPermission> =
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    // Fill in defaults for capabilities not explicitly set
    for cap in CAPABILITIES {
        if !permissions.iter().any(|p| p.capability == *cap) {
            permissions.push(AgentPermission {
                agent_id: Some(agent_id.to_string()),
                capability: cap.to_string(),
                granted: is_read_capability(cap),
                workspace_id: workspace_id.to_string(),
            });
        }
    }

    Ok(permissions)
}

/// Check all permissions for an agent — returns a summary
pub fn check_all_permissions(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
) -> Result<Vec<PermissionCheck>, String> {
    CAPABILITIES
        .iter()
        .map(|cap| Ok(check_permission(connection, agent_id, workspace_id, cap)))
        .collect()
}

/// Validate that an action is allowed before executing it
pub fn validate_action(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
) -> Result<(), String> {
    let check = check_permission(connection, agent_id, workspace_id, capability);
    if !check.allowed {
        return Err(format!(
            "Permission denied: {capability} — {reason}",
            reason = check.reason
        ));
    }
    Ok(())
}
/// Centralized action authorization: checks permission, then approval if needed.
/// Returns Ok(()) if the action is allowed, or Err with a descriptive message.
/// This is the single entry point for ALL write operations from agents.
pub fn authorize_action(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    capability: &str,
    description: &str,
    payload: &serde_json::Value,
) -> Result<Option<super::approvals::ApprovalRequest>, String> {
    // Step 1: Check permission (validate_action centralizes the denial path)
    validate_action(connection, agent_id, workspace_id, capability)?;

    // Step 2: Check if this capability requires approval
    let requires_approval = matches!(
        capability,
        "send_email" | "modify_calendar" | "delete_data" | "execute_commands" | "modify_files" | "draft_email"
    );

    if !requires_approval {
        return Ok(None); // No approval needed
    }

    // Step 3: Check if there's already a recent approval
    let has_recent = super::approvals::has_approval(connection, agent_id, workspace_id, capability)?;
    if has_recent {
        return Ok(None); // Already approved within the last hour
    }

    // Step 4: Create approval request
    let request = super::approvals::request_approval(
        connection, agent_id, workspace_id, capability, description, payload,
    )?;
    Ok(Some(request))
}

/// Set workspace-level tool allowlist for external agents
pub fn set_workspace_allowlist(
    connection: &Connection,
    workspace_id: &str,
    allowed_capabilities: &[String],
) -> Result<(), String> {
    // Clear existing allowlist
    connection
        .execute(
            "DELETE FROM workspace_allowlists WHERE workspace_id = ?1",
            params![workspace_id],
        )
        .map_err(|e| e.to_string())?;

    for cap in allowed_capabilities {
        if CAPABILITIES.contains(&cap.as_str()) {
            connection
                .execute(
                    "INSERT INTO workspace_allowlists (workspace_id, capability) VALUES (?1, ?2)",
                    params![workspace_id, cap],
                )
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Get workspace allowlist
pub fn get_workspace_allowlist(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = connection
        .prepare(
            "SELECT capability FROM workspace_allowlists WHERE workspace_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![workspace_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        Ok(CAPABILITIES.iter().map(|c| c.to_string()).collect())
    } else {
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;
    use serde_json::json;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO agents (id, workspace_id, name, prompt, schedule) VALUES ('agent-1', 'w1', 'Agent', 'prompt', '0 * * * * *')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn read_capabilities_allowed_by_default() {
        let conn = setup();
        let check = check_permission(&conn, "agent-1", "w1", "read_knowledge");
        assert!(check.allowed);
        assert_eq!(check.reason, "Allowed by default (read-only)");
        assert!(check_permission(&conn, "agent-1", "w1", "read_transcripts").allowed);
    }

    #[test]
    fn write_capabilities_denied_by_default() {
        let conn = setup();
        for cap in ["send_email", "modify_files", "execute_commands", "schedule_events", "modify_tasks"] {
            let check = check_permission(&conn, "agent-1", "w1", cap);
            assert!(!check.allowed, "{cap} should be denied by default");
            assert!(check.reason.contains("Denied by default"));
        }
    }

    #[test]
    fn grant_and_revoke_permission() {
        let conn = setup();
        grant_permission(&conn, "agent-1", "w1", "send_email").unwrap();
        let check = check_permission(&conn, "agent-1", "w1", "send_email");
        assert!(check.allowed);
        assert_eq!(check.reason, "Explicitly granted");

        revoke_permission(&conn, "agent-1", "w1", "send_email").unwrap();
        let check = check_permission(&conn, "agent-1", "w1", "send_email");
        assert!(!check.allowed);
        assert_eq!(check.reason, "Explicitly denied");
    }

    #[test]
    fn unknown_capability_is_rejected() {
        let conn = setup();
        let err = grant_permission(&conn, "agent-1", "w1", "hack_the_planet").unwrap_err();
        assert!(err.contains("Unknown capability"));
    }

    #[test]
    fn list_agent_permissions_includes_defaults() {
        let conn = setup();
        let perms = list_agent_permissions(&conn, "agent-1", "w1").unwrap();
        assert_eq!(perms.len(), 10);
        assert!(perms.iter().any(|p| p.capability == "read_knowledge" && p.granted));
        assert!(perms.iter().any(|p| p.capability == "send_email" && !p.granted));
    }

    #[test]
    fn validate_action_enforces_permissions() {
        let conn = setup();
        assert!(validate_action(&conn, "agent-1", "w1", "read_knowledge").is_ok());
        assert!(validate_action(&conn, "agent-1", "w1", "send_email").is_err());
        grant_permission(&conn, "agent-1", "w1", "send_email").unwrap();
        assert!(validate_action(&conn, "agent-1", "w1", "send_email").is_ok());
    }

    #[test]
    fn authorize_action_returns_no_approval_for_reads() {
        let conn = setup();
        let result = authorize_action(&conn, "agent-1", "w1", "read_knowledge", "Read", &json!({})).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn authorize_action_creates_approval_for_send_email() {
        let conn = setup();
        // No explicit grant -> denied outright
        assert!(authorize_action(&conn, "agent-1", "w1", "send_email", "Send email", &json!({"to": "x@y.com"})).is_err());

        // With grant -> approval request created
        grant_permission(&conn, "agent-1", "w1", "send_email").unwrap();
        let request = authorize_action(&conn, "agent-1", "w1", "send_email", "Send email", &json!({"to": "x@y.com"}))
            .unwrap().expect("approval request");
        assert_eq!(request.status, "pending");

        // Approve it, then authorization passes without a new request
        crate::approvals::approve_request(&conn, &request.id, "user").unwrap();
        let second = authorize_action(&conn, "agent-1", "w1", "send_email", "Send email", &json!({"to": "x@y.com"}))
            .unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn workspace_allowlist_restricts_capabilities() {
        let conn = setup();
        grant_permission(&conn, "agent-1", "w1", "send_email").unwrap();

        set_workspace_allowlist(&conn, "w1", &["read_knowledge".to_string()]).unwrap();
        // Even with an explicit grant, send_email is blocked by the allowlist.
        let check = check_permission(&conn, "agent-1", "w1", "send_email");
        assert!(!check.allowed);
        assert_eq!(check.reason, "Not in workspace allowlist");
        // Allowlisted read still works.
        assert!(check_permission(&conn, "agent-1", "w1", "read_knowledge").allowed);
        // And reads not in the allowlist are also blocked.
        assert!(!check_permission(&conn, "agent-1", "w1", "read_transcripts").allowed);

        let allowlist = get_workspace_allowlist(&conn, "w1").unwrap();
        assert_eq!(allowlist, vec!["read_knowledge"]);

        // Empty allowlist means unrestricted (default set of capabilities).
        set_workspace_allowlist(&conn, "w1", &[]).unwrap();
        let all = get_workspace_allowlist(&conn, "w1").unwrap();
        assert_eq!(all.len(), 10);
    }
}
