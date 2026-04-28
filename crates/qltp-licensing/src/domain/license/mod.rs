//! License domain module

pub mod device;
pub mod feature_flags;
pub mod license;
pub mod license_key;
pub mod license_tier;
pub mod signing;

pub use device::{Device, DeviceFingerprint, DeviceId};
pub use feature_flags::{Feature, FeatureFlags};
pub use license::{License, LicenseId};
pub use license_key::LicenseKey;
pub use license_tier::LicenseTier;
pub use signing::{LicenseSigner, LicenseVerifier, PUBLIC_KEY_BYTES, SIGNATURE_BYTES};

// Made with Bob
