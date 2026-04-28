//! License key value object with generation and validation

use super::license_tier::LicenseTier;
use crate::error::{LicenseError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// License key format: QLTP-{TIER}-{SEGMENT1}-{SEGMENT2}-{CHECKSUM}
/// Example: QLTP-PRO-A1B2C3D4-E5F6G7H8-9I0J
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LicenseKey(String);

impl LicenseKey {
    const PREFIX: &'static str = "QLTP";
    const SEGMENT_LENGTH: usize = 8;
    /// Number of characters in the trailing checksum.
    ///
    /// SECURITY (C6): the original 4-character checksum gave only ~20
    /// bits of forgery resistance against a brute-force key generator —
    /// any random key would collide with a valid format with probability
    /// roughly 1 in 36^4 ≈ 1.7M. At 16 characters the search space is
    /// 36^16 ≈ 8e24, which is computationally infeasible to brute-force
    /// even at line-rate against an offline checker.
    const CHECKSUM_LENGTH: usize = 16;

    /// Generate a new license key for a tier
    pub fn generate(tier: LicenseTier) -> Self {
        
        let mut rng = rand::thread_rng();

        // Generate two random segments
        let segment1 = Self::generate_segment(&mut rng);
        let segment2 = Self::generate_segment(&mut rng);

        // Calculate checksum
        let checksum = Self::calculate_checksum(tier, &segment1, &segment2);

        let key = format!(
            "{}-{}-{}-{}-{}",
            Self::PREFIX,
            tier.as_str().to_uppercase(),
            segment1,
            segment2,
            checksum
        );

        Self(key)
    }

    /// Parse and validate a license key
    pub fn from_string(key: String) -> Result<Self> {
        let key = key.trim().to_uppercase();
        
        // Check format: QLTP-TIER-SEGMENT1-SEGMENT2-CHECKSUM
        let parts: Vec<&str> = key.split('-').collect();
        if parts.len() != 5 {
            return Err(LicenseError::InvalidLicenseKey);
        }

        // Validate prefix
        if parts[0] != Self::PREFIX {
            return Err(LicenseError::InvalidLicenseKey);
        }

        // Validate tier
        let tier: LicenseTier = parts[1]
            .parse()
            .map_err(|_| LicenseError::InvalidLicenseKey)?;

        // Validate segments
        let segment1 = parts[2];
        let segment2 = parts[3];
        if segment1.len() != Self::SEGMENT_LENGTH || segment2.len() != Self::SEGMENT_LENGTH {
            return Err(LicenseError::InvalidLicenseKey);
        }

        // Validate checksum length up-front so a malformed key never gets
        // anywhere near the constant-time comparator below.
        let provided_checksum = parts[4];
        if provided_checksum.len() != Self::CHECKSUM_LENGTH {
            return Err(LicenseError::InvalidLicenseKey);
        }
        let calculated_checksum = Self::calculate_checksum(tier, segment1, segment2);

        // Constant-time compare: avoids leaking via timing how many
        // leading characters of a forged checksum happen to be correct.
        use subtle::ConstantTimeEq;
        if provided_checksum
            .as_bytes()
            .ct_eq(calculated_checksum.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(LicenseError::InvalidLicenseKey);
        }

        Ok(Self(key))
    }

    /// Get the license tier from the key
    pub fn tier(&self) -> Result<LicenseTier> {
        let parts: Vec<&str> = self.0.split('-').collect();
        if parts.len() != 5 {
            return Err(LicenseError::InvalidLicenseKey);
        }
        parts[1]
            .parse()
            .map_err(|_| LicenseError::InvalidLicenseKey)
    }

    /// Get the key as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generate a random alphanumeric segment
    fn generate_segment<R: rand::Rng>(rng: &mut R) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        (0..Self::SEGMENT_LENGTH)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Calculate checksum for validation
    fn calculate_checksum(tier: LicenseTier, segment1: &str, segment2: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(Self::PREFIX.as_bytes());
        hasher.update(tier.as_str().as_bytes());
        hasher.update(segment1.as_bytes());
        hasher.update(segment2.as_bytes());
        let hash = hasher.finalize();

        // Take the first CHECKSUM_LENGTH bytes of the SHA-256 digest and
        // map each one through the alphanumeric charset. Widening from 4
        // to 16 bytes raises forgery cost from ~2^20 to ~2^80 — well
        // beyond brute-force territory for an offline attacker.
        let bytes = &hash[0..Self::CHECKSUM_LENGTH];
        Self::encode_checksum(bytes)
    }

    /// Encode checksum bytes to alphanumeric string
    fn encode_checksum(bytes: &[u8]) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        bytes
            .iter()
            .map(|&b| CHARSET[(b % CHARSET.len() as u8) as usize] as char)
            .collect()
    }
}

