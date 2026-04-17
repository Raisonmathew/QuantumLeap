//! QLTP CLI - Command-line interface for high-speed file transfer

mod license;

use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use qltp_core::{chunking, Engine, TransferOptions};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "qltp")]
#[command(author = "QLTP Team <hello@qltp.io>")]
#[command(version = "0.1.0")]
#[command(about = "High-speed file transfer with intelligent optimization", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Transfer a file to a destination (local)
    Transfer {
        /// Source file path
        source: PathBuf,

        /// Destination (file path or remote URL)
        destination: String,

        /// Disable compression
        #[arg(long)]
        no_compression: bool,

        /// Disable deduplication
        #[arg(long)]
        no_dedup: bool,

        /// Enable encryption
        #[arg(short, long)]
        encrypt: bool,
    },

    /// Analyze a file and show chunking information
    Analyze {
        /// File path to analyze
        file: PathBuf,

        /// Chunk size in bytes
        #[arg(short, long, default_value = "4096")]
        chunk_size: usize,

        /// Use content-defined chunking
        #[arg(short = 'C', long)]
        content_defined: bool,
    },

    /// Show version and system information
    Info,

    /// License management commands
    License {
        #[command(subcommand)]
        command: LicenseCommands,
    },
}

#[derive(Subcommand)]
enum LicenseCommands {
    /// Create a new license
    Create {
        /// License tier (free, pro, team, business, enterprise)
        tier: String,

        /// Email address (optional)
        #[arg(short, long)]
        email: Option<String>,
    },

    /// Activate a device with a license key
    Activate {
        /// License key
        key: String,

        /// Device name (optional, defaults to hostname)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Show license status and quota information
    Status {
        /// License key
        key: String,
    },

    /// List available license tiers
    Tiers,

    /// Upgrade license tier
    Upgrade {
        /// Current license key
        key: String,

        /// New tier (pro, team, business, enterprise)
        tier: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let log_level = if cli.debug {
        Level::DEBUG
    } else if cli.verbose {
        Level::INFO
    } else {
        Level::WARN
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    match cli.command {
        Commands::Transfer {
            source,
            destination,
            no_compression,
            no_dedup,
            encrypt,
        } => {
            transfer_file(source, destination, no_compression, no_dedup, encrypt).await?;
        }
        Commands::Analyze {
            file,
            chunk_size,
            content_defined,
        } => {
            analyze_file(file, chunk_size, content_defined).await?;
        }
        Commands::Info => {
            show_info();
        }
        Commands::License { command } => match command {
            LicenseCommands::Create { tier, email } => {
                license::create_license(&tier, email).await?;
            }
            LicenseCommands::Activate { key, name } => {
                license::activate_device(&key, name).await?;
            }
            LicenseCommands::Status { key } => {
                license::show_status(&key).await?;
            }
            LicenseCommands::Tiers => {
                license::list_tiers();
            }
            LicenseCommands::Upgrade { key, tier } => {
                license::upgrade_tier(&key, &tier).await?;
            }
        },
    }

    Ok(())
}

async fn transfer_file(
    source: PathBuf,
    destination: String,
    no_compression: bool,
    no_dedup: bool,
    encrypt: bool,
) -> Result<()> {
    println!("🚀 QLTP Transfer");
    println!("Source: {}", source.display());
    println!("Destination: {}", destination);
    println!();

    // Check if source file exists
    if !source.exists() {
        anyhow::bail!("Source file does not exist: {}", source.display());
    }

    let metadata = std::fs::metadata(&source)?;
    let file_size = metadata.len();

    println!("File size: {} bytes ({:.2} MB)", file_size, file_size as f64 / 1024.0 / 1024.0);
    println!();

    // Create progress bar
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"),
    );

    // Create engine and options
    let engine = Engine::new().await?;
    let options = TransferOptions {
        compression: !no_compression,
        deduplication: !no_dedup,
        encryption: encrypt,
        ..Default::default()
    };

    info!("Starting transfer with options: {:?}", options);

    // Perform transfer
    let result = engine.transfer_file(&source, &destination, options).await?;

    pb.finish_with_message("Transfer complete!");
    println!();

    // Show results
    println!("✅ Transfer Complete");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Bytes transferred: {} bytes", result.bytes_transferred);
    println!("Duration: {:.2}s", result.duration.as_secs_f64());
    println!("Speed: {:.2} MB/s", result.speed_mbps());
    println!("Effective speed: {:.2} GB/s", result.effective_speed_gbps());
    println!("Compression ratio: {:.2}x", result.compression_ratio);
    
    // Show storage stats
    let stats = engine.storage_stats().await;
    println!("Storage: {} chunks, {:.2} MB total", stats.chunk_count, stats.total_size as f64 / 1024.0 / 1024.0);
    println!();

    Ok(())
}

async fn analyze_file(file: PathBuf, chunk_size: usize, content_defined: bool) -> Result<()> {
    println!("🔍 QLTP File Analysis");
    println!("File: {}", file.display());
    println!();

    // Check if file exists
    if !file.exists() {
        anyhow::bail!("File does not exist: {}", file.display());
    }

    let metadata = std::fs::metadata(&file)?;
    let file_size = metadata.len();

    println!("File size: {} bytes ({:.2} MB)", file_size, file_size as f64 / 1024.0 / 1024.0);
    println!("Chunk size: {} bytes", chunk_size);
    println!("Chunking method: {}", if content_defined { "Content-defined" } else { "Fixed-size" });
    println!();

    // Perform chunking
    let chunks = if content_defined {
        let chunker = chunking::ContentDefinedChunker::new(chunk_size);
        chunker.chunk_file(&file).await?
    } else {
        chunking::chunk_file(&file, chunk_size).await?
    };

    println!("📦 Chunking Results");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Total chunks: {}", chunks.len());
    
    if !chunks.is_empty() {
        let avg_size: usize = chunks.iter().map(|c| c.size).sum::<usize>() / chunks.len();
        let min_size = chunks.iter().map(|c| c.size).min().unwrap();
        let max_size = chunks.iter().map(|c| c.size).max().unwrap();

        println!("Average chunk size: {} bytes", avg_size);
        println!("Min chunk size: {} bytes", min_size);
        println!("Max chunk size: {} bytes", max_size);
        println!();

        // Show first few chunks
        println!("First 5 chunks:");
        for (i, chunk) in chunks.iter().take(5).enumerate() {
            println!(
                "  {}. {} bytes @ offset {} (hash: {}...)",
                i + 1,
                chunk.size,
                chunk.offset,
                &chunk.id.to_hex()[..16]
            );
        }

        if chunks.len() > 5 {
            println!("  ... and {} more chunks", chunks.len() - 5);
        }
    }

    println!();
    Ok(())
}

fn show_info() {
    println!("🚀 QLTP - Quantum Leap Transfer Protocol");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Authors: {}", env!("CARGO_PKG_AUTHORS"));
    println!("Homepage: {}", env!("CARGO_PKG_HOMEPAGE"));
    println!();
    println!("Features:");
    println!("  ✓ 5-layer optimization cascade");
    println!("  ✓ Content-addressable deduplication");
    println!("  ✓ Intelligent compression");
    println!("  ✓ High-speed transfer (10x faster)");
    println!("  ✓ 70-95% bandwidth reduction");
    println!();
    println!("System Information:");
    println!("  OS: {}", std::env::consts::OS);
    println!("  Architecture: {}", std::env::consts::ARCH);
    println!("  CPU cores: {}", num_cpus::get());
    println!();
}

// Made with Bob
