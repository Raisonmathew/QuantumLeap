//! Compression utilities for QLTP
//!
//! Provides LZ4 and Zstd compression/decompression functions.

use crate::error::{Error, Result};

/// Compress data using LZ4
pub fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    Ok(lz4_flex::compress_prepend_size(data))
}

/// Decompress LZ4 data
pub fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data)
        .map_err(|e| Error::Compression(format!("LZ4 decompression failed: {}", e)))
}

/// Compress data using Zstd with specified compression level
pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::encode_all(data, level)
        .map_err(|e| Error::Compression(format!("Zstd compression failed: {}", e)))
}

/// Decompress Zstd data
pub fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decode_all(data)
        .map_err(|e| Error::Compression(format!("Zstd decompression failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_compress_decompress() {
        let data = b"Hello, World! This is test data for compression.".repeat(10);
        
        let compressed = compress_lz4(&data).expect("Compression failed");
        assert!(compressed.len() < data.len());
        
        let decompressed = decompress_lz4(&compressed).expect("Decompression failed");
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zstd_compress_decompress() {
        let data = b"Hello, World! This is test data for compression.".repeat(10);
        
        let compressed = compress_zstd(&data, 3).expect("Compression failed");
        assert!(compressed.len() < data.len());
        
        let decompressed = decompress_zstd(&compressed).expect("Decompression failed");
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zstd_levels() {
        let data = b"Test data ".repeat(100);
        
        let compressed_low = compress_zstd(&data, 1).expect("Compression failed");
        let compressed_high = compress_zstd(&data, 9).expect("Compression failed");
        
        // Higher compression level should produce smaller output
        assert!(compressed_high.len() <= compressed_low.len());
    }
}

// Made with Bob
