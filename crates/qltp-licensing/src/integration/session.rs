//! Enhanced session management
//!
//! Combines authentication tokens with license and quota information

use crate::domain::license::{FeatureFlags, License, LicenseTier};
use crate::domain::usage::Quota;
use crate::integration::anonymous::AnonymousId;
use crate::integration::user::UserId;
use chrono::{DateTime, Utc};
use qltp_auth::AuthToken;
use serde::{Deserialize, Serialize};

/// Session type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionType {
    /// Anonymous session
    Anonymous(AnonymousId),
    /// Registered user session
    Registered(UserId),
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session type
    session_type: SessionType,
    /// Authentication token (if registered)
    auth_token: Option<AuthToken>,
    /// License tier
    tier: LicenseTier,
    /// Feature flags
    features: FeatureFlags,
    /// Current quota usage
    quota_used: u64,
    /// Quota limit (None for unlimited)
    quota_limit: Option<u64>,
    /// Session creation time
    created_at: DateTime<Utc>,
    /// Last activity time
    last_activity: DateTime<Utc>,
}

impl SessionInfo {
    /// Create anonymous session
    pub fn anonymous(anonymous_id: AnonymousId) -> Self {
        Self {
            session_type: SessionType::Anonymous(anonymous_id),
            auth_token: None,
            tier: LicenseTier::Free,
            features: FeatureFlags::for_tier(LicenseTier::Free),
            quota_used: 0,
            quota_limit: Some(Quota::for_tier(LicenseTier::Free).monthly_bytes()),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        }
    }

    /// Create registered user session
    pub fn registered(user_id: UserId, token: AuthToken, license: &License) -> Self {
        let quota = Quota::for_tier(license.tier());
        Self {
            session_type: SessionType::Registered(user_id),
            auth_token: Some(token),
            tier: license.tier(),
            features: license.features().clone(),
            quota_used: 0,
            quota_limit: if quota.monthly_bytes() == u64::MAX {
                None
            } else {
                Some(quota.monthly_bytes())
            },
            created_at: Utc::now(),
            last_activity: Utc::now(),
        }
    }

    /// Get session type
    pub fn session_type(&self) -> &SessionType {
        &self.session_type
    }

    /// Check if session is anonymous
    pub fn is_anonymous(&self) -> bool {
        matches!(self.session_type, SessionType::Anonymous(_))
    }

    /// Check if session is registered
    pub fn is_registered(&self) -> bool {
        matches!(self.session_type, SessionType::Registered(_))
    }

    /// Get auth token
    pub fn auth_token(&self) -> Option<&AuthToken> {
        self.auth_token.as_ref()
    }

    /// Get license tier
    pub fn tier(&self) -> LicenseTier {
        self.tier
    }

    /// Get feature flags
    pub fn features(&self) -> &FeatureFlags {
        &self.features
    }

    /// Get quota used
    pub fn quota_used(&self) -> u64 {
        self.quota_used
    }

    /// Get quota limit
    pub fn quota_limit(&self) -> Option<u64> {
        self.quota_limit
    }

    /// Get remaining quota
    pub fn quota_remaining(&self) -> Option<u64> {
        self.quota_limit.map(|limit| limit.saturating_sub(self.quota_used))
    }

    /// Check if quota is exceeded
    pub fn is_quota_exceeded(&self) -> bool {
        if let Some(limit) = self.quota_limit {
            self.quota_used >= limit
        } else {
            false // Unlimited
        }
    }

    /// Update quota usage
    pub fn update_quota_used(&mut self, bytes: u64) {
        self.quota_used = bytes;
        self.last_activity = Utc::now();
    }

    /// Add to quota usage
    pub fn add_quota_usage(&mut self, bytes: u64) {
        self.quota_used = self.quota_used.saturating_add(bytes);
        self.last_activity = Utc::now();
    }

