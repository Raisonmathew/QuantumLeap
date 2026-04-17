//! Feature flags value object

use super::license_tier::LicenseTier;
use serde::{Deserialize, Serialize};

/// Feature flags that control access to QLTP features
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable compression (LZ4, Zstd)
    pub compression: bool,
    /// Enable adaptive compression
    pub adaptive_compression: bool,
    /// Enable deduplication
    pub deduplication: bool,
    /// Enable encryption (TLS)
    pub encryption: bool,
    /// Enable resume capability
    pub resume: bool,
    /// Enable parallel transfers
    pub parallel_transfers: bool,
    /// Enable QUIC protocol
    pub quic: bool,
    /// Enable predictive pre-fetching
    pub prefetch: bool,
    /// Enable priority transfers
    pub priority: bool,
    /// Enable team collaboration features
    pub collaboration: bool,
    /// Enable API access
    pub api_access: bool,
    /// Enable custom branding
    pub custom_branding: bool,
    /// Enable audit logs
    pub audit_logs: bool,
    /// Enable SSO (Single Sign-On)
    pub sso: bool,
}

impl FeatureFlags {
    /// Create feature flags for a specific tier
    pub fn for_tier(tier: LicenseTier) -> Self {
        match tier {
            LicenseTier::Free => Self {
                compression: true,
                adaptive_compression: false,
                deduplication: false,
                encryption: false,
                resume: false,
                parallel_transfers: false,
                quic: false,
                prefetch: false,
                priority: false,
                collaboration: false,
                api_access: false,
                custom_branding: false,
                audit_logs: false,
                sso: false,
            },
            LicenseTier::Pro => Self {
                compression: true,
                adaptive_compression: true,
                deduplication: true,
                encryption: true,
                resume: true,
                parallel_transfers: false,
                quic: false,
                prefetch: false,
                priority: false,
                collaboration: false,
                api_access: false,
                custom_branding: false,
                audit_logs: false,
                sso: false,
            },
            LicenseTier::Team => Self {
                compression: true,
                adaptive_compression: true,
                deduplication: true,
                encryption: true,
                resume: true,
                parallel_transfers: true,
                quic: true,
                prefetch: true,
                priority: false,
                collaboration: true,
                api_access: false,
                custom_branding: false,
                audit_logs: false,
                sso: false,
            },
            LicenseTier::Business => Self {
                compression: true,
                adaptive_compression: true,
                deduplication: true,
                encryption: true,
                resume: true,
                parallel_transfers: true,
                quic: true,
                prefetch: true,
                priority: true,
                collaboration: true,
                api_access: true,
                custom_branding: false,
                audit_logs: true,
                sso: false,
            },
            LicenseTier::Enterprise => Self {
                compression: true,
                adaptive_compression: true,
                deduplication: true,
                encryption: true,
                resume: true,
                parallel_transfers: true,
                quic: true,
                prefetch: true,
                priority: true,
                collaboration: true,
                api_access: true,
                custom_branding: true,
                audit_logs: true,
                sso: true,
            },
        }
    }

    /// Check if a specific feature is enabled
    pub fn has_feature(&self, feature: Feature) -> bool {
        match feature {
            Feature::Compression => self.compression,
            Feature::AdaptiveCompression => self.adaptive_compression,
            Feature::Deduplication => self.deduplication,
            Feature::Encryption => self.encryption,
            Feature::Resume => self.resume,
            Feature::ParallelTransfers => self.parallel_transfers,
            Feature::Quic => self.quic,
            Feature::Prefetch => self.prefetch,
            Feature::Priority => self.priority,
            Feature::Collaboration => self.collaboration,
            Feature::ApiAccess => self.api_access,
            Feature::CustomBranding => self.custom_branding,
            Feature::AuditLogs => self.audit_logs,
            Feature::Sso => self.sso,
        }
    }

    /// Get list of enabled features
    pub fn enabled_features(&self) -> Vec<Feature> {
        let mut features = Vec::new();
        if self.compression { features.push(Feature::Compression); }
        if self.adaptive_compression { features.push(Feature::AdaptiveCompression); }
        if self.deduplication { features.push(Feature::Deduplication); }
        if self.encryption { features.push(Feature::Encryption); }
        if self.resume { features.push(Feature::Resume); }
        if self.parallel_transfers { features.push(Feature::ParallelTransfers); }
        if self.quic { features.push(Feature::Quic); }
        if self.prefetch { features.push(Feature::Prefetch); }
        if self.priority { features.push(Feature::Priority); }
        if self.collaboration { features.push(Feature::Collaboration); }
        if self.api_access { features.push(Feature::ApiAccess); }
        if self.custom_branding { features.push(Feature::CustomBranding); }
        if self.audit_logs { features.push(Feature::AuditLogs); }
        if self.sso { features.push(Feature::Sso); }
        features
    }
}

/// Individual features that can be enabled/disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    Compression,
    AdaptiveCompression,
    Deduplication,
    Encryption,
    Resume,
    ParallelTransfers,
    Quic,
    Prefetch,
    Priority,
    Collaboration,
    ApiAccess,
    CustomBranding,
    AuditLogs,
    Sso,
}

impl Feature {
    pub fn as_str(&self) -> &'static str {
        match self {
            Feature::Compression => "compression",
            Feature::AdaptiveCompression => "adaptive_compression",
            Feature::Deduplication => "deduplication",
            Feature::Encryption => "encryption",
            Feature::Resume => "resume",
            Feature::ParallelTransfers => "parallel_transfers",
            Feature::Quic => "quic",
            Feature::Prefetch => "prefetch",
            Feature::Priority => "priority",
            Feature::Collaboration => "collaboration",
            Feature::ApiAccess => "api_access",
            Feature::CustomBranding => "custom_branding",
            Feature::AuditLogs => "audit_logs",
            Feature::Sso => "sso",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_tier_features() {
        let flags = FeatureFlags::for_tier(LicenseTier::Free);
        assert!(flags.compression);
        assert!(!flags.adaptive_compression);
        assert!(!flags.encryption);
        assert!(!flags.parallel_transfers);
    }

    #[test]
    fn test_pro_tier_features() {
        let flags = FeatureFlags::for_tier(LicenseTier::Pro);
        assert!(flags.compression);
        assert!(flags.adaptive_compression);
        assert!(flags.encryption);
        assert!(flags.resume);
        assert!(!flags.parallel_transfers);
    }

    #[test]
    fn test_enterprise_tier_features() {
        let flags = FeatureFlags::for_tier(LicenseTier::Enterprise);
        assert!(flags.compression);
        assert!(flags.encryption);
        assert!(flags.parallel_transfers);
        assert!(flags.sso);
        assert!(flags.custom_branding);
    }

    #[test]
    fn test_has_feature() {
        let flags = FeatureFlags::for_tier(LicenseTier::Team);
        assert!(flags.has_feature(Feature::Compression));
        assert!(flags.has_feature(Feature::ParallelTransfers));
        assert!(!flags.has_feature(Feature::Sso));
    }

    #[test]
    fn test_enabled_features() {
        let flags = FeatureFlags::for_tier(LicenseTier::Pro);
        let enabled = flags.enabled_features();
        assert!(enabled.contains(&Feature::Compression));
        assert!(enabled.contains(&Feature::Encryption));
        assert!(!enabled.contains(&Feature::ParallelTransfers));
    }
}

// Made with Bob
