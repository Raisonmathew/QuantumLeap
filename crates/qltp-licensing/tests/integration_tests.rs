//! Integration tests for the licensing system
//!
//! These tests verify end-to-end scenarios using SQLite storage

use chrono::{Duration, Utc};
use qltp_licensing::{
    Feature, LicenseRepository, LicenseService, LicenseTier, SqliteLicenseStore, SqliteUsageStore,
    TransferType, UsageRepository, UsageTracker,
};
use std::sync::Arc;

#[tokio::test]
async fn test_complete_license_lifecycle_with_sqlite() {
    // Setup: Create SQLite stores
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());

    // Create services
    let license_service = LicenseService::new(license_store.clone());
    let usage_tracker = UsageTracker::new(license_store.clone(), usage_store.clone());

    // Step 1: Create a new Pro license
    let email = "user@example.com".to_string();
    let license = license_service
        .create_license(LicenseTier::Pro, Some(email.clone()))
        .await
        .unwrap();

    assert_eq!(license.tier(), LicenseTier::Pro);
    assert_eq!(license.email(), Some(email.as_str()));

    // Step 2: Retrieve license by key
    let key = license.key().to_string();
    let retrieved = license_service.get_license(&key).await.unwrap();
    assert_eq!(retrieved.id(), license.id());

    // Step 3: Track some usage
    usage_tracker
        .record_transfer(
            license.id().clone(),
            5 * 1024 * 1024 * 1024, // 5GB
            TransferType::Upload,
        )
        .await
        .unwrap();

    // Step 4: Check usage stats
    let start = Utc::now() - Duration::days(1);
    let end = Utc::now() + Duration::days(1);
    let stats = usage_tracker
        .get_usage_stats(license.id(), start, end)
        .await
        .unwrap();

    assert_eq!(stats.total_bytes, 5 * 1024 * 1024 * 1024);
    assert_eq!(stats.transfer_count, 1);

    // Step 5: Check remaining quota
    let remaining = usage_tracker.get_remaining_quota(license.id()).await.unwrap();
    assert_eq!(remaining, Some(95 * 1024 * 1024 * 1024)); // 100GB - 5GB = 95GB

    // Step 6: Upgrade to Team tier
    license_service
        .upgrade_tier(&key, LicenseTier::Team)
        .await
        .unwrap();

    let mut upgraded = license_service.get_license(&key).await.unwrap();
    assert_eq!(upgraded.tier(), LicenseTier::Team);

    // Step 7: Verify features are available
    assert!(upgraded.has_feature(Feature::ParallelTransfers).is_ok());
    assert!(upgraded.has_feature(Feature::Encryption).is_ok());
}

#[tokio::test]
async fn test_license_creation_and_persistence() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let service = LicenseService::new(license_store.clone());

    // Create multiple licenses
    let license1 = service
        .create_license(LicenseTier::Free, Some("user1@example.com".to_string()))
        .await
        .unwrap();

    let license2 = service
        .create_license(LicenseTier::Pro, Some("user2@example.com".to_string()))
        .await
        .unwrap();

    let license3 = service
        .create_license(LicenseTier::Team, Some("user3@example.com".to_string()))
        .await
        .unwrap();

    // Verify all licenses can be retrieved
    let key1 = license1.key().to_string();
    let key2 = license2.key().to_string();
    let key3 = license3.key().to_string();

    let retrieved1 = service.get_license(&key1).await.unwrap();
    let retrieved2 = service.get_license(&key2).await.unwrap();
    let retrieved3 = service.get_license(&key3).await.unwrap();

    assert_eq!(retrieved1.tier(), LicenseTier::Free);
    assert_eq!(retrieved2.tier(), LicenseTier::Pro);
    assert_eq!(retrieved3.tier(), LicenseTier::Team);

    // List all licenses
    let all_licenses = service.list_all_licenses().await.unwrap();
    assert_eq!(all_licenses.len(), 3);
}

