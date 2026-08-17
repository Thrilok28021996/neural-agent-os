use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct FallbackChain {
    pub capability: String,
    pub providers: Vec<FallbackProvider>,
}

#[derive(Serialize, Clone)]
pub struct FallbackProvider {
    pub provider: String,
    pub model: String,
    pub priority: u32,
    pub healthy: bool,
}

/// Return the ordered fallback chain for a given workspace capability.
pub fn get_fallback_chain(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
) -> Result<FallbackChain, String> {
    log::debug!("fallback::get_fallback_chain: enter");
    let mut statement = connection
        .prepare(
            "SELECT provider, model, priority, healthy
             FROM provider_fallback
             WHERE workspace_id = ?1 AND capability = ?2
             ORDER BY priority ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = statement
        .query_map(
            params![workspace_id, capability],
            |row| {
                Ok(FallbackProvider {
                    provider: row.get(0)?,
                    model: row.get(1)?,
                    priority: row.get(2)?,
                    healthy: row.get::<_, bool>(3).unwrap_or(true),
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let providers: Vec<FallbackProvider> = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(FallbackChain {
        capability: capability.to_string(),
        providers,
    })
}

/// Set a fallback provider for a capability
pub fn set_fallback_provider(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    provider: &str,
    model: &str,
    priority: u32,
) -> Result<(), String> {
    log::info!("fallback::set_fallback_provider: enter");
    connection
        .execute(
            "INSERT INTO provider_fallback (workspace_id, capability, provider, model, priority, healthy)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(workspace_id, capability, priority)
             DO UPDATE SET provider = excluded.provider, model = excluded.model",
            params![workspace_id, capability, provider, model, priority],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark a fallback provider as unhealthy after a failure
pub fn mark_fallback_unhealthy(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    provider: &str,
) -> Result<(), String> {
    log::info!("fallback::mark_fallback_unhealthy: enter");
    connection
        .execute(
            "UPDATE provider_fallback SET healthy = 0
             WHERE workspace_id = ?1 AND capability = ?2 AND provider = ?3",
            params![workspace_id, capability, provider],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Resolve the next healthy provider for a capability, with fallback.
/// If no fallback is configured, returns the primary route.
#[allow(dead_code)] // tested public API (fallback resolution unit tests)
pub fn resolve_provider(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    primary_model: &str,
) -> (String, String) {
    log::info!("fallback::resolve_provider: enter");
    // Try fallback chain first
    let chain = get_fallback_chain(connection, workspace_id, capability).ok();
    if let Some(chain) = chain {
        for fb in &chain.providers {
            if fb.healthy {
                return (fb.provider.clone(), fb.model.clone());
            }
        }
    }
    // Fall back to the primary route
    let model = connection
        .query_row(
            "SELECT model FROM model_routes WHERE workspace_id = ?1 AND capability = ?2",
            params![workspace_id, capability],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| primary_model.to_string());
    ("primary".to_string(), model)
}

/// Remove all fallback entries for a capability
pub fn clear_fallback_chain(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
) -> Result<(), String> {
    log::info!("fallback::clear_fallback_chain: enter");
    connection
        .execute(
            "DELETE FROM provider_fallback WHERE workspace_id = ?1 AND capability = ?2",
            params![workspace_id, capability],
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
        conn.execute("INSERT INTO model_routes (workspace_id, capability, provider, model) VALUES ('w1', 'chat', 'ollama', 'qwen3:14b')", []).unwrap();
        conn
    }

    #[test]
    fn fallback_chain_respects_priority_order() {
        let conn = setup();
        set_fallback_provider(&conn, "w1", "chat", "openai", "gpt-4o", 1).unwrap();
        set_fallback_provider(&conn, "w1", "chat", "anthropic", "claude-3-sonnet", 2).unwrap();

        let chain = get_fallback_chain(&conn, "w1", "chat").unwrap();
        assert_eq!(chain.providers.len(), 2);
        assert_eq!(chain.providers[0].provider, "openai");
        assert_eq!(chain.providers[1].provider, "anthropic");
        assert!(chain.providers.iter().all(|p| p.healthy));
    }

    #[test]
    fn unhealthy_provider_is_skipped_in_resolution() {
        let conn = setup();
        set_fallback_provider(&conn, "w1", "chat", "openai", "gpt-4o", 1).unwrap();
        set_fallback_provider(&conn, "w1", "chat", "anthropic", "claude-3-sonnet", 2).unwrap();

        mark_fallback_unhealthy(&conn, "w1", "chat", "openai").unwrap();
        let (provider, model) = resolve_provider(&conn, "w1", "chat", "fallback-model");
        assert_eq!(provider, "anthropic");
        assert_eq!(model, "claude-3-sonnet");
    }

    #[test]
    fn resolution_falls_back_to_primary_route() {
        let conn = setup();
        // No fallback configured -> primary model route
        let (provider, model) = resolve_provider(&conn, "w1", "chat", "default-model");
        assert_eq!(provider, "primary");
        assert_eq!(model, "qwen3:14b");

        // Unconfigured capability -> caller-provided default
        let (_, model) = resolve_provider(&conn, "w1", "embeddings", "nomic-embed");
        assert_eq!(model, "nomic-embed");
    }

    #[test]
    fn clear_fallback_chain_removes_entries() {
        let conn = setup();
        set_fallback_provider(&conn, "w1", "chat", "openai", "gpt-4o", 1).unwrap();
        clear_fallback_chain(&conn, "w1", "chat").unwrap();
        assert!(get_fallback_chain(&conn, "w1", "chat").unwrap().providers.is_empty());
    }
}
