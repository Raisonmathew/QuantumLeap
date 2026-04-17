//! Compression layer for QLTP
//!
//! Provides high-speed compression using LZ4 and Zstandard algorithms.

use std::io::{Read, Write};
use tracing::{debug, instrument};

/// Error type for compression operations
pub type Error = anyhow::Error;

/// Result type for compression operations
pub type Result<T> = std::result::Result<T, Error>;

/// Create a compression error
fn compression_error(msg: String) -> Error {
    anyhow::anyhow!("{}", msg)
}

/// Create a decompression error
fn decompression_error(msg: String) -> Error {
    anyhow::anyhow!("{}", msg)
}

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// LZ4 - Fast compression (500+ MB/s)
    Lz4,
    /// Zstandard - Balanced compression/ratio
    Zstd,
    /// No compression
    None,
}

impl Algorithm {
    /// Get the name of the algorithm
    pub fn name(&self) -> &'static str {
        match self {
            Algorithm::Lz4 => "LZ4",
            Algorithm::Zstd => "Zstandard",
            Algorithm::None => "None",
        }
    }
}

/// Compression level (1-22 for Zstd, ignored for LZ4)
#[derive(Debug, Clone, Copy)]
pub struct CompressionLevel(i32);

impl CompressionLevel {
    /// Fast compression (level 1)
    pub const FAST: Self = Self(1);
    /// Default compression (level 3)
    pub const DEFAULT: Self = Self(3);
    /// Best compression (level 22)
    pub const BEST: Self = Self(22);

    /// Create a new compression level
    pub fn new(level: i32) -> Result<Self> {
        if !(1..=22).contains(&level) {
            return Err(compression_error(format!(
                "Invalid compression level: {} (must be 1-22)",
                level
            )));
        }
        Ok(Self(level))
    }

    /// Get the level value
    pub fn value(&self) -> i32 {
        self.0
    }
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Compress data using the specified algorithm
#[instrument(skip(data))]
pub fn compress(data: &[u8], algorithm: Algorithm, level: CompressionLevel) -> Result<Vec<u8>> {
    debug!(
        "Compressing {} bytes with {} (level {})",
        data.len(),
        algorithm.name(),
        level.value()
    );

    let compressed = match algorithm {
        Algorithm::Lz4 => compress_lz4(data)?,
        Algorithm::Zstd => compress_zstd(data, level)?,
        Algorithm::None => data.to_vec(),
    };

    let ratio = data.len() as f64 / compressed.len() as f64;
    debug!(
        "Compressed {} bytes -> {} bytes (ratio: {:.2}x)",
        data.len(),
        compressed.len(),
        ratio
    );

    Ok(compressed)
}

/// Decompress data using the specified algorithm
#[instrument(skip(data))]
pub fn decompress(data: &[u8], algorithm: Algorithm) -> Result<Vec<u8>> {
    debug!("Decompressing {} bytes with {}", data.len(), algorithm.name());

    let decompressed = match algorithm {
        Algorithm::Lz4 => decompress_lz4(data)?,
        Algorithm::Zstd => decompress_zstd(data)?,
        Algorithm::None => data.to_vec(),
    };

    debug!("Decompressed {} bytes -> {} bytes", data.len(), decompressed.len());

    Ok(decompressed)
}

/// Compress data using LZ4
fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = lz4::EncoderBuilder::new()
        .level(4) // Fast compression
        .build(Vec::new())
        .map_err(|e| compression_error(format!("LZ4 encoder error: {}", e)))?;

    encoder
        .write_all(data)
        .map_err(|e| compression_error(format!("LZ4 write error: {}", e)))?;

    let (compressed, result): (Vec<u8>, std::io::Result<()>) = encoder.finish();
    result.map_err(|e| compression_error(format!("LZ4 finish error: {}", e)))?;

    Ok(compressed)
}

/// Decompress data using LZ4
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = lz4::Decoder::new(data)
        .map_err(|e| decompression_error(format!("LZ4 decoder error: {}", e)))?;

    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| decompression_error(format!("LZ4 read error: {}", e)))?;

    Ok(decompressed)
}