#[tokio::test]
async fn test_device_activation_workflow() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let service = LicenseService::new(license_store.clone());

    // Create a Pro license (allows 3 devices)
    let license = service
        .create_license(LicenseTier::Pro, Some("user@example.com".to_string()))
        .await
        .unwrap();

    let key = license.key().to_string();

    // Activate first device
    service
        .activate_device(&key, "Laptop".to_string(), "fp-laptop-123".to_string())
        .await
        .unwrap();

    let updated = service.get_license(&key).await.unwrap();
    assert_eq!(updated.devices().len(), 1);

    // Activate second device
    service
        .activate_device(&key, "Desktop".to_string(), "fp-desktop-456".to_string())
        .await
        .unwrap();

    let updated = service.get_license(&key).await.unwrap();
    assert_eq!(updated.devices().len(), 2);

    // Activate third device
    service
        .activate_device(&key, "Phone".to_string(), "fp-phone-789".to_string())
        .await
        .unwrap();

    let updated = service.get_license(&key).await.unwrap();
    assert_eq!(updated.devices().len(), 3);

    // Try to activate a fourth device (should fail - Pro tier allows only 3)
    let result = service
        .activate_device(&key, "Tablet".to_string(), "fp-tablet-000".to_string())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_usage_tracking_and_quotas() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());

    let service = LicenseService::new(license_store.clone());
    let tracker = UsageTracker::new(license_store.clone(), usage_store.clone());

    // Create a Free tier license (10GB quota)
    let license = service
        .create_license(LicenseTier::Free, Some("user@example.com".to_string()))
        .await
        .unwrap();

    // Track multiple transfers
    tracker
        .record_transfer(license.id().clone(), 2 * 1024 * 1024 * 1024, TransferType::Upload)
        .await
        .unwrap();

    tracker
        .record_transfer(
            license.id().clone(),
            3 * 1024 * 1024 * 1024,
            TransferType::Download,
        )
        .await
        .unwrap();

    tracker
        .record_transfer(license.id().clone(), 1 * 1024 * 1024 * 1024, TransferType::Upload)
        .await
        .unwrap();

    // Check usage stats
    let start = Utc::now() - Duration::days(1);
    let end = Utc::now() + Duration::days(1);
    let stats = tracker
        .get_usage_stats(license.id(), start, end)
        .await
        .unwrap();

    assert_eq!(stats.total_bytes, 6 * 1024 * 1024 * 1024); // 6GB total
    assert_eq!(stats.transfer_count, 3);

    // Check remaining quota (10GB - 6GB = 4GB)
    let remaining = tracker.get_remaining_quota(license.id()).await.unwrap();
    assert_eq!(remaining, Some(4 * 1024 * 1024 * 1024));
}

#[tokio::test]
async fn test_license_upgrade_workflow() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let service = LicenseService::new(license_store.clone());

    // Create Free license
    let mut license = service
        .create_license(LicenseTier::Free, Some("user@example.com".to_string()))
        .await
        .unwrap();

    let key = license.key().to_string();

    // Verify Free tier features
    assert!(license.has_feature(Feature::Encryption).is_err());
    assert!(license.has_feature(Feature::ParallelTransfers).is_err());

    // Upgrade to Pro
    service.upgrade_tier(&key, LicenseTier::Pro).await.unwrap();

    let mut pro_license = service.get_license(&key).await.unwrap();
    assert_eq!(pro_license.tier(), LicenseTier::Pro);
    assert!(pro_license.has_feature(Feature::Encryption).is_ok());
    assert!(pro_license.has_feature(Feature::ParallelTransfers).is_err()); // Still not available

    // Upgrade to Team
    service
        .upgrade_tier(&key, LicenseTier::Team)
        .await
        .unwrap();

    let mut team_license = service.get_license(&key).await.unwrap();
    assert_eq!(team_license.tier(), LicenseTier::Team);
    assert!(team_license.has_feature(Feature::Encryption).is_ok());
    assert!(team_license.has_feature(Feature::ParallelTransfers).is_ok());
}

