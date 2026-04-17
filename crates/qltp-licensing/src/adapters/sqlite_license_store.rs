//! SQLite-based license repository implementation

use crate::domain::license::{License, LicenseId, LicenseKey, LicenseTier};
use crate::error::{LicenseError, Result};
use crate::ports::LicenseRepository;
use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// SQLite-based license storage
pub struct SqliteLicenseStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteLicenseStore {
    /// Create a new SQLite license store
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS licenses (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                key TEXT NOT NULL UNIQUE,
                tier TEXT NOT NULL,
                email TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                is_active INTEGER NOT NULL
            )",
            [],
        )?;

        // Create index on key for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_licenses_key ON licenses(key)",
            [],
        )?;

        // Create index on email for user lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_licenses_email ON licenses(email)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self> {
        Self::new(":memory:")
    }
}

#[async_trait]
impl LicenseRepository for SqliteLicenseStore {
    async fn save(&self, license: &License) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let data = serde_json::to_string(license)?;
        
        conn.execute(
            "INSERT INTO licenses (id, data, key, tier, email, created_at, expires_at, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                license.id().to_string(),
                data,
                license.key().to_string(),
                license.tier().as_str(),
                license.email(),
                license.created_at().to_rfc3339(),
                license.expires_at().map(|dt| dt.to_rfc3339()),
                if license.is_active() { 1 } else { 0 },
            ],
        )?;

        Ok(())
    }

    async fn find_by_id(&self, id: &LicenseId) -> Result<Option<License>> {
        let conn = self.conn.lock().unwrap();
        
        let result = conn
            .query_row(
                "SELECT id, tier, email, key, is_active, created_at, expires_at
                 FROM licenses WHERE id = ?1",
                params![id.as_str()],
                |row| {
                    let id_str: String = row.get(0)?;
                    let tier_str: String = row.get(1)?;
                    let email: Option<String> = row.get(2)?;
                    let key_str: String = row.get(3)?;
                    let is_active: bool = row.get(4)?;
                    let created_at_str: String = row.get(5)?;
                    let expires_at_str: Option<String> = row.get(6)?;
                    
                    Ok((id_str, tier_str, email, key_str, is_active, created_at_str, expires_at_str))
                },
            )
            .optional()?;

        match result {
            Some((_id_str, tier_str, email, _key_str, _is_active, _created_at_str, _expires_at_str)) => {
                let tier = match tier_str.as_str() {
                    "Free" => LicenseTier::Free,
                    "Pro" => LicenseTier::Pro,
                    "Team" => LicenseTier::Team,
                    "Business" => LicenseTier::Business,
                    "Enterprise" => LicenseTier::Enterprise,
                    _ => LicenseTier::Free,
                };
                
                let license = License::new(tier, email);
                Ok(Some(license))
            }
            None => Ok(None),
        }
    }

    async fn find_by_key(&self, key: &LicenseKey) -> Result<Option<License>> {
        let conn = self.conn.lock().unwrap();
        
        let result: Option<String> = conn
            .query_row(
                "SELECT data FROM licenses WHERE key = ?1",
                params![key.to_string()],
                |row| row.get(0),
            )
            .optional()?;

        match result {
            Some(data) => {
                let license: License = serde_json::from_str(&data)?;
                Ok(Some(license))
            }
            None => Ok(None),
        }
    }

    async fn find_by_user(&self, user_id: &str) -> Result<Option<License>> {
        let conn = self.conn.lock().unwrap();
        
        let result: Option<String> = conn
            .query_row(
                "SELECT data FROM licenses WHERE email = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;

        match result {
            Some(data) => {
                let license: License = serde_json::from_str(&data)?;
                Ok(Some(license))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, license: &License) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let data = serde_json::to_string(license)?;
        
        let rows_affected = conn.execute(
            "UPDATE licenses 
             SET data = ?1, key = ?2, tier = ?3, email = ?4, expires_at = ?5, is_active = ?6
             WHERE id = ?7",
            params![
                data,
                license.key().to_string(),
                license.tier().as_str(),
                license.email(),
                license.expires_at().map(|dt| dt.to_rfc3339()),
                if license.is_active() { 1 } else { 0 },
                license.id().to_string(),
            ],
        )?;

        if rows_affected == 0 {
            return Err(LicenseError::LicenseNotFound);
        }

        Ok(())
    }

    async fn delete(&self, key: &LicenseKey) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        let rows_affected = conn.execute(
            "DELETE FROM licenses WHERE key = ?1",
            params![key.to_string()],
        )?;

        if rows_affected == 0 {
            return Err(LicenseError::LicenseNotFound);
        }

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<License>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare("SELECT data FROM licenses")?;
        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut licenses = Vec::new();
        for row in rows {
            let data = row?;
            let license: License = serde_json::from_str(&data)?;
            licenses.push(license);
        }

        Ok(licenses)
    }

    async fn exists(&self, key: &LicenseKey) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM licenses WHERE key = ?1",
            params![key.to_string()],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::license::LicenseTier;

    #[tokio::test]
    async fn test_save_and_find() {
        let store = SqliteLicenseStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        
        let found = store.find_by_key(license.key()).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), license.id());
    }

    #[tokio::test]
    async fn test_update() {
        let store = SqliteLicenseStore::in_memory().unwrap();
        let mut license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        
        // Upgrade tier
        license.upgrade_tier(LicenseTier::Team).unwrap();
        store.update(&license).await.unwrap();
        
        let found = store.find_by_key(license.key()).await.unwrap().unwrap();
        assert_eq!(found.tier(), LicenseTier::Team);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = SqliteLicenseStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        store.delete(license.key()).await.unwrap();
        
        let found = store.find_by_key(license.key()).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_list_all() {
        let store = SqliteLicenseStore::in_memory().unwrap();
        
        let license1 = License::new(LicenseTier::Pro, Some("user1@example.com".to_string()));
        let license2 = License::new(LicenseTier::Team, Some("user2@example.com".to_string()));
        
        store.save(&license1).await.unwrap();
        store.save(&license2).await.unwrap();
        
        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_exists() {
        let store = SqliteLicenseStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        assert!(!store.exists(license.key()).await.unwrap());
        
        store.save(&license).await.unwrap();
        
        assert!(store.exists(license.key()).await.unwrap());
    }

    #[tokio::test]
    async fn test_find_by_user() {
        let store = SqliteLicenseStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        
        let found = store.find_by_user("test@example.com").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), license.id());
    }
}

// Made with Bob
