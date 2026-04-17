//! Adapters layer
//!
//! Provides concrete implementations of the ports (interfaces) defined
//! in the ports layer, following the Hexagonal Architecture pattern.

pub mod memory_license_store;
pub mod memory_usage_store;

#[cfg(feature = "sqlite")]
pub mod sqlite_license_store;
#[cfg(feature = "sqlite")]
pub mod sqlite_usage_store;

pub use memory_license_store::MemoryLicenseStore;
pub use memory_usage_store::MemoryUsageStore;

#[cfg(feature = "sqlite")]
pub use sqlite_license_store::SqliteLicenseStore;
#[cfg(feature = "sqlite")]
pub use sqlite_usage_store::SqliteUsageStore;

// Made with Bob