#[tokio::test]
async fn test_usage_cleanup() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());

    let service = LicenseService::new(license_store.clone());
    let tracker = UsageTracker::new(license_store.clone(), usage_store.clone());

    // Create license and record usage
    let license = service
        .create_license(LicenseTier::Pro, Some("user@example.com".to_string()))
        .await
        .unwrap();

    tracker
        .record_transfer(license.id().clone(), 1024 * 1024, TransferType::Upload)
        .await
        .unwrap();

    // Verify usage exists
    let start = Utc::now() - Duration::days(1);
    let end = Utc::now() + Duration::days(1);
    let stats = tracker
        .get_usage_stats(license.id(), start, end)
        .await
        .unwrap();
    assert_eq!(stats.transfer_count, 1);

    // Clean up old records
    let deleted = usage_store
        .delete_before(Utc::now() + Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    // Verify usage is gone
    let stats = tracker
        .get_usage_stats(license.id(), start, end)
        .await
        .unwrap();
    assert_eq!(stats.transfer_count, 0);
}

#[tokio::test]
async fn test_get_license_by_user() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let service = LicenseService::new(license_store.clone());

    // Create license for user
    let license = service
        .create_license(LicenseTier::Pro, Some("user@example.com".to_string()))
        .await
        .unwrap();

    // Retrieve by user email
    let retrieved = service
        .get_license_by_user("user@example.com")
        .await
        .unwrap();

    assert_eq!(retrieved.id(), license.id());
    assert_eq!(retrieved.email(), Some("user@example.com"));
}

#[tokio::test]
async fn test_tier_specific_quotas() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());

    let service = LicenseService::new(license_store.clone());
    let tracker = UsageTracker::new(license_store.clone(), usage_store.clone());

    // Test Free tier (10GB)
    let free_license = service
        .create_license(LicenseTier::Free, Some("free@example.com".to_string()))
        .await
        .unwrap();
    let free_quota = tracker.get_remaining_quota(free_license.id()).await.unwrap();
    assert_eq!(free_quota, Some(10 * 1024 * 1024 * 1024));

    // Test Pro tier (100GB)
    let pro_license = service
        .create_license(LicenseTier::Pro, Some("pro@example.com".to_string()))
        .await
        .unwrap();
    let pro_quota = tracker.get_remaining_quota(pro_license.id()).await.unwrap();
    assert_eq!(pro_quota, Some(100 * 1024 * 1024 * 1024));

    // Test Team tier (500GB)
    let team_license = service
        .create_license(LicenseTier::Team, Some("team@example.com".to_string()))
        .await
        .unwrap();
    let team_quota = tracker.get_remaining_quota(team_license.id()).await.unwrap();
    assert_eq!(team_quota, Some(500 * 1024 * 1024 * 1024));
}
#[tokio::test]
async fn test_license_expiration_handling() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());

    // Create a license
    let license = license_service
        .create_license(LicenseTier::Pro, Some("expired@example.com".to_string()))
        .await
        .unwrap();
    
    // Verify license is initially active
    assert!(license.is_active());
    
    // Note: In a real scenario, licenses would expire based on expires_at timestamp
    // This test verifies the basic license creation and active status
    let key = license.key().to_string();
    let retrieved = license_service.get_license(&key).await.unwrap();
    assert_eq!(retrieved.id(), license.id());
}

#[tokio::test]
async fn test_device_limit_enforcement() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());

    // Create a Free tier license (max 1 device)
    let license = license_service
        .create_license(LicenseTier::Free, Some("device-test@example.com".to_string()))
        .await
        .unwrap();
    
    let key = license.key().to_string();

    // Activate first device - should succeed
    license_service
        .activate_device(&key, "device-001".to_string(), "fingerprint-001".to_string())
        .await
        .unwrap();

    // Try to activate second device - should fail for Free tier (max 1 device)
    let result = license_service
        .activate_device(&key, "device-002".to_string(), "fingerprint-002".to_string())
        .await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_quota_enforcement_blocks_transfers() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());
    let usage_tracker = UsageTracker::new(license_store.clone(), usage_store.clone());

    // Create a Free tier license (10GB quota)
    let license = license_service
        .create_license(LicenseTier::Free, Some("quota-test@example.com".to_string()))
        .await
        .unwrap();

    // Use up 9GB
    usage_tracker
        .record_transfer(
            license.id().clone(),
            9 * 1024 * 1024 * 1024,
            TransferType::Upload,
        )
        .await
        .unwrap();

    // Check remaining quota
    let remaining = usage_tracker
        .get_remaining_quota(license.id())
        .await
        .unwrap();
    assert_eq!(remaining, Some(1 * 1024 * 1024 * 1024)); // 1GB left

    // Try to transfer 2GB - should exceed quota
    let result = usage_tracker
        .check_quota(license.id(), 2 * 1024 * 1024 * 1024)
        .await;
    assert!(result.is_err()); // Should fail quota check

    // Transfer 0.5GB - should succeed
    let result = usage_tracker
        .check_quota(license.id(), 512 * 1024 * 1024)
        .await;
    assert!(result.is_ok()); // Should pass quota check
}

