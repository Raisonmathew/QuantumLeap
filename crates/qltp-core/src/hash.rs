//! Hashing utilities for content addressing

use sha2::{Digest, Sha256};

/// Compute SHA-256 hash of data
pub fn compute_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Compute BLAKE3 hash of data (faster alternative)
pub fn compute_hash_blake3(data: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(data);
    *hash.as_bytes()
}

/// Verify that data matches expected hash
pub fn verify_hash(data: &[u8], expected: &[u8; 32]) -> bool {
    let actual = compute_hash(data);
    &actual == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let data = b"Hello, World!";
        let hash = compute_hash(data);
        
        // SHA-256 of "Hello, World!" is known
        assert_eq!(hash.len(), 32);
        
        // Same data should produce same hash
        let hash2 = compute_hash(data);
        assert_eq!(hash, hash2);
        
        // Different data should produce different hash
        let hash3 = compute_hash(b"Different data");
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_compute_hash_blake3() {
        let data = b"Hello, World!";
        let hash = compute_hash_blake3(data);
        
        assert_eq!(hash.len(), 32);
        
        // Same data should produce same hash
        let hash2 = compute_hash_blake3(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_verify_hash() {
        let data = b"Hello, World!";
        let hash = compute_hash(data);
        
        assert!(verify_hash(data, &hash));
        assert!(!verify_hash(b"Different data", &hash));
    }

    #[test]
    fn test_hash_consistency() {
        // Verify that SHA-256 produces expected output
        let data = b"test";
        let hash = compute_hash(data);
        let hex = hex::encode(hash);
        
        // SHA-256 of "test" should be consistent
        assert_eq!(
            hex,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
    }
}

// Made with Bob