    /// Update last activity
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }

    /// Get creation time
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get last activity time
    pub fn last_activity(&self) -> DateTime<Utc> {
        self.last_activity
    }

    /// Check if session is expired (24 hours of inactivity)
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_activity);
        duration.num_hours() > 24
    }
}

/// Enhanced session combining auth and licensing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSession {
    /// Session information
    info: SessionInfo,
    /// Associated license (if any)
    license: Option<License>,
}

impl EnhancedSession {
    /// Create anonymous session
    pub fn anonymous() -> Self {
        let anonymous_id = AnonymousId::generate();
        Self {
            info: SessionInfo::anonymous(anonymous_id),
            license: None,
        }
    }

    /// Create registered session
    pub fn registered(user_id: UserId, token: AuthToken, license: License) -> Self {
        let info = SessionInfo::registered(user_id, token, &license);
        Self {
            info,
            license: Some(license),
        }
    }

    /// Get session info
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    /// Get mutable session info
    pub fn info_mut(&mut self) -> &mut SessionInfo {
        &mut self.info
    }

    /// Get license
    pub fn license(&self) -> Option<&License> {
        self.license.as_ref()
    }

    /// Set license
    pub fn set_license(&mut self, license: License) {
        let quota = Quota::for_tier(license.tier());
        self.info.tier = license.tier();
        self.info.features = license.features().clone();
        self.info.quota_limit = if quota.monthly_bytes() == u64::MAX {
            None
        } else {
            Some(quota.monthly_bytes())
        };
        self.license = Some(license);
    }

    /// Upgrade from anonymous to registered
    pub fn upgrade_to_registered(&mut self, user_id: UserId, token: AuthToken, license: License) {
        self.info.session_type = SessionType::Registered(user_id);
        self.info.auth_token = Some(token);
        self.set_license(license);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anonymous_session() {
        let session = EnhancedSession::anonymous();
        assert!(session.info().is_anonymous());
        assert!(!session.info().is_registered());
        assert_eq!(session.info().tier(), LicenseTier::Free);
        assert!(session.license().is_none());
    }

    #[test]
    fn test_registered_session() {
        let user_id = UserId::new();
        let token = AuthToken::new();
        let license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        let session = EnhancedSession::registered(user_id, token, license);
        assert!(session.info().is_registered());
        assert!(!session.info().is_anonymous());
        assert_eq!(session.info().tier(), LicenseTier::Pro);
        assert!(session.license().is_some());
    }

    #[test]
    fn test_quota_tracking() {
        let mut info = SessionInfo::anonymous(AnonymousId::generate());
        assert_eq!(info.quota_used(), 0);
        
        info.add_quota_usage(1024);
        assert_eq!(info.quota_used(), 1024);
        
        info.update_quota_used(2048);
        assert_eq!(info.quota_used(), 2048);
    }

    #[test]
    fn test_quota_exceeded() {
        let mut info = SessionInfo::anonymous(AnonymousId::generate());
        assert!(!info.is_quota_exceeded());
        
        // Set quota to limit
        if let Some(limit) = info.quota_limit() {
            info.update_quota_used(limit);
            assert!(info.is_quota_exceeded());
        }
    }

    #[test]
    fn test_session_upgrade() {
        let mut session = EnhancedSession::anonymous();
        assert!(session.info().is_anonymous());
        
        let user_id = UserId::new();
        let token = AuthToken::new();
        let license = License::new(LicenseTier::Pro, Some("test@example.com".to_string()));
        
        session.upgrade_to_registered(user_id, token, license);
        assert!(session.info().is_registered());
        assert_eq!(session.info().tier(), LicenseTier::Pro);
    }

    #[test]
    fn test_quota_remaining() {
        let info = SessionInfo::anonymous(AnonymousId::generate());
        if let Some(limit) = info.quota_limit() {
            assert_eq!(info.quota_remaining(), Some(limit));
        }
    }
}

// Made with Bob
