//! Application layer - Use cases and orchestration

pub mod backend_monitor;
pub mod backend_selector;
pub mod fallback_manager;
pub mod transport_manager;
pub mod transfer_client;
pub mod transfer_server;

pub use backend_monitor::{
    BackendMetrics, BackendMonitor, HealthCheckResult, HealthStatus, MonitorConfig, MonitorSummary,
};
pub use backend_selector::{BackendSelector, SelectionCriteria, SelectionResult};
pub use fallback_manager::{FallbackAttempt, FallbackManager, FallbackResult, RetryConfig};
pub use transport_manager::{TransportManager, TransportManagerConfig};
pub use transfer_client::TransferClient;
pub use transfer_server::TransferServer;

// Made with Bob
