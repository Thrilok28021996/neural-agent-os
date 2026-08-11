use rusqlite::{params, Connection};
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone)]
pub struct ProviderHealth {
    pub provider: String,
    pub capability: String,
    pub model: String,
    pub healthy: bool,
    pub last_checked_at: Option<String>,
    pub response_time_ms: Option<u64>,
    pub error_message: Option<String>,
    pub consecutive_failures: i32,
}

/// Check if a provider is healthy by making a lightweight API call
pub fn health_check(
    provider: &str,
    capability: &str,
    model: &str,
) -> (bool, Option<u64>, Option<String>) {
    let start = Instant::now();

    match provider {
        "ollama" | "Ollama" => {
            let url = std::env::var("NEURAL_OLLAMA_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434/api/tags".into());
            match reqwest::blocking::Client::new()
                .get(&url)
                .timeout(Duration::from_secs(5))
                .send()
            {
                Ok(resp) if resp.status().is_success() => {
                    (true, Some(start.elapsed().as_millis() as u64), None)
                }
                Ok(resp) => (false, None, Some(format!("HTTP {}", resp.status()))),
                Err(e) => (false, None, Some(e.to_string())),
            }
        }
        "openai" | "OpenAI" => {
            let api_key = keyring::Entry::new("neural-agent-os/provider", "openai:api_key")
                .ok()
                .and_then(|e| e.get_password().ok());
            match api_key {
                Some(_) => (true, Some(start.elapsed().as_millis() as u64), None),
                None => (false, None, Some("No API key configured".into())),
            }
        }
        "anthropic" | "Anthropic" => {
            let api_key = keyring::Entry::new("neural-agent-os/provider", "anthropic:api_key")
                .ok()
                .and_then(|e| e.get_password().ok());
            if api_key.is_some() {
                (true, None, None)
            } else {
                (false, None, Some("No API key configured".into()))
            }
        }
        "google" | "Google" => {
            let api_key = keyring::Entry::new("neural-agent-os/provider", "google:api_key")
                .ok()
                .and_then(|e| e.get_password().ok());
            if api_key.is_some() {
                (true, None, None)
            } else {
                (false, None, Some("No API key configured".into()))
            }
        }
        _ => {
            // Local providers: check if they're configured
            if capability == "transcription" {
                let whisper = std::env::var("NEURAL_WHISPER_BIN").unwrap_or_else(|_| "whisper".into());
                let installed = std::process::Command::new(&whisper).arg("--help").output().is_ok();
                (installed, None, if !installed { Some("Whisper not found".into()) } else { None })
            } else {
                (true, None, None) // Assume local providers are healthy
            }
        }
    }
}

/// Run health checks on all configured providers and update status
pub fn refresh_provider_health(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<ProviderHealth>, String> {
    let mut stmt = connection.prepare(
        "SELECT provider, capability, model FROM model_routes WHERE workspace_id = ?1
         UNION SELECT provider, capability, model FROM provider_fallback WHERE workspace_id = ?1"
    ).map_err(|e| e.to_string())?;

    let routes: Vec<(String, String, String)> = stmt.query_map(
        params![workspace_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut results = Vec::new();

    for (provider, capability, model) in &routes {
        let (healthy, response_time, error) = health_check(provider, capability, model);

        // Update fallback chain health
        if !healthy {
            let _ = super::fallback::mark_fallback_unhealthy(
                connection, workspace_id, capability, provider,
            );
        }

        // Track consecutive failures
        let failures: i32 = if healthy {
            0
        } else {
            connection.query_row(
                "SELECT COALESCE(MAX(consecutive_failures), 0) + 1 FROM provider_health_log
                 WHERE provider = ?1 AND capability = ?2 AND workspace_id = ?3
                 AND checked_at >= datetime('now', '-1 hour')",
                params![provider, capability, workspace_id],
                |row| row.get(0),
            ).unwrap_or(1)
        };

        connection.execute(
            "INSERT INTO provider_health_log (provider, capability, workspace_id, healthy, response_time_ms, error_message, consecutive_failures, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![provider, capability, workspace_id, healthy, response_time.map(|t| t as i64), error, failures, now],
        ).map_err(|e| e.to_string())?;

        results.push(ProviderHealth {
            provider: provider.clone(),
            capability: capability.clone(),
            model: model.clone(),
            healthy,
            last_checked_at: Some(now.clone()),
            response_time_ms: response_time,
            error_message: error,
            consecutive_failures: failures,
        });
    }

    Ok(results)
}

/// Get current health status for all providers
pub fn get_provider_health(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<ProviderHealth>, String> {
    let mut stmt = connection.prepare(
        "SELECT h.provider, h.capability, COALESCE(r.model, f.model, 'unknown'),
                h.healthy, h.checked_at, h.response_time_ms, h.error_message, h.consecutive_failures
         FROM provider_health_log h
         LEFT JOIN model_routes r ON r.provider = h.provider AND r.capability = h.capability
         LEFT JOIN provider_fallback f ON f.provider = h.provider AND f.capability = h.capability
         WHERE h.workspace_id = ?1
         AND h.id IN (SELECT MAX(id) FROM provider_health_log WHERE workspace_id = ?1 GROUP BY provider, capability)
         ORDER BY h.capability, h.provider"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![workspace_id], |row| Ok(ProviderHealth {
        provider: row.get(0)?, capability: row.get(1)?, model: row.get(2)?,
        healthy: row.get(3)?, last_checked_at: row.get(4)?,
        response_time_ms: row.get(5)?, error_message: row.get(6)?,
        consecutive_failures: row.get(7)?,
    })).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
