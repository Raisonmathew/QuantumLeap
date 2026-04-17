//! Usage record entity for tracking data transfers

use crate::domain::license::LicenseId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Usage record entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Unique record identifier
    id: UsageRecordId,
    /// License ID
    license_id: LicenseId,
    /// Bytes transferred
    bytes: u64,
    /// Transfer type
    transfer_type: TransferType,
    /// Source path (optional)
    source: Option<String>,
    /// Destination path (optional)
    destination: Option<String>,
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Duration in seconds
    duration_secs: u64,
}

impl UsageRecord {
    /// Create a new usage record
    pub fn new(
        license_id: LicenseId,
        bytes: u64,
        transfer_type: TransferType,
    ) -> Self {
        Self {
            id: UsageRecordId::new(),
            license_id,
            bytes,
            transfer_type,
            source: None,
            destination: None,
            timestamp: Utc::now(),
            duration_secs: 0,
        }
    }

    /// Create with full details
    pub fn with_details(
        license_id: LicenseId,
        bytes: u64,
        transfer_type: TransferType,
        source: Option<String>,
        destination: Option<String>,
        duration_secs: u64,
    ) -> Self {
        Self {
            id: UsageRecordId::new(),
            license_id,
            bytes,
            transfer_type,
            source,
            destination,
            timestamp: Utc::now(),
            duration_secs,
        }
    }

    /// Get record ID
    pub fn id(&self) -> &UsageRecordId {
        &self.id
    }

    /// Get license ID
    pub fn license_id(&self) -> &LicenseId {
        &self.license_id
    }

    /// Get bytes transferred
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Get transfer type
    pub fn transfer_type(&self) -> TransferType {
        self.transfer_type
    }

    /// Get source path
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Get destination path
    pub fn destination(&self) -> Option<&str> {
        self.destination.as_deref()
    }

    /// Get timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Get duration in seconds
    pub fn duration_secs(&self) -> u64 {
        self.duration_secs
    }

    /// Calculate transfer speed in bytes per second
    pub fn speed_bps(&self) -> u64 {
        if self.duration_secs > 0 {
            self.bytes / self.duration_secs
        } else {
            0
        }
    }

    /// Get human-readable size
    pub fn human_readable_size(&self) -> String {
        human_bytes(self.bytes)
    }
}

/// Usage record identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UsageRecordId(Uuid);

impl UsageRecordId {
    /// Create a new usage record ID
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

impl Default for UsageRecordId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for UsageRecordId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Transfer type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransferType {
    /// File upload
    Upload,
    /// File download
    Download,
    /// Peer-to-peer transfer
    P2P,
}

impl TransferType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferType::Upload => "upload",
            TransferType::Download => "download",
            TransferType::P2P => "p2p",
        }
    }
}

impl std::fmt::Display for TransferType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Convert bytes to human-readable format
fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f = bytes as f64;
    let exp = (bytes_f.log10() / 3.0).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);
    
    let value = bytes_f / 1000_f64.powi(exp as i32);
    
    if exp == 0 {
        format!("{} {}", bytes, UNITS[exp])
    } else {
        format!("{:.2} {}", value, UNITS[exp])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_record_creation() {
        let license_id = LicenseId::new();
        let record = UsageRecord::new(
            license_id.clone(),
            1024 * 1024,
            TransferType::Upload,
        );

        assert_eq!(record.license_id(), &license_id);
        assert_eq!(record.bytes(), 1024 * 1024);
        assert_eq!(record.transfer_type(), TransferType::Upload);
    }

    #[test]
    fn test_usage_record_with_details() {
        let license_id = LicenseId::new();
        let record = UsageRecord::with_details(
            license_id.clone(),
            1024 * 1024,
            TransferType::Download,
            Some("/path/to/source".to_string()),
            Some("/path/to/dest".to_string()),
            10,
        );

        assert_eq!(record.source(), Some("/path/to/source"));
        assert_eq!(record.destination(), Some("/path/to/dest"));
        assert_eq!(record.duration_secs(), 10);
    }

    #[test]
    fn test_transfer_speed() {
        let license_id = LicenseId::new();
        let record = UsageRecord::with_details(
            license_id,
            1000,
            TransferType::P2P,
            None,
            None,
            10,
        );

        assert_eq!(record.speed_bps(), 100);
    }

    #[test]
    fn test_human_readable_size() {
        let license_id = LicenseId::new();
        
        let record1 = UsageRecord::new(license_id.clone(), 500, TransferType::Upload);
        assert_eq!(record1.human_readable_size(), "500 B");

        let record2 = UsageRecord::new(license_id.clone(), 1500, TransferType::Upload);
        assert_eq!(record2.human_readable_size(), "1.50 KB");

        let record3 = UsageRecord::new(license_id.clone(), 1_500_000, TransferType::Upload);
        assert_eq!(record3.human_readable_size(), "1.50 MB");

        let record4 = UsageRecord::new(license_id, 1_500_000_000, TransferType::Upload);
        assert_eq!(record4.human_readable_size(), "1.50 GB");
    }

    #[test]
    fn test_transfer_type_display() {
        assert_eq!(TransferType::Upload.to_string(), "upload");
        assert_eq!(TransferType::Download.to_string(), "download");
        assert_eq!(TransferType::P2P.to_string(), "p2p");
    }

    #[test]
    fn test_usage_record_id_uniqueness() {
        let id1 = UsageRecordId::new();
        let id2 = UsageRecordId::new();
        assert_ne!(id1, id2);
    }
}

// Made with Bob