/// Compress data using Zstandard
fn compress_zstd(data: &[u8], level: CompressionLevel) -> Result<Vec<u8>> {
    zstd::encode_all(data, level.value())
        .map_err(|e| compression_error(format!("Zstd compression error: {}", e)))
}

/// Decompress data using Zstandard
fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decode_all(data)
        .map_err(|e| decompression_error(format!("Zstd decompression error: {}", e)))
}

/// Calculate compression ratio
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if compressed_size == 0 {
        return 0.0;
    }
    original_size as f64 / compressed_size as f64
}

/// Estimate if compression is worthwhile
pub fn should_compress(data: &[u8], min_size: usize, _min_ratio: f64) -> bool {
    if data.len() < min_size {
        return false;
    }

    // Quick entropy check: if data is already compressed/encrypted, skip
    // Count unique bytes in first 1KB
    let sample_size = data.len().min(1024);
    let mut seen = [false; 256];
    let mut unique_count = 0;

    for &byte in &data[..sample_size] {
        if !seen[byte as usize] {
            seen[byte as usize] = true;
            unique_count += 1;
        }
    }

    // If entropy is high (many unique bytes), compression likely won't help much
    let entropy_ratio = unique_count as f64 / 256.0;
    entropy_ratio < 0.9 // Compress if entropy is below 90%
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_compression() {
        let data = b"Hello, World! This is a test of LZ4 compression. ".repeat(100);
        
        let compressed = compress(&data, Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, Algorithm::Lz4).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_zstd_compression() {
        let data = b"Hello, World! This is a test of Zstandard compression. ".repeat(100);
        
        let compressed = compress(&data, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, Algorithm::Zstd).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";
        
        let compressed = compress(data, Algorithm::None, CompressionLevel::DEFAULT).unwrap();
        assert_eq!(data, compressed.as_slice());

        let decompressed = decompress(&compressed, Algorithm::None).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(compression_ratio(1000, 500), 2.0);
        assert_eq!(compression_ratio(1000, 250), 4.0);
        assert_eq!(compression_ratio(1000, 1000), 1.0);
    }

    #[test]
    fn test_should_compress() {
        // Small data should not be compressed
        let small_data = b"Hello";
        assert!(!should_compress(small_data, 1024, 1.5));

        // Repetitive data should be compressed
        let repetitive_data = b"AAAAAAAAAA".repeat(200);
        assert!(should_compress(&repetitive_data, 1024, 1.5));

        // Random data (high entropy) should not be compressed
        let random_data: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        assert!(!should_compress(&random_data, 1024, 1.5));
    }

    #[test]
    fn test_compression_levels() {
        let data = b"Test data for compression levels. ".repeat(100);

        // Fast compression
        let fast = compress(&data, Algorithm::Zstd, CompressionLevel::FAST).unwrap();
        
        // Default compression
        let default = compress(&data, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();
        
        // Best compression
        let best = compress(&data, Algorithm::Zstd, CompressionLevel::BEST).unwrap();

        // Higher levels should compress better (smaller size)
        assert!(best.len() <= default.len());
        assert!(default.len() <= fast.len());

        // All should decompress correctly
        assert_eq!(data, decompress(&fast, Algorithm::Zstd).unwrap().as_slice());
        assert_eq!(data, decompress(&default, Algorithm::Zstd).unwrap().as_slice());
        assert_eq!(data, decompress(&best, Algorithm::Zstd).unwrap().as_slice());
    }

    #[test]
    fn test_lz4_vs_zstd() {
        let data = b"Comparing LZ4 and Zstandard compression algorithms. ".repeat(100);

        let lz4_compressed = compress(&data, Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap();
        let zstd_compressed = compress(&data, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();

        // Zstd typically has better compression ratio
        println!("LZ4: {} bytes", lz4_compressed.len());
        println!("Zstd: {} bytes", zstd_compressed.len());

        // Both should decompress correctly
        assert_eq!(data, decompress(&lz4_compressed, Algorithm::Lz4).unwrap().as_slice());
        assert_eq!(data, decompress(&zstd_compressed, Algorithm::Zstd).unwrap().as_slice());
    }
}

// Made with Bob
