//! QLTP Relay Service
//!
//! Unified cloud relay service implementing:
//! - Signaling (peer coordination via WebSocket)
//! - Connection Management (intelligent cascade with NAT traversal)
//! - STUN (NAT discovery)
//! - TURN (relay server)
//!
//! ## Architecture
//!
//! This crate follows Domain-Driven Design (DDD) and Hexagonal Architecture:
//!
//! - `domain/` - Pure business logic (entities, value objects, domain services)
//! - `application/` - Use cases and orchestration
//! - `ports/` - Interfaces for external dependencies
//! - `adapters/` - Implementations (WebSocket, STUN, TURN)
//! - `infrastructure/` - Technical concerns (config, metrics, logging)
//! - `protocol/` - Message formats and serialization

// Error types
pub mod error;

// Domain layer - Pure business logic
pub mod domain;

// Application layer - Use cases and orchestration
pub mod application;

// Ports layer - Interfaces for external dependencies
pub mod ports;

// Adapters layer - Protocol handlers and message routing
pub mod adapters;

// Infrastructure layer - Technical implementations
pub mod infrastructure;

// STUN/TURN implementation
pub mod stun;
pub mod turn;

// Re-export commonly used types
pub use domain::{NatCompatibility, NatType, PeerId, SessionId};
pub use application::{PeerService, PeerServiceError};
pub use error::{Error, Result};
pub use ports::{DomainEvent, EventPublisher, PeerRepository, SessionRepository, ConnectionRepository};
pub use adapters::{MessageHandler, SignalingMessage, SignalingResponse};
pub use infrastructure::{
    InMemoryConnectionRepository, InMemoryEventPublisher, InMemoryPeerRepository,
    InMemorySessionRepository, RelayConfig, RelayService, RelayServiceConfig,
    RelayServiceHandles, WebSocketServer,
};

// Made with Bob
