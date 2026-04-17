//! Adaptive compression selection based on content analysis
//!
//! This module provides intelligent compression algorithm selection by analyzing
//! file content and choosing the optimal compression method for maximum efficiency.

use crate::compression::{compress_lz4, compress_zstd, decompress_lz4, decompress_zstd};
use crate::error::Result;
use std::path::Path;
use tracing::{debug, info};

/// Compression algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// LZ4 - Fast compression, lower ratio
    Lz4,
    /// Zstd - Balanced compression
    Zstd,
    /// Zstd with high compression level
    ZstdHigh,
}

/// Content type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// Text files (source code, documents, etc.)
    Text,
    /// Binary executables and libraries
    Binary,
    /// Already compressed (zip, gz, etc.)
    Compressed,
    /// Media files (images, video, audio)
    Media,
    /// Database files
    Database,
    /// Unknown content type
    Unknown,
}

/// Adaptive compression configuration
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Enable adaptive compression
    pub enabled: bool,
    /// Sample size for content analysis (bytes)
    pub sample_size: usize,
    /// Minimum compression ratio to use compression (e.g., 1.1 = 10% reduction)
    pub min_compression_ratio: f64,
    /// Force specific algorithm (overrides adaptive selection)
    pub force_algorithm: Option<CompressionAlgorithm>,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_size: 8192,
            min_compression_ratio: 1.05,
            force_algorithm: None,
        }
    }
}

/// Adaptive compression analyzer
pub struct AdaptiveCompressor {
    config: AdaptiveConfig,
}

impl AdaptiveCompressor {
    /// Create a new adaptive compressor
    pub fn new(config: AdaptiveConfig) -> Self {
        Self { config }
    }

    /// Analyze content and select optimal compression algorithm
    pub fn select_algorithm(&self, data: &[u8], path: Option<&Path>) -> CompressionAlgorithm {
        // If forced algorithm is set, use it
        if let Some(algo) = self.config.force_algorithm {
            debug!("Using forced compression algorithm: {:?}", algo);
            return algo;
        }

        // If adaptive compression is disabled, use default
        if !self.config.enabled {
            return CompressionAlgorithm::Lz4;
        }

        // Classify content type
        let content_type = self.classify_content(data, path);
        debug!("Detected content type: {:?}", content_type);

        // Select algorithm based on content type
        let algorithm = match content_type {
            ContentType::Text => {
                // Text compresses very well with Zstd
                CompressionAlgorithm::ZstdHigh
            }
            ContentType::Binary => {
                // Binaries benefit from balanced compression
                CompressionAlgorithm::Zstd
            }
            ContentType::Compressed | ContentType::Media => {
                // Already compressed, skip compression
                CompressionAlgorithm::None
            }
            ContentType::Database => {
                // Databases often have patterns, use fast compression
                CompressionAlgorithm::Lz4
            }
            ContentType::Unknown => {
                // Test compression ratio with sample
                self.test_compression_ratio(data)
            }
        };

        info!("Selected compression algorithm: {:?} for content type: {:?}", algorithm, content_type);
        algorithm
    }

    /// Classify content type based on data and file extension
    fn classify_content(&self, data: &[u8], path: Option<&Path>) -> ContentType {
        // Check file extension first
        if let Some(path) = path {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                
                // Already compressed formats
                if matches!(ext_lower.as_str(), 
                    "zip" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tar" | "tgz" | "tbz2"
                ) {
                    return ContentType::Compressed;
                }
                
                // Media formats
                if matches!(ext_lower.as_str(),
                    "jpg" | "jpeg" | "png" | "gif" | "webp" | "mp4" | "mkv" | "avi" | 
                    "mp3" | "flac" | "ogg" | "wav" | "m4a" | "mov" | "wmv"
                ) {
                    return ContentType::Media;
                }
                
                // Text formats
                if matches!(ext_lower.as_str(),
                    "txt" | "md" | "json" | "xml" | "yaml" | "yml" | "toml" | "ini" |
                    "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "h" | "hpp" |
                    "go" | "rb" | "php" | "html" | "css" | "scss" | "sql" | "sh" | "bash"
                ) {
                    return ContentType::Text;
                }
                
                // Binary formats
                if matches!(ext_lower.as_str(),
                    "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "a" | "lib"
                ) {
                    return ContentType::Binary;
                }
                
                // Database formats
                if matches!(ext_lower.as_str(),
                    "db" | "sqlite" | "sqlite3" | "mdb" | "accdb"
                ) {
                    return ContentType::Database;
                }
            }
        }

