//! Ports layer
//!
//! Defines interfaces (ports) for external dependencies following
//! the Hexagonal Architecture pattern.

pub mod license_repository;
pub mod usage_repository;

pub use license_repository::LicenseRepository;
pub use usage_repository::UsageRepository;

// Made with Bob