impl std::fmt::Display for LicenseKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LicenseKey {
    type Err = LicenseError;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_string(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_license_key() {
        let key = LicenseKey::generate(LicenseTier::Pro);
        assert!(key.as_str().starts_with("QLTP-PRO-"));
        assert_eq!(key.as_str().split('-').count(), 5);
    }

    #[test]
    fn test_license_key_format() {
        let key = LicenseKey::generate(LicenseTier::Team);
        let parts: Vec<&str> = key.as_str().split('-').collect();
        
        assert_eq!(parts[0], "QLTP");
        assert_eq!(parts[1], "TEAM");
        assert_eq!(parts[2].len(), 8);
        assert_eq!(parts[3].len(), 8);
        assert_eq!(parts[4].len(), 16);
    }

    #[test]
    fn test_license_key_validation() {
        let key = LicenseKey::generate(LicenseTier::Business);
        let key_str = key.as_str().to_string();
        
        // Valid key should parse successfully
        let parsed = LicenseKey::from_string(key_str.clone()).unwrap();
        assert_eq!(parsed.as_str(), key.as_str());
    }

    #[test]
    fn test_invalid_license_key() {
        // Invalid prefix
        assert!(LicenseKey::from_string("INVALID-PRO-A1B2C3D4-E5F6G7H8-9I0J9I0J9I0J9I0J".to_string()).is_err());
        
        // Invalid tier
        assert!(LicenseKey::from_string("QLTP-INVALID-A1B2C3D4-E5F6G7H8-9I0J9I0J9I0J9I0J".to_string()).is_err());
        
        // Wrong number of parts
        assert!(LicenseKey::from_string("QLTP-PRO-A1B2C3D4".to_string()).is_err());
        
        // Invalid checksum (right length, wrong content)
        assert!(LicenseKey::from_string("QLTP-PRO-A1B2C3D4-E5F6G7H8-XXXXXXXXXXXXXXXX".to_string()).is_err());

        // Old short checksum (4 chars) must be rejected — keys minted by
        // the legacy generator are no longer valid for the strengthened
        // format.
        assert!(LicenseKey::from_string("QLTP-PRO-A1B2C3D4-E5F6G7H8-9I0J".to_string()).is_err());
    }

    #[test]
    fn test_get_tier_from_key() {
        let key = LicenseKey::generate(LicenseTier::Enterprise);
        assert_eq!(key.tier().unwrap(), LicenseTier::Enterprise);
    }

    #[test]
    fn test_license_key_uniqueness() {
        let key1 = LicenseKey::generate(LicenseTier::Pro);
        let key2 = LicenseKey::generate(LicenseTier::Pro);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_license_key_case_insensitive() {
        let key = LicenseKey::generate(LicenseTier::Pro);
        let lowercase = key.as_str().to_lowercase();
        let parsed = LicenseKey::from_string(lowercase).unwrap();
        assert_eq!(parsed.as_str(), key.as_str());
    }
}

// Made with Bob
