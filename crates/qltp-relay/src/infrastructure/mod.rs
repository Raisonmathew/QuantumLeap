//! Infrastructure Layer - Technical Implementations
//!
//! This layer provides concrete implementations of the port interfaces
//! and handles technical concerns such as:
//! - Storage (in-memory, persistent)
//! - WebSocket server
//! - Event publishing
//! - Configuration
//! - Metrics and monitoring

pub mod storage;
pub mod websocket;
pub mod events;
pub mod config;
pub mod relay_service;
pub mod auth;
pub mod rate_limit;
pub mod metrics;
pub mod cascade;

pub use storage::{InMemoryPeerRepository, InMemorySessionRepository, InMemoryConnectionRepository};
pub use websocket::WebSocketServer;
pub use events::InMemoryEventPublisher;
pub use config::{RelayConfig, WebSocketConfig};
pub use relay_service::{RelayService, RelayServiceConfig, RelayServiceHandles};
pub use auth::{AuthManager, Credentials};
pub use rate_limit::{RateLimiter, RateLimitConfig, start_cleanup_task};
pub use metrics::{ServerMetrics, MetricsSnapshot};
pub use cascade::{ConnectionCascade, CascadeConfig, CascadeResult};

// Made with Bob
