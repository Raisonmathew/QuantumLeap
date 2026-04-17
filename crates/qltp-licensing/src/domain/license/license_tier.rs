//! License tier value object

use serde::{Deserialize, Serialize};

/// License tiers with different feature sets and quotas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LicenseTier {
    /// Free tier - Basic features, limited quota
    Free,
    /// Pro tier - Individual users, enhanced features
    Pro,
    /// Team tier - Small teams, collaboration features
    Team,
    /// Business tier - Larger organizations, advanced features
    Business,
    /// Enterprise tier - Unlimited, custom features
    Enterprise,
}

impl LicenseTier {
    /// Get monthly data quota in bytes
    pub fn monthly_quota(&self) -> u64 {
        match self {
            LicenseTier::Free => 10 * 1024 * 1024 * 1024,        // 10 GB
            LicenseTier::Pro => 100 * 1024 * 1024 * 1024,        // 100 GB
            LicenseTier::Team => 500 * 1024 * 1024 * 1024,       // 500 GB
            LicenseTier::Business => 2 * 1024 * 1024 * 1024 * 1024, // 2 TB
            LicenseTier::Enterprise => u64::MAX,                  // Unlimited
        }
    }

    /// Get maximum file size in bytes
    pub fn max_file_size(&self) -> u64 {
        match self {
            LicenseTier::Free => 1024 * 1024 * 1024,             // 1 GB
            LicenseTier::Pro => 10 * 1024 * 1024 * 1024,         // 10 GB
            LicenseTier::Team => 50 * 1024 * 1024 * 1024,        // 50 GB
            LicenseTier::Business => 100 * 1024 * 1024 * 1024,   // 100 GB
            LicenseTier::Enterprise => u64::MAX,                  // Unlimited
        }
    }

    /// Get maximum number of devices
    pub fn max_devices(&self) -> usize {
        match self {
            LicenseTier::Free => 1,
            LicenseTier::Pro => 3,
            LicenseTier::Team => 10,
            LicenseTier::Business => 50,
            LicenseTier::Enterprise => usize::MAX,
        }
    }

    /// Get maximum concurrent transfers
    pub fn max_concurrent_transfers(&self) -> usize {
        match self {
            LicenseTier::Free => 1,
            LicenseTier::Pro => 3,
            LicenseTier::Team => 10,
            LicenseTier::Business => 25,
            LicenseTier::Enterprise => 100,
        }
    }

    /// Get tier name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            LicenseTier::Free => "Free",
            LicenseTier::Pro => "Pro",
            LicenseTier::Team => "Team",
            LicenseTier::Business => "Business",
            LicenseTier::Enterprise => "Enterprise",
        }
    }

    /// Get monthly price in USD cents
    pub fn monthly_price_cents(&self) -> u32 {
        match self {
            LicenseTier::Free => 0,
            LicenseTier::Pro => 999,        // $9.99
            LicenseTier::Team => 4999,      // $49.99
            LicenseTier::Business => 19999, // $199.99
            LicenseTier::Enterprise => 0,   // Custom pricing
        }
    }
}

impl std::fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for LicenseTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(LicenseTier::Free),
            "pro" => Ok(LicenseTier::Pro),
            "team" => Ok(LicenseTier::Team),
            "business" => Ok(LicenseTier::Business),
            "enterprise" => Ok(LicenseTier::Enterprise),
            _ => Err(format!("Invalid license tier: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_quotas() {
        assert_eq!(LicenseTier::Free.monthly_quota(), 10 * 1024 * 1024 * 1024);
        assert_eq!(LicenseTier::Pro.monthly_quota(), 100 * 1024 * 1024 * 1024);
        assert_eq!(LicenseTier::Enterprise.monthly_quota(), u64::MAX);
    }

    #[test]
    fn test_tier_devices() {
        assert_eq!(LicenseTier::Free.max_devices(), 1);
        assert_eq!(LicenseTier::Pro.max_devices(), 3);
        assert_eq!(LicenseTier::Team.max_devices(), 10);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(LicenseTier::Free.to_string(), "Free");
        assert_eq!(LicenseTier::Pro.to_string(), "Pro");
    }

    #[test]
    fn test_tier_from_str() {
        assert_eq!("free".parse::<LicenseTier>().unwrap(), LicenseTier::Free);
        assert_eq!("PRO".parse::<LicenseTier>().unwrap(), LicenseTier::Pro);
        assert!("invalid".parse::<LicenseTier>().is_err());
    }

    #[test]
    fn test_tier_pricing() {
        assert_eq!(LicenseTier::Free.monthly_price_cents(), 0);
        assert_eq!(LicenseTier::Pro.monthly_price_cents(), 999);
        assert_eq!(LicenseTier::Team.monthly_price_cents(), 4999);
    }
}

// Made with Bob
