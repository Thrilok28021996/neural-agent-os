use serde::{Serialize, Deserialize};


#[derive(Serialize)]
pub struct OAuthConfig {
    pub provider: String,
    pub authorize_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub redirect_uri: String,
}

#[derive(Serialize, Deserialize)]
pub struct PKCEParams {
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
}

/// Preconfigured OAuth client IDs — these are built into the application
/// so users don't need to create their own Google Cloud / Azure projects.
const PRECONFIGURED_CLIENTS: &[(&str, &str, &str)] = &[
    ("google", "neural-agent-os-google", "https://accounts.google.com/o/oauth2/v2/auth"),
    ("outlook", "neural-agent-os-outlook", "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"),
];

/// Get preconfigured OAuth config for a provider.
/// Falls back to environment variables if preconfigured client is unavailable.
pub fn get_oauth_config(provider: &str) -> Result<OAuthConfig, String> {
    log::debug!("oauth::get_oauth_config: enter");
    let env_client_id = match provider {
        "google" => std::env::var("NEURAL_GOOGLE_CLIENT_ID").ok(),
        "outlook" => std::env::var("NEURAL_MICROSOFT_CLIENT_ID").ok(),
        _ => None,
    };

    let client_id = env_client_id.unwrap_or_else(|| {
        PRECONFIGURED_CLIENTS
            .iter()
            .find(|(p, _, _)| *p == provider)
            .map(|(_, id, _)| id.to_string())
            .unwrap_or_default()
    });

    if client_id.is_empty() {
        return Err(format!("No OAuth client ID configured for {provider}. Set NEURAL_{}_CLIENT_ID environment variable.", provider.to_uppercase()));
    }

    match provider {
        "google" => Ok(OAuthConfig {
            provider: "google".into(),
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: vec![
                "https://www.googleapis.com/auth/calendar.readonly".into(),
                "https://www.googleapis.com/auth/calendar.events".into(),
                "https://www.googleapis.com/auth/gmail.readonly".into(),
                "https://www.googleapis.com/auth/gmail.send".into(),
                "https://www.googleapis.com/auth/gmail.modify".into(),
            ],
            client_id,
            redirect_uri: "http://127.0.0.1:8787/oauth/callback".into(),
        }),
        "outlook" => Ok(OAuthConfig {
            provider: "outlook".into(),
            authorize_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".into(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".into(),
            scopes: vec![
                "Calendars.Read".into(),
                "Calendars.ReadWrite".into(),
                "Mail.Read".into(),
                "Mail.Send".into(),
                "offline_access".into(),
            ],
            client_id,
            redirect_uri: "http://127.0.0.1:8787/oauth/callback".into(),
        }),
        _ => Err(format!("Unsupported OAuth provider: {provider}")),
    }
}

/// Generate PKCE code verifier and challenge
pub fn generate_pkce() -> PKCEParams {
    log::info!("oauth::generate_pkce: enter");
    use rand::Rng;

    // Code verifier: 128 random URL-safe chars
    let verifier: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(96)
        .map(char::from)
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();

    // Code challenge: SHA256(verifier) base64url
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    let challenge = base64_url_encode(&hash);

    let state = uuid::Uuid::new_v4().to_string();

    PKCEParams {
        code_verifier: verifier,
        code_challenge: challenge,
        state,
    }
}

fn base64_url_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((combined >> 18) & 63) as usize] as char);
        result.push(CHARS[((combined >> 12) & 63) as usize] as char);
        if chunk.len() > 1 { result.push(CHARS[((combined >> 6) & 63) as usize] as char); }
        else { result.push('='); }
        if chunk.len() > 2 { result.push(CHARS[(combined & 63) as usize] as char); }
        else { result.push('='); }
    }
    result.trim_end_matches('=').to_string()
}

/// Build the full authorization URL with PKCE
pub fn build_authorize_url(provider: &str, pkce: &PKCEParams) -> Result<String, String> {
    log::info!("oauth::build_authorize_url: enter");
    let config = get_oauth_config(provider)?;
    let scope = config.scopes.join(" ");
    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&\
         scope={}&code_challenge={}&code_challenge_method=S256&\
         state={}&access_type=offline&prompt=consent",
        config.authorize_url, config.client_id, config.redirect_uri,
        scope, pkce.code_challenge, pkce.state,
    );
    Ok(url)
}

