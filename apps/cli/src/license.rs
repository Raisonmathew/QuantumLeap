//! License management commands

use anyhow::Result;
use qltp_licensing::{
    Feature, LicenseService, LicenseTier, MemoryLicenseStore, MemoryUsageStore, UsageTracker,
};
use std::sync::Arc;

/// Create a new license
pub async fn create_license(tier: &str, email: Option<String>) -> Result<()> {
    let tier = tier
        .parse::<LicenseTier>()
        .map_err(|e| anyhow::anyhow!("Invalid license tier: {}", e))?;

    let store = Arc::new(MemoryLicenseStore::new());
    let service = LicenseService::new(store);

    let license = service.create_license(tier, email).await?;

    println!("✓ License created successfully!");
    println!();
    println!("License Key: {}", license.key());
    println!("Tier: {}", license.tier());
    println!("License ID: {}", license.id());
    if let Some(email) = license.email() {
        println!("Email: {}", email);
    }
    println!();
    println!("Save this license key - you'll need it to activate devices.");

    Ok(())
}

/// Activate a device with a license key
pub async fn activate_device(key: &str, device_name: Option<String>) -> Result<()> {
    let store = Arc::new(MemoryLicenseStore::new());
    let service = LicenseService::new(store);

    // Generate device fingerprint
    let fingerprint = generate_device_fingerprint();
    let device_name = device_name.unwrap_or_else(|| {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string())
    });

    service
        .activate_device(key, device_name.clone(), fingerprint)
        .await?;

    println!("✓ Device activated successfully!");
    println!();
    println!("Device: {}", device_name);
    println!("License Key: {}", key);

    Ok(())
}

/// Show license status and quota information
pub async fn show_status(key: &str) -> Result<()> {
    let license_store = Arc::new(MemoryLicenseStore::new());
    let usage_store = Arc::new(MemoryUsageStore::new());
    let service = LicenseService::new(license_store.clone());
    let tracker = UsageTracker::new(license_store, usage_store);

    let license = service.get_license(key).await?;

    println!("License Status");
    println!("═══════════════════════════════════════");
    println!();
    println!("License Key: {}", license.key());
    println!("Tier: {}", license.tier());
    println!("Status: {}", if license.is_expired() { "❌ Expired" } else { "✓ Active" });
    
    if let Some(email) = license.email() {
        println!("Email: {}", email);
    }

    println!();
    println!("Quota Information");
    println!("───────────────────────────────────────");
    
    let quota = qltp_licensing::Quota::for_tier(license.tier());
    let current_usage = tracker.get_current_month_usage(license.id()).await?;
    let remaining = quota.monthly_bytes().saturating_sub(current_usage);

    println!("Monthly Limit: {}", format_bytes(quota.monthly_bytes()));
    println!("Used This Month: {}", format_bytes(current_usage));
    println!("Remaining: {}", format_bytes(remaining));
    println!("Max File Size: {}", format_bytes(quota.max_file_size()));
    println!("Max Concurrent: {}", quota.max_concurrent());

    println!();
    println!("Features");
    println!("───────────────────────────────────────");
    
    let features = license.features();
    print_feature("Compression", features.has_feature(Feature::Compression));
    print_feature("Adaptive Compression", features.has_feature(Feature::AdaptiveCompression));
    print_feature("Deduplication", features.has_feature(Feature::Deduplication));
    print_feature("Encryption", features.has_feature(Feature::Encryption));
    print_feature("Resume", features.has_feature(Feature::Resume));
    print_feature("Parallel Transfers", features.has_feature(Feature::ParallelTransfers));
    print_feature("QUIC Protocol", features.has_feature(Feature::Quic));
    print_feature("Prefetch", features.has_feature(Feature::Prefetch));
    print_feature("Priority", features.has_feature(Feature::Priority));
    print_feature("Collaboration", features.has_feature(Feature::Collaboration));
    print_feature("API Access", features.has_feature(Feature::ApiAccess));
    print_feature("Custom Branding", features.has_feature(Feature::CustomBranding));
    print_feature("Audit Logs", features.has_feature(Feature::AuditLogs));
    print_feature("SSO", features.has_feature(Feature::Sso));

    println!();
    println!("Devices ({}/{})", license.devices().len(), license.tier().max_devices());
    println!("───────────────────────────────────────");
    
    if license.devices().is_empty() {
        println!("No devices activated");
    } else {
        for device in license.devices() {
            println!("• {} ({})", device.name(), device.os());
        }
    }

    Ok(())
}

/// List available license tiers
pub fn list_tiers() {
    println!("Available License Tiers");
    println!("═══════════════════════════════════════");
    println!();

    print_tier_info(LicenseTier::Free);
    println!();
    print_tier_info(LicenseTier::Pro);
    println!();
    print_tier_info(LicenseTier::Team);
    println!();
    print_tier_info(LicenseTier::Business);
    println!();
    print_tier_info(LicenseTier::Enterprise);
}

/// Upgrade license tier
pub async fn upgrade_tier(key: &str, new_tier: &str) -> Result<()> {
    let new_tier = new_tier
        .parse::<LicenseTier>()
        .map_err(|e| anyhow::anyhow!("Invalid license tier: {}", e))?;

    let store = Arc::new(MemoryLicenseStore::new());
    let service = LicenseService::new(store);

    let license = service.upgrade_tier(key, new_tier).await?;

    println!("✓ License upgraded successfully!");
    println!();
    println!("New Tier: {}", license.tier());
    println!("New License Key: {}", license.key());
    println!();
    println!("Your license has been upgraded. Use the new key for future operations.");

    Ok(())
}

// Helper functions

fn generate_device_fingerprint() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());
    
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let mut hasher = DefaultHasher::new();
    hostname.hash(&mut hasher);
    os.hash(&mut hasher);
    arch.hash(&mut hasher);
    username.hash(&mut hasher);

    format!("{:x}", hasher.finish())
}

fn format_bytes(bytes: u64) -> String {
    if bytes == u64::MAX {
        return "Unlimited".to_string();
    }

    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

fn print_feature(name: &str, enabled: bool) {
    let status = if enabled { "✓" } else { "✗" };
    println!("{} {}", status, name);
}

fn print_tier_info(tier: LicenseTier) {
    let price = match tier.monthly_price_cents() {
        0 if tier == LicenseTier::Free => "$0/month".to_string(),
        0 => "Custom pricing".to_string(),
        cents => format!("${:.2}/month", cents as f64 / 100.0),
    };

    println!("{} - {}", tier, price);
    println!("  Monthly Quota: {}", format_bytes(tier.monthly_quota()));
    println!("  Max File Size: {}", format_bytes(tier.max_file_size()));
    println!("  Max Devices: {}", if tier.max_devices() == usize::MAX { "Unlimited".to_string() } else { tier.max_devices().to_string() });
    println!("  Concurrent Transfers: {}", tier.max_concurrent_transfers());
}

// Made with Bob
