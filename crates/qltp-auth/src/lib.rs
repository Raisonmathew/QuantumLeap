//! QLTP Authentication Module
//!
//! Provides token-based authentication and session management following
//! Domain-Driven Design (DDD) and Hexagonal Architecture principles.
//!
//! # Architecture
//!
//! This crate is organized into layers:
//!
//! - **Domain Layer**: Core business entities and value objects
//!   - `AuthToken`: Authentication token entity
//!   - `Session`: Session entity with expiration
//!   - `Credentials`: Username/password value object
//!
//! - **Ports Layer**: Interfaces for hexagonal architecture
//!   - `SessionStore`: Port for session storage implementations
//!
//! - **Adapters Layer**: Infrastructure implementations
//!   - `MemorySessionStore`: In-memory session storage
//!
//! - **Application Layer**: Use cases and services
//!   - `AuthService`: Main authentication service
//!
//! # Example
//!
//! ```rust
//! use qltp_auth::{AuthService, Credentials, MemorySessionStore};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! // Create auth service with in-memory storage
//! let store = Arc::new(MemorySessionStore::new());
//! let auth_service = AuthService::new(store, Duration::from_secs(3600));
//!
//! // Add a user
//! auth_service.add_user("alice".to_string(), "password123".to_string()).unwrap();
//!
//! // Authenticate
//! let creds = Credentials::new("alice".to_string(), "password123".to_string());
//! let token = auth_service.authenticate(&creds).unwrap();
//!
//! // Verify token
//! let username = auth_service.verify_token(&token).unwrap();
//! assert_eq!(username, "alice");
//! ```

pub mod domain;
pub mod application;
pub mod ports;
pub mod adapters;
pub mod error;

// Re-export main types for convenience
pub use domain::{AuthToken, Credentials, Session};
pub use application::{AuthService, SessionInfo};
pub use ports::SessionStore;
pub use adapters::MemorySessionStore;
pub use error::{AuthError, Result};

// Convenience alias for backward compatibility with qltp-network
/// Alias for `AuthService` to maintain backward compatibility
pub type AuthManager = AuthService;

// Made with Bob
