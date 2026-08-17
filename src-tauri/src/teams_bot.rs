// ── Teams bot: shared-secret auth + recording decryption ────────────────────
// The Docker-deployed Teams bot authenticates to the local API with a shared
// secret (`NAO_TEAMS_BOT_SECRET`, configured identically on both sides) and
// uploads recordings encrypted with AES-256-GCM. Key material is derived from
// the shared secret with SHA-256, so one secret controls both auth and
// confidentiality. Payload layout (bot -> app):
//     nonce(12 bytes) || AES-256-GCM ciphertext || auth tag(16 bytes)
use sha2::{Digest, Sha256};

/// Derive the 32-byte AES-256 key from the shared Teams-bot secret.
pub fn derive_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.finalize().into()
}

/// Shared Teams-bot secret from the environment, if configured.
pub fn shared_secret() -> Option<String> {
    std::env::var("NAO_TEAMS_BOT_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
}

/// Constant-time string comparison: no early exit, so response timing does not
/// reveal how many prefix bytes matched.
pub fn tokens_equal(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Validate a presented token against the configured shared secret.
/// Returns Ok(()) when valid; Err(reason) otherwise. When the secret is not
/// configured the endpoints stay locked down — there is no unauthenticated
/// fallback path.
pub fn validate_token_with_secret(secret: Option<&str>, provided: &str) -> Result<(), String> {
    // An empty/whitespace secret is the same as "not configured": locked down.
    match secret.map(str::trim).filter(|s| !s.is_empty()) {
        None => Err(
            "Teams-bot auth is not configured (set NAO_TEAMS_BOT_SECRET on both the app and the bot)"
                .into(),
        ),
        Some(secret) if tokens_equal(provided.trim(), secret) => Ok(()),
        Some(_) => Err("Invalid Teams-bot token".into()),
    }
}

/// Validate a token against `NAO_TEAMS_BOT_SECRET` from the environment.
pub fn validate_token(provided: &str) -> Result<(), String> {
    validate_token_with_secret(shared_secret().as_deref(), provided)
}

/// Decrypt a bot recording payload: nonce(12) || ciphertext+tag.
/// Returns the plaintext recording bytes, or Err on malformed/tampered input.
pub fn decrypt_recording(key: &[u8; 32], payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.len() < 12 + 16 {
        return Err(format!(
            "Encrypted payload too short ({} bytes; expected >= 28)",
            payload.len()
        ));
    }
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    let (nonce_bytes, ciphertext) = payload.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Recording decryption/authentication failed (wrong key or corrupted payload)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Key, Nonce};

    fn encrypt(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let mut out = nonce.to_vec();
        out.extend_from_slice(
            &cipher
                .encrypt(Nonce::from_slice(nonce), plaintext)
                .expect("encrypt"),
        );
        out
    }

    #[test]
    fn derive_key_is_deterministic_32_bytes() {
        let a = derive_key("hunter2-secret");
        let b = derive_key("hunter2-secret");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
        let c = derive_key("different-secret");
        assert_ne!(a, c);
    }

    #[test]
    fn tokens_equal_is_constant_time_and_correct() {
        assert!(tokens_equal("abc", "abc"));
        assert!(!tokens_equal("abc", "abd"));
        assert!(!tokens_equal("abc", "abcd"));
        assert!(!tokens_equal("", "x"));
        assert!(tokens_equal("", ""));
    }

    #[test]
    fn validate_token_with_secret_accepts_only_the_secret() {
        assert!(validate_token_with_secret(Some("s3cret"), "s3cret").is_ok());
        assert!(validate_token_with_secret(Some("s3cret"), "wrong").is_err());
        assert!(validate_token_with_secret(Some("s3cret"), "s3cret ").is_ok(), "trims surrounding whitespace");
        assert!(validate_token_with_secret(None, "anything").is_err(), "no secret configured -> locked down");
        assert!(validate_token_with_secret(Some(""), "").is_err());
    }

    #[test]
    fn decrypt_recording_roundtrips_and_detects_tampering() {
        let key = derive_key("roundtrip-secret");
        let nonce = [7u8; 12];
        let plaintext = b"RIFF....simulated wav bytes";
        let payload = encrypt(&key, &nonce, plaintext);
        let decrypted = decrypt_recording(&key, &payload).expect("decrypt");
        assert_eq!(decrypted, plaintext);

        // Tamper with one ciphertext byte -> authentication must fail.
        let mut tampered = payload.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(decrypt_recording(&key, &tampered).is_err());

        // Wrong key -> failure.
        let other_key = derive_key("other-secret");
        assert!(decrypt_recording(&other_key, &payload).is_err());

        // Truncated payload -> clear error, not a panic.
        assert!(decrypt_recording(&key, &payload[..10]).is_err());
        assert!(decrypt_recording(&key, &[]).is_err());
    }

    #[test]
    fn shared_secret_reads_env() {
        let _env_guard = crate::tests::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: tests in this module are the only users of this env var.
        std::env::set_var("NAO_TEAMS_BOT_SECRET", "env-secret");
        assert_eq!(shared_secret().as_deref(), Some("env-secret"));
        assert!(validate_token("env-secret").is_ok());
        assert!(validate_token("nope").is_err());
        std::env::remove_var("NAO_TEAMS_BOT_SECRET");
        assert!(shared_secret().is_none());
        assert!(validate_token("env-secret").is_err());
    }
}
