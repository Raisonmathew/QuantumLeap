//! In-memory session store adapter

use crate::domain::{AuthToken, Session};
use crate::error::{AuthError, Result};
use crate::ports::SessionStore;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory session store adapter
/// 
/// This adapter stores sessions in memory using a HashMap.
/// Suitable for development, testing, and single-instance deployments.
/// For production with multiple instances, consider RedisSessionStore.
pub struct MemorySessionStore {
    sessions: Arc<RwLock<HashMap<AuthToken, Session>>>,
}

impl MemorySessionStore {
    /// Create a new in-memory session store
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore for MemorySessionStore {
    fn save(&self, session: Session) -> Result<()> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;
        
        sessions.insert(session.token().clone(), session);
        Ok(())
    }

    fn get(&self, token: &AuthToken) -> Result<Option<Session>> {
        let sessions = self
            .sessions
            .read()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;
        
        Ok(sessions.get(token).cloned())
    }

    fn remove(&self, token: &AuthToken) -> Result<()> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;
        
        sessions.remove(token);
        Ok(())
    }

    fn cleanup_expired(&self) -> Result<usize> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;
        
        let before = sessions.len();
        sessions.retain(|_, session| !session.is_expired());
        Ok(before - sessions.len())
    }

    fn count(&self) -> Result<usize> {
        let sessions = self
            .sessions
            .read()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;
        
        Ok(sessions.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_memory_store_save_and_get() {
        let store = MemorySessionStore::new();
        let token = AuthToken::new();
        let session = Session::new(token.clone(), "alice".to_string(), Duration::from_secs(3600));
        
        store.save(session.clone()).unwrap();
        let retrieved = store.get(&token).unwrap();
        
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().username(), "alice");
    }

    #[test]
    fn test_memory_store_remove() {
        let store = MemorySessionStore::new();
        let token = AuthToken::new();
        let session = Session::new(token.clone(), "bob".to_string(), Duration::from_secs(3600));
        
        store.save(session).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        
        store.remove(&token).unwrap();
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn test_memory_store_cleanup_expired() {
        let store = MemorySessionStore::new();
        let token = AuthToken::new();
        let session = Session::new(token, "charlie".to_string(), Duration::from_millis(10));
        
        store.save(session).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        
        std::thread::sleep(Duration::from_millis(20));
        
        let removed = store.cleanup_expired().unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.count().unwrap(), 0);
    }
}

// Made with Bob
