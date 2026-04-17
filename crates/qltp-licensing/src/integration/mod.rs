//! Authentication-Licensing Integration
//!
//! This module provides unified management of authentication and licensing,
//! combining both systems for seamless user experience.

pub mod manager;
pub mod session;
pub mod user;
pub mod anonymous;

pub use manager::AuthLicenseManager;
pub use session::{EnhancedSession, SessionInfo};
pub use user::{UserAccount, UserRegistration};
pub use anonymous::{AnonymousUser, AnonymousId};

// Made with Bob
