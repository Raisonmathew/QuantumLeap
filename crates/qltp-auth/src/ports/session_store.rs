//! Session storage port (hexagonal architecture)

use crate::domain::{AuthToken, Session};
use crate::error::Result;

/// Port for session storage (hexagonal architecture interface)
/// 
/// This trait defines the contract for session storage implementations.
/// Different adapters can implement this trait to provide various storage backends:
/// - MemorySessionStore (in-memory, for development/testing)
/// - RedisSessionStore (distributed sessions)
/// - DatabaseSessionStore (persistent sessions)
pub trait SessionStore: Send + Sync {
    /// Save a session to storage
    fn save(&self, session: Session) -> Result<()>;

    /// Retrieve a session by token
    fn get(&self, token: &AuthToken) -> Result<Option<Session>>;

    /// Remove a session from storage
    fn remove(&self, token: &AuthToken) -> Result<()>;

    /// Clean up expired sessions and return count of removed sessions
    fn cleanup_expired(&self) -> Result<usize>;

    /// Get count of active sessions
    fn count(&self) -> Result<usize>;
}

// Made with Bob
