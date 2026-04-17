//! QLTP Licensing Module
//!
//! Provides licensing and access control for QLTP following
//! Domain-Driven Design (DDD) and Hexagonal Architecture principles.
//!
//! # Architecture
//!
//! This crate is organized into layers:
//!
//! - **Domain Layer**: Core business entities and value objects
//!   - License: Main aggregate root with tier, features, devices
//!   - Device: Activated device tracking
//!   - LicenseKey: Secure key generation and validation
//!   - FeatureFlags: Tier-based feature access control
//!   - Quota: Usage limits and quotas
//!   - UsageRecord: Transfer usage tracking
//!
//! - **Ports Layer**: Interfaces for hexagonal architecture
//!   - LicenseRepository: License storage interface
//!   - UsageRepository: Usage tracking interface
//!
//! - **Adapters Layer**: Infrastructure implementations
//!   - MemoryLicenseStore: In-memory license storage
//!   - SQLiteLicenseStore: Persistent license storage
//!
//! - **Application Layer**: Use cases and services
//!   - LicenseService: License management operations
//!   - UsageTracker: Usage tracking and quota enforcement
//!
//! # Example
//!
//! ```rust
//! use qltp_licensing::{License, LicenseTier, Feature};
//!
//! // Create a new license
//! let mut license = License::new(
//!     LicenseTier::Pro,
//!     Some("user@example.com".to_string())
//! );
//!
//! // Check if a feature is available
//! assert!(license.has_feature(Feature::Encryption).is_ok());
//! assert!(license.has_feature(Feature::ParallelTransfers).is_err());
//!
//! // Upgrade tier
//! license.upgrade_tier(LicenseTier::Team).unwrap();
//! assert!(license.has_feature(Feature::ParallelTransfers).is_ok());
//! ```

pub mod adapters;
pub mod application;
pub mod domain;
pub mod error;
pub mod integration;
pub mod middleware;
pub mod ports;

// Re-export main types for convenience
pub use adapters::{MemoryLicenseStore, MemoryUsageStore};

#[cfg(feature = "sqlite")]
pub use adapters::{SqliteLicenseStore, SqliteUsageStore};

pub use application::{LicenseService, UsageStats, UsageTracker};
pub use domain::{
    Device, DeviceFingerprint, DeviceId,
    Feature, FeatureFlags,
    License, LicenseId, LicenseKey, LicenseTier,
    Quota, TransferType, UsageRecord, UsageRecordId,
};
pub use error::{LicenseError, Result};
pub use integration::{
    AnonymousId, AnonymousUser, AuthLicenseManager, EnhancedSession,
    SessionInfo, UserAccount, UserRegistration,
};
pub use middleware::{TransferValidator, TransferValidationError, ValidationContext};
pub use ports::{LicenseRepository, UsageRepository};

// Made with Bob
