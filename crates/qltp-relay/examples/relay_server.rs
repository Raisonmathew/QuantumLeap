//! Example Relay Server
//!
//! Demonstrates running the unified relay service with STUN, TURN, and signaling

use qltp_relay::{RelayService, RelayServiceConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          QLTP Unified Relay Service v1.0                  ║");
    println!("║  High-Performance NAT Traversal & Connection Relay         ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Create configuration with defaults
    let config = RelayServiceConfig {
        signaling_addr: "0.0.0.0:8080".parse()?,
        stun_addr: "0.0.0.0:3478".parse()?,
        turn_addr: "0.0.0.0:3479".parse()?,
        turn_relay_base: "0.0.0.0:0".parse()?,
        max_turn_allocations: 1000,
        software: "QLTP-Relay/1.0".to_string(),
        ..Default::default()
    };

    // Create and start relay service
    let service = Arc::new(RelayService::new(config));
    let handles = service.clone().start().await?;

    println!();
    println!("📡 Relay Service Information:");
    println!("   WebSocket Signaling: ws://{}", service.signaling_endpoint());
    println!("   STUN Server:         {}", service.stun_endpoint());
    println!("   TURN Server:         {}", service.turn_endpoint());
    println!();
    println!("🔧 Connection Cascade Strategy:");
    println!("   1. Direct P2P (fastest, if NAT allows)");
    println!("   2. STUN-Assisted (NAT hole punching)");
    println!("   3. TURN Relay (guaranteed connectivity)");
    println!();
    println!("✓ All services running. Press Ctrl+C to stop.");
    println!();

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    println!("\n🛑 Shutting down relay service...");
    handles.abort_all();

    println!("✓ Relay service stopped.");

    Ok(())
}

// Made with Bob