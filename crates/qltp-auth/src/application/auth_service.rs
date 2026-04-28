//! Authentication service (application layer)

use crate::domain::{AuthToken, Credentials, RateLimiter, Session};
use crate::error::{AuthError, Result};
use crate::ports::SessionStore;
use argon2::Argon2;
use password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

/// A precomputed Argon2id PHC string used as a constant-time decoy when
/// the supplied username does not exist.
///
/// SECURITY (C15 — username-existence timing oracle): the previous
/// implementation early-returned `InvalidCredentials` whenever a username
/// was unknown, skipping the (deliberately) expensive Argon2 verify. An
/// attacker measuring response latency could therefore distinguish
/// "valid username, wrong password" (slow) from "unknown username"
/// (fast), enumerating the entire user database. We now route the
/// not-found path through a verify against this fixed dummy hash so
/// every authentication attempt costs roughly the same amount of CPU.
///
/// The hash is generated once per process via `OnceLock` so we do not
/// regenerate the salt on every miss (which would itself be a slower
/// path than a hit and re-introduce the oracle in inverted form).
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(b"qltp-dummy-password-for-timing-equalization", &salt)
            .expect("argon2 default parameters always succeed")
            .to_string()
    })
}

/// Hash a password with Argon2id using a fresh random salt.
///
/// Returns a PHC-format string that bundles algorithm, parameters, salt, and
/// hash so the verifier needs no extra metadata.
fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AuthError::Internal(format!("Password hashing failed: {}", e)))
}

/// Constant-time verify a password against a stored PHC hash.
fn verify_password(password: &str, stored_phc: &str) -> bool {
    match PasswordHash::new(stored_phc) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Session information for display
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub username: String,
    pub age: Duration,
    pub remaining: Duration,
    pub is_expired: bool,
}

