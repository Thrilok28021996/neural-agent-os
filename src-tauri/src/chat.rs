// ── Provider-aware chat routing ─────────────────────────────────────────────
// `chat_completion` resolves the workspace's provider + model route for a
// capability and dispatches to the right backend: Ollama, OpenAI, Anthropic,
// Google, or any local OpenAI-compatible runtime (LM Studio, llama.cpp, vLLM,
// OpenCode, arbitrary endpoints). Cloud calls are gated by cost limits,
// token-tracked, and written to the egress audit log.
use rusqlite::Connection;

/// Normalize a provider string (from model_routes or UI labels/ids) into a
/// canonical provider id.
pub fn normalize_provider(provider: &str) -> String {
    let p = provider.trim().to_lowercase().replace(" (cloud)", "").replace("_", " ");
    let p = p.trim();
    match p {
        "openai" => "openai".to_string(),
        "anthropic" => "anthropic".to_string(),
        "google" => "google".to_string(),
        "ollama" | "local" => "ollama".to_string(),
        "lm studio" | "lmstudio" | "llama.cpp" | "llamacpp" | "llama cpp"
        | "vllm" | "v llm" | "opencode" | "opencode go plan"
        | "openai compatible" | "openai-compatible" | "openai-compatible endpoint"
        | "openai compatible endpoint" => "openai_compatible".to_string(),
        _ => "ollama".to_string(),
    }
}

pub fn is_cloud_provider(provider: &str) -> bool {
    matches!(provider, "openai" | "anthropic" | "google")
}

/// Resolve the workspace provider for a capability (falls back to ollama).
pub(crate) fn routed_provider(connection: &Connection, workspace_id: &str, capability: &str) -> String {
    connection
        .query_row(
            "SELECT provider FROM model_routes WHERE workspace_id = ?1 AND capability = ?2",
            rusqlite::params![workspace_id, capability],
            |row| row.get::<_, String>(0),
        )
        .map(|p| normalize_provider(&p))
        .unwrap_or_else(|_| "openai_compatible".to_string()) // default: LM Studio / local OpenAI-compatible
}

/// Get the API key for a cloud provider: environment variable first, then the
/// OS keychain (stored via store_provider_secret as `<provider>:api_key`).
pub(crate) fn provider_api_key(provider: &str) -> Result<String, String> {
    let env_name = match provider {
        "openai" => "NEURAL_OPENAI_API_KEY",
        "anthropic" => "NEURAL_ANTHROPIC_API_KEY",
        "google" => "NEURAL_GOOGLE_API_KEY",
        "openai_compatible" => "NEURAL_OPENAI_COMPATIBLE_API_KEY",
        _ => "",
    };
    if !env_name.is_empty() {
        if let Ok(key) = std::env::var(env_name) {
            if !key.trim().is_empty() {
                return Ok(key);
            }
        }
    }
    keyring::Entry::new("neural-agent-os/provider", &format!("{provider}:api_key"))
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|_| {
            format!(
                "No API key stored for {provider}. Add one in Settings → Models (or set {})",
                if env_name.is_empty() { "the environment variable".to_string() } else { env_name.to_string() }
            )
        })
}

fn chat_ollama(model: &str, prompt: &str) -> Result<String, String> {
    let endpoint = std::env::var("NEURAL_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/generate".into());
    let response = reqwest::blocking::Client::new()
        .post(&endpoint)
        .json(&serde_json::json!({ "model": model, "prompt": prompt, "stream": false }))
        .send()
        .map_err(|e| { log::error!("chat_ollama: request to {endpoint} failed: {e}"); format!("Ollama is unavailable at {endpoint}: {e}. Start Ollama and pull the configured model.") })?
        .error_for_status()
        .map_err(|e| { log::error!("chat_ollama: HTTP error from {endpoint} for model {model}: {e}"); e.to_string() })?
        .json::<crate::OllamaResponse>()
        .map_err(|e| { log::error!("chat_ollama: bad JSON from {endpoint}: {e}"); e.to_string() })?;
    Ok(response.response)
}



/// Retry once on 5xx / 429 (rate limit) responses with a short backoff.
fn send_with_retry<F>(send: F) -> Result<reqwest::blocking::Response, String>
where
    F: Fn() -> Result<reqwest::blocking::Response, reqwest::Error>,
{
    let first = send().map_err(|e| e.to_string())?;
    if first.status().is_server_error() || first.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        std::thread::sleep(std::time::Duration::from_millis(800));
        send().map_err(|e| e.to_string())
    } else {
        Ok(first)
    }
}

fn chat_openai(model: &str, prompt: &str) -> Result<String, String> {
    let api_key = provider_api_key("openai")?;
    let client = reqwest::blocking::Client::new();
    let response = send_with_retry(|| client.post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&api_key)
        .json(&serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": 0.7,
        }))
        .send())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    response["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "OpenAI returned no content".to_string())
}

fn chat_anthropic(model: &str, prompt: &str) -> Result<String, String> {
    let api_key = provider_api_key("anthropic")?;
    let client = reqwest::blocking::Client::new();
    let response = send_with_retry(|| client.post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": model,
            "max_tokens": 2048,
            "messages": [{ "role": "user", "content": prompt }],
        }))
        .send())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    response["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Anthropic returned no content".to_string())
}

fn chat_google(model: &str, prompt: &str) -> Result<String, String> {
    let api_key = provider_api_key("google")?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );
    let client = reqwest::blocking::Client::new();
    let response = send_with_retry(|| client.post(&url)
        .json(&serde_json::json!({ "contents": [{ "parts": [{ "text": prompt }] }] }))
        .send())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Google returned no content".to_string())
}

