//! Ports Layer - Interfaces for External Dependencies
//!
//! This layer defines the interfaces (ports) that the application layer
//! depends on. Following the Dependency Inversion Principle, the application
//! layer depends on these abstractions, not on concrete implementations.
//!
//! ## Port Types
//!
//! - **Repositories**: Data persistence interfaces
//! - **Event Publishers**: Event notification interfaces
//! - **External Services**: Third-party service interfaces

pub mod repositories;
pub mod events;

pub use repositories::{ConnectionRepository, PeerRepository, SessionRepository};
pub use events::{DomainEvent, EventPublisher};

// Made with Bob
