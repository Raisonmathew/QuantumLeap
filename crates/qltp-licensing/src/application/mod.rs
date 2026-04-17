//! Application layer
//!
//! Contains use cases and services that orchestrate domain logic
//! and coordinate with infrastructure through ports.

pub mod license_service;
pub mod usage_tracker;

pub use license_service::LicenseService;
pub use usage_tracker::{UsageStats, UsageTracker};

// Made with Bob
