//! Device entity for license activation tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Device entity representing an activated device
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// Unique device identifier
    id: DeviceId,
    /// Device name (user-friendly)
    name: String,
    /// Device fingerprint (hardware-based)
    fingerprint: String,
    /// Operating system
    os: String,
    /// Hostname
    hostname: String,
    /// Activation timestamp
    activated_at: DateTime<Utc>,
    /// Last seen timestamp
    last_seen_at: DateTime<Utc>,
    /// Whether device is active
    is_active: bool,
}

impl Device {
    /// Create a new device
    pub fn new(
        name: String,
        fingerprint: String,
        os: String,
        hostname: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: DeviceId::new(),
            name,
            fingerprint,
            os,
            hostname,
            activated_at: now,
            last_seen_at: now,
            is_active: true,
        }
    }

    /// Get device ID
    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get device fingerprint
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Get operating system
    pub fn os(&self) -> &str {
        &self.os
    }

    /// Get hostname
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    /// Get activation timestamp
    pub fn activated_at(&self) -> DateTime<Utc> {
        self.activated_at
    }

    /// Get last seen timestamp
    pub fn last_seen_at(&self) -> DateTime<Utc> {
        self.last_seen_at
    }

    /// Check if device is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen_at = Utc::now();
    }

    /// Deactivate device
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Reactivate device
    pub fn reactivate(&mut self) {
        self.is_active = true;
        self.last_seen_at = Utc::now();
    }

    /// Update device name
    pub fn update_name(&mut self, name: String) {
        self.name = name;
    }

    /// Check if device matches fingerprint
    pub fn matches_fingerprint(&self, fingerprint: &str) -> bool {
        self.fingerprint == fingerprint
    }

    /// Get days since activation
    pub fn days_since_activation(&self) -> i64 {
        let now = Utc::now();
        (now - self.activated_at).num_days()
    }

    /// Get days since last seen
    pub fn days_since_last_seen(&self) -> i64 {
        let now = Utc::now();
        (now - self.last_seen_at).num_days()
    }
}

/// Device identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(Uuid);

impl DeviceId {
    /// Create a new device ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get as UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get as string
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for DeviceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Device fingerprint generator
pub struct DeviceFingerprint;

impl DeviceFingerprint {
    /// Generate a device fingerprint based on system information
    pub fn generate() -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        // Add hostname
        if let Ok(hostname) = hostname::get() {
            hasher.update(hostname.to_string_lossy().as_bytes());
        }

        // Add OS
        hasher.update(std::env::consts::OS.as_bytes());
        hasher.update(std::env::consts::ARCH.as_bytes());

        // Add username
        if let Ok(username) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
            hasher.update(username.as_bytes());
        }

        let hash = hasher.finalize();
        hex::encode(&hash[0..16]) // Use first 16 bytes
    }

    /// Get current OS name
    pub fn current_os() -> String {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Get current hostname
    pub fn current_hostname() -> String {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_creation() {
        let device = Device::new(
            "My Laptop".to_string(),
            "abc123".to_string(),
            "Linux".to_string(),
            "laptop-01".to_string(),
        );

        assert_eq!(device.name(), "My Laptop");
        assert_eq!(device.fingerprint(), "abc123");
        assert_eq!(device.os(), "Linux");
        assert_eq!(device.hostname(), "laptop-01");
        assert!(device.is_active());
    }

    #[test]
    fn test_device_deactivation() {
        let mut device = Device::new(
            "Test Device".to_string(),
            "xyz789".to_string(),
            "macOS".to_string(),
            "mac-01".to_string(),
        );

        assert!(device.is_active());
        device.deactivate();
        assert!(!device.is_active());
    }

    #[test]
    fn test_device_reactivation() {
        let mut device = Device::new(
            "Test Device".to_string(),
            "xyz789".to_string(),
            "Windows".to_string(),
            "win-01".to_string(),
        );

        device.deactivate();
        assert!(!device.is_active());
        
        device.reactivate();
        assert!(device.is_active());
    }

    #[test]
    fn test_device_update_last_seen() {
        let mut device = Device::new(
            "Test Device".to_string(),
            "xyz789".to_string(),
            "Linux".to_string(),
            "linux-01".to_string(),
        );

        let initial_last_seen = device.last_seen_at();
        std::thread::sleep(std::time::Duration::from_millis(10));
        device.update_last_seen();
        
        assert!(device.last_seen_at() > initial_last_seen);
    }

    #[test]
    fn test_device_fingerprint_matching() {
        let device = Device::new(
            "Test Device".to_string(),
            "fingerprint123".to_string(),
            "Linux".to_string(),
            "test-host".to_string(),
        );

        assert!(device.matches_fingerprint("fingerprint123"));
        assert!(!device.matches_fingerprint("different"));
    }

    #[test]
    fn test_device_id_creation() {
        let id1 = DeviceId::new();
        let id2 = DeviceId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_device_id_display() {
        let id = DeviceId::new();
        let id_str = id.to_string();
        assert!(!id_str.is_empty());
    }

    #[test]
    fn test_device_fingerprint_generation() {
        let fp1 = DeviceFingerprint::generate();
        let fp2 = DeviceFingerprint::generate();
        
        // Should be consistent on same machine
        assert_eq!(fp1, fp2);
        assert!(!fp1.is_empty());
    }

    #[test]
    fn test_device_update_name() {
        let mut device = Device::new(
            "Old Name".to_string(),
            "fp123".to_string(),
            "Linux".to_string(),
            "host".to_string(),
        );

        device.update_name("New Name".to_string());
        assert_eq!(device.name(), "New Name");
    }
}

// Made with Bob
