use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
pub struct CostSummary {
    pub total_tokens: i64,
    pub total_cost_cents: i64,
    pub by_provider: Vec<ProviderCost>,
    pub by_capability: Vec<CapabilityCost>,
    pub daily_usage: Vec<DailyUsage>,
}

#[derive(Serialize)]
pub struct ProviderCost {
    pub provider: String,
    pub tokens: i64,
    pub cost_cents: i64,
}

#[derive(Serialize)]
pub struct CapabilityCost {
    pub capability: String,
    pub tokens: i64,
    pub cost_cents: i64,
}

#[derive(Serialize)]
pub struct DailyUsage {
    pub date: String,
    pub tokens: i64,
    pub cost_cents: i64,
}

/// Pricing per 1M tokens in cents (USD * 100)
fn provider_pricing(provider: &str, model: &str) -> (f64, f64) {
    // (input_price_per_1M_tokens_cents, output_price_per_1M_tokens_cents)
    match provider {
        "openai" => match model {
            m if m.contains("gpt-4o") => (250.0, 1000.0),
            m if m.contains("gpt-4") => (3000.0, 6000.0),
            m if m.contains("gpt-3.5") => (50.0, 150.0),
            _ => (250.0, 1000.0),
        },
        "anthropic" => match model {
            m if m.contains("claude-3-opus") => (1500.0, 7500.0),
            m if m.contains("claude-3-sonnet") => (300.0, 1500.0),
            m if m.contains("claude-3-haiku") => (25.0, 125.0),
            _ => (300.0, 1500.0),
        },
        "google" => match model {
            m if m.contains("gemini-1.5-pro") => (350.0, 1050.0),
            m if m.contains("gemini-1.5-flash") => (7.5, 30.0),
            _ => (7.5, 30.0),
        },
        _ => (0.0, 0.0), // local models are free
    }
}

/// Record token usage for cost tracking
pub fn record_token_usage(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<(), String> {
    let (input_price, output_price) = provider_pricing(provider, model);
    let cost_cents =
        (input_tokens as f64 / 1_000_000.0 * input_price
            + output_tokens as f64 / 1_000_000.0 * output_price) as i64;

    let id = format!("usage-{}", uuid::Uuid::new_v4());
    connection
        .execute(
            "INSERT INTO token_usage (id, workspace_id, capability, provider, model,
             input_tokens, output_tokens, cost_cents, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)",
            params![
                id,
                workspace_id,
                capability,
                provider,
                model,
                input_tokens,
                output_tokens,
                cost_cents
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get cost summary for a workspace
pub fn get_cost_summary(
    connection: &Connection,
    workspace_id: &str,
) -> Result<CostSummary, String> {
    let total_tokens: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
             FROM token_usage WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_cost_cents: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(cost_cents), 0)
             FROM token_usage WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut provider_stmt = connection
        .prepare(
            "SELECT provider, COALESCE(SUM(input_tokens + output_tokens), 0),
             COALESCE(SUM(cost_cents), 0)
             FROM token_usage WHERE workspace_id = ?1 GROUP BY provider",
        )
        .map_err(|e| e.to_string())?;

    let by_provider = provider_stmt
        .query_map(params![workspace_id], |row| {
            Ok(ProviderCost {
                provider: row.get(0)?,
                tokens: row.get(1)?,
                cost_cents: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut cap_stmt = connection
        .prepare(
            "SELECT capability, COALESCE(SUM(input_tokens + output_tokens), 0),
             COALESCE(SUM(cost_cents), 0)
             FROM token_usage WHERE workspace_id = ?1 GROUP BY capability",
        )
        .map_err(|e| e.to_string())?;

    let by_capability = cap_stmt
        .query_map(params![workspace_id], |row| {
            Ok(CapabilityCost {
                capability: row.get(0)?,
                tokens: row.get(1)?,
                cost_cents: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut daily_stmt = connection
        .prepare(
            "SELECT DATE(created_at), COALESCE(SUM(input_tokens + output_tokens), 0),
             COALESCE(SUM(cost_cents), 0)
             FROM token_usage WHERE workspace_id = ?1
             GROUP BY DATE(created_at) ORDER BY DATE(created_at) DESC LIMIT 30",
        )
        .map_err(|e| e.to_string())?;

    let daily_usage = daily_stmt
        .query_map(params![workspace_id], |row| {
            Ok(DailyUsage {
                date: row.get(0)?,
                tokens: row.get(1)?,
                cost_cents: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(CostSummary {
        total_tokens,
        total_cost_cents,
        by_provider,
        by_capability,
        daily_usage,
    })
}

/// Check if a workspace has exceeded its cost limit (in cents).
/// Returns true if the limit would be exceeded.
pub fn check_cost_limit(
    connection: &Connection,
    workspace_id: &str,
    limit_cents: i64,
) -> Result<bool, String> {
    let total: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(cost_cents), 0) FROM token_usage WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(total >= limit_cents)
}

/// Set a cost limit for a workspace capability
pub fn set_cost_limit(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    limit_cents: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO cost_limits (workspace_id, capability, limit_cents)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(workspace_id, capability) DO UPDATE SET limit_cents = excluded.limit_cents",
            params![workspace_id, capability, limit_cents],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get cost limit for a capability
pub fn get_cost_limit(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
) -> Result<Option<i64>, String> {
    connection
        .query_row(
            "SELECT limit_cents FROM cost_limits WHERE workspace_id = ?1 AND capability = ?2",
            params![workspace_id, capability],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.to_string()),
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
    fn token_usage_is_priced_per_provider() {
        let conn = setup();
        // gpt-4o: 250c per 1M input tokens, 1000c per 1M output tokens
        record_token_usage(&conn, "w1", "chat", "openai", "gpt-4o", 1_000_000, 1_000_000).unwrap();
        // local models are free
        record_token_usage(&conn, "w1", "chat", "ollama", "llama3", 5_000_000, 5_000_000).unwrap();

        let summary = get_cost_summary(&conn, "w1").unwrap();
        assert_eq!(summary.total_tokens, 12_000_000);
        assert_eq!(summary.total_cost_cents, 1250); // 250 + 1000

        let openai = summary.by_provider.iter().find(|p| p.provider == "openai").unwrap();
        assert_eq!(openai.cost_cents, 1250);
        let ollama = summary.by_provider.iter().find(|p| p.provider == "ollama").unwrap();
        assert_eq!(ollama.cost_cents, 0);
    }

    #[test]
    fn cost_limit_is_enforced() {
        let conn = setup();
        record_token_usage(&conn, "w1", "chat", "openai", "gpt-4o", 1_000_000, 1_000_000).unwrap(); // 1250c

        assert!(!check_cost_limit(&conn, "w1", 2000).unwrap());
        assert!(check_cost_limit(&conn, "w1", 1000).unwrap()); // 1250 >= 1000
    }

    #[test]
    fn cost_limit_roundtrip() {
        let conn = setup();
        assert!(get_cost_limit(&conn, "w1", "chat").unwrap().is_none());
        set_cost_limit(&conn, "w1", "chat", 500).unwrap();
        assert_eq!(get_cost_limit(&conn, "w1", "chat").unwrap(), Some(500));
        set_cost_limit(&conn, "w1", "chat", 900).unwrap(); // upsert
        assert_eq!(get_cost_limit(&conn, "w1", "chat").unwrap(), Some(900));
    }
}