        // Analyze content if no extension match
        self.analyze_content_bytes(data)
    }

    /// Analyze raw bytes to determine content type
    fn analyze_content_bytes(&self, data: &[u8]) -> ContentType {
        if data.is_empty() {
            return ContentType::Unknown;
        }

        let sample_size = self.config.sample_size.min(data.len());
        let sample = &data[..sample_size];

        // Check for common file signatures (magic numbers)
        if sample.len() >= 4 {
            let magic = &sample[0..4];
            
            // ZIP signature
            if magic == b"PK\x03\x04" || magic == b"PK\x05\x06" {
                return ContentType::Compressed;
            }
            
            // GZIP signature
            if magic[0..2] == [0x1f, 0x8b] {
                return ContentType::Compressed;
            }
            
            // PNG signature
            if magic == b"\x89PNG" {
                return ContentType::Media;
            }
            
            // JPEG signature
            if magic[0..2] == [0xff, 0xd8] {
                return ContentType::Media;
            }
            
            // ELF binary
            if magic == b"\x7fELF" {
                return ContentType::Binary;
            }
            
            // PE binary (Windows)
            if magic[0..2] == [0x4d, 0x5a] { // MZ
                return ContentType::Binary;
            }
            
            // Mach-O binary (macOS)
            if magic == b"\xfe\xed\xfa\xce" || magic == b"\xfe\xed\xfa\xcf" ||
               magic == b"\xce\xfa\xed\xfe" || magic == b"\xcf\xfa\xed\xfe" {
                return ContentType::Binary;
            }
        }

        // Analyze byte distribution
        let mut ascii_count = 0;
        let mut printable_count = 0;
        let mut null_count = 0;

        for &byte in sample {
            if byte == 0 {
                null_count += 1;
            } else if byte.is_ascii() {
                ascii_count += 1;
                if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
                    printable_count += 1;
                }
            }
        }

        let ascii_ratio = ascii_count as f64 / sample_size as f64;
        let printable_ratio = printable_count as f64 / sample_size as f64;
        let null_ratio = null_count as f64 / sample_size as f64;

        // High printable ratio suggests text
        if printable_ratio > 0.85 {
            return ContentType::Text;
        }

        // High null byte ratio suggests binary
        if null_ratio > 0.1 {
            return ContentType::Binary;
        }

        // High ASCII but not printable suggests binary
        if ascii_ratio > 0.7 && printable_ratio < 0.6 {
            return ContentType::Binary;
        }

        ContentType::Unknown
    }

    /// Test compression ratio with a sample
    fn test_compression_ratio(&self, data: &[u8]) -> CompressionAlgorithm {
        let sample_size = self.config.sample_size.min(data.len());
        let sample = &data[..sample_size];

        // Try LZ4 compression
        if let Ok(compressed) = compress_lz4(sample) {
            let ratio = sample.len() as f64 / compressed.len() as f64;
            
            if ratio >= self.config.min_compression_ratio {
                // Good compression, try Zstd for potentially better ratio
                if let Ok(zstd_compressed) = compress_zstd(sample, 3) {
                    let zstd_ratio = sample.len() as f64 / zstd_compressed.len() as f64;
                    
                    if zstd_ratio > ratio * 1.1 {
                        // Zstd is significantly better
                        return CompressionAlgorithm::Zstd;
                    }
                }
                
                // LZ4 is good enough
                return CompressionAlgorithm::Lz4;
            }
        }

        // Poor compression ratio, don't compress
        CompressionAlgorithm::None
    }

    /// Compress data using selected algorithm
    pub fn compress(&self, data: &[u8], path: Option<&Path>) -> Result<(Vec<u8>, CompressionAlgorithm)> {
        let algorithm = self.select_algorithm(data, path);

        let compressed = match algorithm {
            CompressionAlgorithm::None => {
                debug!("Skipping compression");
                data.to_vec()
            }
            CompressionAlgorithm::Lz4 => {
                debug!("Compressing with LZ4");
                compress_lz4(data)?
            }
            CompressionAlgorithm::Zstd => {
                debug!("Compressing with Zstd (level 3)");
                compress_zstd(data, 3)?
            }
            CompressionAlgorithm::ZstdHigh => {
                debug!("Compressing with Zstd (level 9)");
                compress_zstd(data, 9)?
            }
        };

        let ratio = if compressed.len() > 0 {
            data.len() as f64 / compressed.len() as f64
        } else {
            1.0
        };

        info!("Compression complete: {} bytes -> {} bytes (ratio: {:.2}x, algorithm: {:?})",
              data.len(), compressed.len(), ratio, algorithm);

        Ok((compressed, algorithm))
    }

    /// Decompress data using specified algorithm
    pub fn decompress(&self, data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<u8>> {
        match algorithm {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Lz4 => decompress_lz4(data),
            CompressionAlgorithm::Zstd | CompressionAlgorithm::ZstdHigh => decompress_zstd(data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_from_extension() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        // Text files
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("test.txt"))),
            ContentType::Text
        );
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("code.rs"))),
            ContentType::Text
        );

        // Compressed files
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("archive.zip"))),
            ContentType::Compressed
        );
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("data.gz"))),
            ContentType::Compressed
        );

        // Media files
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("image.jpg"))),
            ContentType::Media
        );
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("video.mp4"))),
            ContentType::Media
        );

        // Binary files
        assert_eq!(
            compressor.classify_content(b"", Some(Path::new("program.exe"))),
            ContentType::Binary
        );
    }

    #[test]
    fn test_content_type_from_magic_numbers() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        // ZIP magic
        let zip_data = b"PK\x03\x04test data";
        assert_eq!(
            compressor.analyze_content_bytes(zip_data),
            ContentType::Compressed
        );

        // GZIP magic
        let gzip_data = b"\x1f\x8btest data";
        assert_eq!(
            compressor.analyze_content_bytes(gzip_data),
            ContentType::Compressed
        );

        // PNG magic
        let png_data = b"\x89PNGtest data";
        assert_eq!(
            compressor.analyze_content_bytes(png_data),
            ContentType::Media
        );
    }

    #[test]
    fn test_text_detection() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        let text_data = b"This is plain text content with lots of readable characters.";
        assert_eq!(
            compressor.analyze_content_bytes(text_data),
            ContentType::Text
        );
    }

    #[test]
    fn test_binary_detection() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        let binary_data = vec![0u8; 100]; // Lots of null bytes
        assert_eq!(
            compressor.analyze_content_bytes(&binary_data),
            ContentType::Binary
        );
    }

    #[test]
    fn test_algorithm_selection_text() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        let text_data = b"This is text content that should compress well with Zstd.";
        let algorithm = compressor.select_algorithm(text_data, Some(Path::new("test.txt")));
        
        assert_eq!(algorithm, CompressionAlgorithm::ZstdHigh);
    }

    #[test]
    fn test_algorithm_selection_compressed() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        let data = b"some data";
        let algorithm = compressor.select_algorithm(data, Some(Path::new("archive.zip")));
        
        assert_eq!(algorithm, CompressionAlgorithm::None);
    }

    #[test]
    fn test_forced_algorithm() {
        let config = AdaptiveConfig {
            force_algorithm: Some(CompressionAlgorithm::Lz4),
            ..Default::default()
        };
        let compressor = AdaptiveCompressor::new(config);

        let data = b"any data";
        let algorithm = compressor.select_algorithm(data, Some(Path::new("test.txt")));
        
        assert_eq!(algorithm, CompressionAlgorithm::Lz4);
    }

    #[test]
    fn test_compress_decompress_text() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        let original = b"This is test data that should compress well. ".repeat(10);
        let (compressed, algorithm) = compressor.compress(&original, Some(Path::new("test.txt")))
            .expect("Compression failed");

        assert!(compressed.len() < original.len());
        assert_eq!(algorithm, CompressionAlgorithm::ZstdHigh);

        let decompressed = compressor.decompress(&compressed, algorithm)
            .expect("Decompression failed");

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_already_compressed() {
        let compressor = AdaptiveCompressor::new(AdaptiveConfig::default());

        let data = b"PK\x03\x04random compressed data";
        let (compressed, algorithm) = compressor.compress(data, Some(Path::new("archive.zip")))
            .expect("Compression failed");

        assert_eq!(algorithm, CompressionAlgorithm::None);
        assert_eq!(compressed, data);
    }
}

// Made with Bob
