//! Error types for licensing

use thiserror::Error;

/// Licensing errors
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum LicenseError {
    /// Invalid license key format or checksum
    #[error("Invalid license key")]
    InvalidLicenseKey,

    /// License has expired
    #[error("License expired")]
    LicenseExpired,

    /// License not found
    #[error("License not found")]
    LicenseNotFound,

    /// License already exists
    #[error("License already exists")]
    LicenseAlreadyExists,

    /// Feature not available in current tier
    #[error("Feature not available in {tier} tier")]
    FeatureNotAvailable { tier: String },

    /// Quota exceeded
    #[error("Quota exceeded: {message}")]
    QuotaExceeded { message: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },

    /// Device limit exceeded
    #[error("Device limit exceeded: maximum {max} devices allowed")]
    DeviceLimitExceeded { max: usize },

    /// License already activated on another device
    #[error("License already activated on device: {device_id}")]
    AlreadyActivated { device_id: String },

    /// Authentication required
    #[error("Authentication required")]
    AuthenticationRequired,

    /// Invalid credentials
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// No active session
    #[error("No active session")]
    NoActiveSession,

    /// Invalid input
    #[error("Invalid input for {field}: {message}")]
    InvalidInput { field: String, message: String },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// License signature is missing where one is required by policy
    #[error("License signature is required by policy but is absent")]
    SignatureRequired,

    /// License signature is present but did not verify against the trusted key
    #[error("License signature verification failed")]
    InvalidSignature,
}

/// Result type for licensing operations
pub type Result<T> = std::result::Result<T, LicenseError>;

impl From<serde_json::Error> for LicenseError {
    fn from(err: serde_json::Error) -> Self {
        LicenseError::Serialization(err.to_string())
    }
}

#[cfg(feature = "sqlite")]
impl From<rusqlite::Error> for LicenseError {
    fn from(err: rusqlite::Error) -> Self {
        LicenseError::Storage(err.to_string())
    }
}

impl From<qltp_auth::AuthError> for LicenseError {
    fn from(err: qltp_auth::AuthError) -> Self {
        match err {
            qltp_auth::AuthError::InvalidCredentials => LicenseError::InvalidCredentials,
            qltp_auth::AuthError::TokenExpired => LicenseError::AuthenticationRequired,
            _ => LicenseError::Internal(err.to_string()),
        }
    }
}

// Made with Bob
