//! Middleware for license validation and enforcement

pub mod transfer_validator;

pub use transfer_validator::{TransferValidator, TransferValidationError, ValidationContext};

// Made with Bob