/// Exchange authorization code for tokens using PKCE
pub fn exchange_code_with_pkce(
    provider: &str,
    code: &str,
    pkce: &PKCEParams,
) -> Result<(String, String), String> {
    log::info!("oauth::exchange_code_with_pkce: enter");
    let config = get_oauth_config(provider)?;
    let client_secret = match provider {
        "google" => std::env::var("NEURAL_GOOGLE_CLIENT_SECRET").unwrap_or_default(),
        "outlook" => std::env::var("NEURAL_MICROSOFT_CLIENT_SECRET").unwrap_or_default(),
        _ => return Err(format!("Unsupported provider: {provider}")),
    };

    if client_secret.is_empty() {
        return Err(format!("OAuth client secret not configured for {provider}"));
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&config.token_url)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code),
            ("code_verifier", pkce.code_verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &config.redirect_uri),
        ])
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("Token exchange failed: {e}"))?
        .json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;

    let access_token = response["access_token"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let refresh_token = response["refresh_token"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if access_token.is_empty() {
        return Err("No access token received".into());
    }

    // Store refresh token in OS keychain
    if !refresh_token.is_empty() {
        let entry = keyring::Entry::new(
            "neural-agent-os/oauth",
            &format!("{provider}:refresh_token"),
        )
        .map_err(|e| e.to_string())?;
        entry
            .set_password(&refresh_token)
            .map_err(|e| e.to_string())?;
    }

    Ok((access_token, refresh_token))
}

/// Refresh an access token using the stored refresh token
pub fn refresh_access_token(provider: &str) -> Result<String, String> {
    log::info!("oauth::refresh_access_token: enter");
    let config = get_oauth_config(provider)?;
    let client_secret = match provider {
        "google" => std::env::var("NEURAL_GOOGLE_CLIENT_SECRET").unwrap_or_default(),
        "outlook" => std::env::var("NEURAL_MICROSOFT_CLIENT_SECRET").unwrap_or_default(),
        _ => return Err(format!("Unsupported provider: {provider}")),
    };

    let entry = keyring::Entry::new(
        "neural-agent-os/oauth",
        &format!("{provider}:refresh_token"),
    )
    .map_err(|e| e.to_string())?;

    let refresh_token = entry.get_password().map_err(|e| format!(
        "No refresh token found for {provider}. Re-authorize the application: {e}"
    ))?;

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&config.token_url)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("Token refresh failed: {e}"))?
        .json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;

    let access_token = response["access_token"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if access_token.is_empty() {
        return Err("No access token in refresh response".into());
    }

    // Update stored refresh token if new one provided
    if let Some(new_refresh) = response["refresh_token"].as_str() {
        entry
            .set_password(new_refresh)
            .map_err(|e| e.to_string())?;
    }

    Ok(access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_generates_verifier_and_challenge() {
        let pkce = generate_pkce();
        assert!(pkce.code_verifier.len() >= 43);
        assert!(!pkce.code_challenge.is_empty());
        assert!(!pkce.state.is_empty());
        // Verifier must be URL-safe
        assert!(pkce.code_verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn base64_url_encoding_matches_standard() {
        // "hello" -> base64url without padding
        assert_eq!(base64_url_encode(b"hello"), "aGVsbG8");
        assert_eq!(base64_url_encode(b""), "");
        assert_eq!(base64_url_encode(b"a"), "YQ");
        assert_eq!(base64_url_encode(b"ab"), "YWI");
    }

    #[test]
    fn authorize_url_contains_pkce_params() {
        let pkce = generate_pkce();
        let url = build_authorize_url("google", &pkce).unwrap();
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&pkce.code_challenge));
        assert!(url.contains(&pkce.state));
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let pkce = generate_pkce();
        assert!(build_authorize_url("not-a-provider", &pkce).is_err());
    }
}