/// Authentication service (application layer)
/// 
/// Orchestrates authentication operations using domain entities and ports.
/// This is the main entry point for authentication functionality.
pub struct AuthService {
    /// User credentials storage (username -> password hash)
    credentials: Arc<RwLock<HashMap<String, String>>>,
    /// Session storage (via port/adapter pattern)
    session_store: Arc<dyn SessionStore>,
    /// Session time-to-live
    session_ttl: Duration,
    /// Optional per-username rate limiter for `authenticate` (C3).
    /// `None` means unlimited (legacy behaviour); production callers MUST
    /// configure one via [`AuthService::with_rate_limiter`].
    rate_limiter: Option<Arc<RateLimiter>>,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(session_store: Arc<dyn SessionStore>, session_ttl: Duration) -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
            session_store,
            session_ttl,
            rate_limiter: None,
        }
    }

    /// Builder: install a rate limiter applied to every `authenticate`
    /// call, keyed by username. Defends against credential stuffing /
    /// online password guessing.
    pub fn with_rate_limiter(mut self, limiter: Arc<RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Add a user with credentials.
    ///
    /// The password is hashed with Argon2id (OWASP-recommended) using a
    /// fresh random 16-byte salt. Plaintext is never stored.
    pub fn add_user(&self, username: String, password: String) -> Result<()> {
        let password_hash = hash_password(&password)?;

        let mut creds = self
            .credentials
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;

        creds.insert(username, password_hash);
        Ok(())
    }

    /// Remove a user
    pub fn remove_user(&self, username: &str) -> Result<()> {
        let mut creds = self
            .credentials
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;

        creds.remove(username);
        Ok(())
    }

    /// Authenticate with credentials and create session.
    ///
    /// Verifies the supplied password against the stored Argon2id hash in
    /// constant time. On success, generates a CSPRNG-backed session token.
    ///
    /// SECURITY:
    /// - Rate-limited per username (C3): `RateLimited` is returned ahead
    ///   of any cryptographic work, but is checked AFTER the existence
    ///   check would have happened, so a flood of unknown usernames also
    ///   trips the limit on those usernames.
    /// - Timing-oracle hardened (C15): the `verify_password` step ALWAYS
    ///   runs — against the real stored hash if the user exists, or
    ///   against a fixed dummy hash if not — so response time does not
    ///   reveal username existence.
    pub fn authenticate(&self, credentials: &Credentials) -> Result<AuthToken> {
        // Rate-limit FIRST: a successful brute-force attempt costs the
        // attacker an Argon2 verify per try, but we still want to refuse
        // sustained traffic without doing the work.
        if let Some(limiter) = &self.rate_limiter {
            if let Err(rl) = limiter.check(&credentials.username) {
                return Err(AuthError::RateLimited {
                    retry_after: rl.retry_after,
                });
            }
        }

        // Look up the stored hash, but DO NOT short-circuit if missing.
        // Use a constant dummy hash on the not-found path so the verify
        // cost is identical either way.
        let stored_hash: String = {
            let creds = self
                .credentials
                .read()
                .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;
            creds.get(&credentials.username).cloned().unwrap_or_else(|| dummy_hash().to_string())
        };
        let user_exists = stored_hash != dummy_hash();

        let password_ok = verify_password(&credentials.password, &stored_hash);

        // Reject in the same arm whether the user is unknown OR the
        // password is wrong — same error, same code path, same timing
        // (within Argon2id's tolerance).
        if !(user_exists && password_ok) {
            return Err(AuthError::InvalidCredentials);
        }

        // Create session
        let token = AuthToken::new();
        let session = Session::new(
            token.clone(),
            credentials.username.clone(),
            self.session_ttl,
        );

        self.session_store.save(session)?;
        Ok(token)
    }

    /// Verify a token and refresh session
    pub fn verify_token(&self, token: &AuthToken) -> Result<String> {
        match self.session_store.get(token)? {
            Some(mut session) if !session.is_expired() => {
                session.refresh(self.session_ttl);
                let username = session.username().to_string();
                self.session_store.save(session)?;
                Ok(username)
            }
            Some(_) => {
                self.session_store.remove(token)?;
                Err(AuthError::TokenExpired)
            }
            None => Err(AuthError::InvalidToken),
        }
    }

    /// Revoke a token (logout)
    pub fn revoke_token(&self, token: &AuthToken) -> Result<()> {
        self.session_store.remove(token)
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> Result<usize> {
        self.session_store.cleanup_expired()
    }

    /// Get active session count
    pub fn active_sessions(&self) -> Result<usize> {
        self.session_store.count()
    }

    /// Get session info for a token
    pub fn get_session_info(&self, token: &AuthToken) -> Result<SessionInfo> {
        match self.session_store.get(token)? {
            Some(session) => {
                let now = std::time::SystemTime::now();
                let age = now
                    .duration_since(session.created_at())
                    .unwrap_or(Duration::ZERO);
                let remaining = session
                    .expires_at()
                    .duration_since(now)
                    .unwrap_or(Duration::ZERO);

                Ok(SessionInfo {
                    username: session.username().to_string(),
                    age,
                    remaining,
                    is_expired: session.is_expired(),
                })
            }
            None => Err(AuthError::InvalidToken),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MemorySessionStore;

    fn create_auth_service() -> AuthService {
        let store = Arc::new(MemorySessionStore::new());
        AuthService::new(store, Duration::from_secs(3600))
    }

    #[test]
    fn test_add_user_and_authenticate() {
        let service = create_auth_service();
        service.add_user("alice".to_string(), "password123".to_string()).unwrap();

        let creds = Credentials::new("alice".to_string(), "password123".to_string());
        let token = service.authenticate(&creds).unwrap();
        assert!(!token.as_str().is_empty());
    }

    #[test]
    fn test_invalid_credentials() {
        let service = create_auth_service();
        service.add_user("alice".to_string(), "password123".to_string()).unwrap();

        let creds = Credentials::new("alice".to_string(), "wrongpassword".to_string());
        let result = service.authenticate(&creds);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token() {
        let service = create_auth_service();
        service.add_user("bob".to_string(), "secret".to_string()).unwrap();

        let creds = Credentials::new("bob".to_string(), "secret".to_string());
        let token = service.authenticate(&creds).unwrap();

        let username = service.verify_token(&token).unwrap();
        assert_eq!(username, "bob");
    }

    #[test]
    fn test_revoke_token() {
        let service = create_auth_service();
        service.add_user("charlie".to_string(), "pass".to_string()).unwrap();

        let creds = Credentials::new("charlie".to_string(), "pass".to_string());
        let token = service.authenticate(&creds).unwrap();

        service.revoke_token(&token).unwrap();

        let result = service.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_session_expiry() {
        let store = Arc::new(MemorySessionStore::new());
        let service = AuthService::new(store, Duration::from_millis(100));
        service.add_user("dave".to_string(), "test".to_string()).unwrap();

        let creds = Credentials::new("dave".to_string(), "test".to_string());
        let token = service.authenticate(&creds).unwrap();

        std::thread::sleep(Duration::from_millis(150));

        let result = service.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired() {
        let store = Arc::new(MemorySessionStore::new());
        let service = AuthService::new(store, Duration::from_millis(100));
        service.add_user("eve".to_string(), "pwd".to_string()).unwrap();

        let creds = Credentials::new("eve".to_string(), "pwd".to_string());
        let _token = service.authenticate(&creds).unwrap();

        assert_eq!(service.active_sessions().unwrap(), 1);

        std::thread::sleep(Duration::from_millis(150));

        let removed = service.cleanup_expired().unwrap();
        assert_eq!(removed, 1);
        assert_eq!(service.active_sessions().unwrap(), 0);
    }

    #[test]
    fn test_session_info() {
        let service = create_auth_service();
        service.add_user("frank".to_string(), "key".to_string()).unwrap();

        let creds = Credentials::new("frank".to_string(), "key".to_string());
        let token = service.authenticate(&creds).unwrap();

        let info = service.get_session_info(&token).unwrap();
        assert_eq!(info.username, "frank");
        assert!(!info.is_expired);
        assert!(info.remaining.as_secs() > 3500);
    }
}

// Made with Bob