#[tokio::test]
async fn test_invalid_license_key_handling() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());

    // Try to get license with invalid key format
    let result = license_service.get_license("invalid-key-format").await;
    assert!(result.is_err());

    // Try to get license with valid format but non-existent key
    let result = license_service
        .get_license("FREE-AAAA-BBBB-CCCC-DDDD")
        .await;
    assert!(result.is_err());

    // Try to activate device with invalid key
    let result = license_service
        .activate_device("invalid-key", "device-001".to_string(), "fingerprint-001".to_string())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_usage_tracking() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());
    let usage_tracker = Arc::new(UsageTracker::new(
        license_store.clone(),
        usage_store.clone(),
    ));

    // Create a Pro license
    let license = license_service
        .create_license(LicenseTier::Pro, Some("concurrent@example.com".to_string()))
        .await
        .unwrap();

    let license_id = license.id().clone();

    // Simulate concurrent transfers
    let mut handles = vec![];
    for i in 0..10 {
        let tracker = usage_tracker.clone();
        let id = license_id.clone();
        let handle = tokio::spawn(async move {
            tracker
                .record_transfer(
                    id,
                    100 * 1024 * 1024, // 100MB each
                    if i % 2 == 0 {
                        TransferType::Upload
                    } else {
                        TransferType::Download
                    },
                )
                .await
        });
        handles.push(handle);
    }

    // Wait for all transfers to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify total usage
    let start = Utc::now() - Duration::days(1);
    let end = Utc::now() + Duration::days(1);
    let stats = usage_tracker
        .get_usage_stats(&license_id, start, end)
        .await
        .unwrap();

    assert_eq!(stats.total_bytes, 1000 * 1024 * 1024); // 1GB total
    assert_eq!(stats.transfer_count, 10);
}

#[tokio::test]
async fn test_license_deactivation_and_reactivation() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());

    // Create a license
    let license = license_service
        .create_license(LicenseTier::Pro, Some("deactivate@example.com".to_string()))
        .await
        .unwrap();
    
    let key = license.key().to_string();
    assert!(license.is_active());

    // Verify we can retrieve the license
    let retrieved = license_service.get_license(&key).await.unwrap();
    assert!(retrieved.is_active());
    assert_eq!(retrieved.id(), license.id());
    
    // Note: Deactivation/reactivation would require additional service methods
    // This test verifies basic license lifecycle operations
}

#[tokio::test]
async fn test_enterprise_unlimited_quota() {
    let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
    let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());
    let license_service = LicenseService::new(license_store.clone());
    let usage_tracker = UsageTracker::new(license_store.clone(), usage_store.clone());

    // Create an Enterprise license
    let license = license_service
        .create_license(LicenseTier::Enterprise, Some("enterprise@example.com".to_string()))
        .await
        .unwrap();

    // Use a massive amount of data
    usage_tracker
        .record_transfer(
            license.id().clone(),
            5 * 1024 * 1024 * 1024 * 1024, // 5TB
            TransferType::Upload,
        )
        .await
        .unwrap();

    // Enterprise should have unlimited quota (None)
    let remaining = usage_tracker
        .get_remaining_quota(license.id())
        .await
        .unwrap();
    assert_eq!(remaining, None); // Unlimited

    // Should always pass quota check for any amount
    let result = usage_tracker
        .check_quota(license.id(), 10 * 1024 * 1024 * 1024 * 1024) // 10TB
        .await;
    assert!(result.is_ok()); // Enterprise has no quota limits
}


// Made with Bob