fn chat_openai_compatible(model: &str, prompt: &str) -> Result<String, String> {
    let base = std::env::var("NEURAL_OPENAI_COMPATIBLE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:1234/v1".into());
    let url = if base.ends_with("/chat/completions") {
        base
    } else if base.ends_with('/') {
        format!("{base}chat/completions")
    } else {
        format!("{base}/chat/completions")
    };
    let client = reqwest::blocking::Client::new();
    let json_body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "stream": false,
        "temperature": 0.7,
    });
    let response = send_with_retry(|| {
        let mut req = client.post(&url);
        if let Ok(key) = provider_api_key("openai_compatible") {
            req = req.bearer_auth(&key);
        }
        req.json(&json_body).send()
    })
    .map_err(|e| format!("OpenAI-compatible endpoint {url} unreachable: {e}. Set NEURAL_OPENAI_COMPATIBLE_URL to your LM Studio / llama.cpp / vLLM server."))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    response["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "OpenAI-compatible endpoint returned no content".to_string())
}

/// Run a chat completion for a workspace capability using the configured
/// provider + model route. Records cost and egress audit for cloud providers.
pub fn chat_completion(
    connection: &Connection,
    workspace_id: &str,
    capability: &str,
    fallback_model: &str,
    prompt: &str,
) -> Result<String, String> {
    let provider = routed_provider(connection, workspace_id, capability);
    let model = crate::routed_model(connection, workspace_id, capability, fallback_model);
    log::info!("chat_completion: workspace={workspace_id} capability={capability} provider={provider} model={model} prompt_chars={}", prompt.len());

    // Cost-limit gate for cloud providers (local runtimes are free).
    if is_cloud_provider(&provider) {
        if let Ok(Some(limit)) = crate::costs::get_cost_limit(connection, workspace_id, capability) {
            if crate::costs::check_cost_limit(connection, workspace_id, limit)? {
                return Err(format!(
                    "{capability} cost limit of {} cents exceeded for this workspace. Switch to a local model or raise the limit.",
                    limit
                ));
            }
        }
    }

    let response = match provider.as_str() {
        "openai" => chat_openai(&model, prompt)?,
        "anthropic" => chat_anthropic(&model, prompt)?,
        "google" => chat_google(&model, prompt)?,
        "openai_compatible" => chat_openai_compatible(&model, prompt)?,
        _ => chat_ollama(&model, prompt)?,
    };
    log::info!("chat_completion: {provider}/{model} returned {} chars", response.len());

    // Cost tracking + comprehensive egress audit for cloud providers.
    if is_cloud_provider(&provider) {
        let _ = crate::costs::record_token_usage(
            connection,
            workspace_id,
            capability,
            &provider,
            &model,
            prompt.len() as i64 / 4,
            response.len() as i64 / 4,
        );
        let _ = crate::retention::record_audit(
            connection,
            "cloud_chat",
            Some(&provider),
            Some("prompt_and_response"),
            Some(workspace_id),
            &format!("Sent prompt ({} chars) to {}/{} — response {} chars", prompt.len(), provider, model, response.len()),
        );
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_cloud_provider_names() {
        assert_eq!(normalize_provider("OpenAI"), "openai");
        assert_eq!(normalize_provider("openai"), "openai");
        assert_eq!(normalize_provider("Anthropic"), "anthropic");
        assert_eq!(normalize_provider("Google"), "google");
        assert_eq!(normalize_provider(" Google "), "google");
    }

    #[test]
    fn normalizes_local_provider_names() {
        assert_eq!(normalize_provider("Ollama"), "ollama");
        assert_eq!(normalize_provider("Local"), "ollama");
        assert_eq!(normalize_provider("LM Studio"), "openai_compatible");
        assert_eq!(normalize_provider("lmstudio"), "openai_compatible");
        assert_eq!(normalize_provider("llama.cpp"), "openai_compatible");
        assert_eq!(normalize_provider("vLLM"), "openai_compatible");
        assert_eq!(normalize_provider("OpenCode Go plan (cloud)"), "openai_compatible");
        assert_eq!(normalize_provider("OpenAI-compatible endpoint"), "openai_compatible");
        // Unknown providers fall back to ollama (local default)
        assert_eq!(normalize_provider("anything-else"), "ollama");
    }

    #[test]
    fn cloud_provider_detection() {
        assert!(is_cloud_provider("openai"));
        assert!(is_cloud_provider("anthropic"));
        assert!(is_cloud_provider("google"));
        assert!(!is_cloud_provider("ollama"));
        assert!(!is_cloud_provider("openai_compatible"));
    }

    #[test]
    fn chat_routing_uses_workspace_model_route() {
        let conn = crate::tests::test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        conn.execute(
            "INSERT INTO model_routes (workspace_id, capability, provider, model) VALUES ('w1', 'chat', 'OpenAI', 'gpt-4o-mini')",
            [],
        ).unwrap();
        // Provider + model resolve from the route; the call itself would hit the
        // network, so we only verify resolution helpers.
        assert_eq!(routed_provider(&conn, "w1", "chat"), "openai");
        assert_eq!(crate::routed_model(&conn, "w1", "chat", "qwen3:14b"), "gpt-4o-mini");
        assert_eq!(routed_provider(&conn, "w1", "embeddings"), "openai_compatible"); // unset -> LM Studio default
    }
}
