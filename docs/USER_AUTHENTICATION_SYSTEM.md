# QLTP User Authentication & Account Management System

## Executive Summary

This document outlines a comprehensive **user authentication and account management system** that supports both anonymous usage and registered accounts, enabling seamless transitions between modes while maintaining user data and preferences.

**Key Features**:
- Anonymous usage (no account required)
- Email/password registration
- Social login (Google, GitHub, Apple)
- Account linking (anonymous → registered)
- Multi-device sync
- Single Sign-On (SSO) for enterprise

---

## User Journey Flows

### Flow 1: Anonymous User (No Account)

```
User Downloads App
    ↓
First Launch
    ↓
Anonymous ID Generated (UUID)
    ↓
Local Usage Tracking Starts
    ↓
User Transfers Files (Free Tier)
    ↓
Usage Stored Locally
    ↓
[Optional] User Creates Account Later
```

### Flow 2: New User Registration

```
User Downloads App
    ↓
First Launch
    ↓
"Sign Up" or "Continue as Guest"
    ↓
User Chooses "Sign Up"
    ↓
Registration Form
    ├── Email + Password
    ├── Google Sign-In
    ├── GitHub Sign-In
    └── Apple Sign-In
    ↓
Email Verification (if email/password)
    ↓
Account Created
    ↓
User Profile Setup
    ↓
License Tier Selected (Free/Pro/Team)
    ↓
Usage Synced to Cloud
    ↓
Multi-Device Access Enabled
```

### Flow 3: Existing User Login

```
User Downloads App (New Device)
    ↓
First Launch
    ↓
"Log In" or "Continue as Guest"
    ↓
User Chooses "Log In"
    ↓
Login Form
    ├── Email + Password
    ├── Google Sign-In
    ├── GitHub Sign-In
    └── Apple Sign-In
    ↓
Authentication
    ↓
Sync User Data from Cloud
    ├── License Information
    ├── Usage History
    ├── Preferences
    └── Transfer History
    ↓
Device Registered
    ↓
Ready to Use
```

### Flow 4: Anonymous to Registered (Account Linking)

```
Anonymous User Using App
    ↓
Reaches Free Tier Limit
    ↓
Upgrade Prompt Shown
    ↓
"Create Account to Continue"
    ↓
Registration Form
    ↓
Account Created
    ↓
Anonymous Data Migrated
    ├── Usage History
    ├── Preferences
    ├── Transfer History
    └── Local Files
    ↓
Account Linked
    ↓
Cloud Sync Enabled
    ↓
Multi-Device Access Available
```

---

## Authentication Methods

### 1. Email/Password Authentication

**Registration**:
```rust
pub struct RegistrationRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub anonymous_id: Option<String>, // For account linking
}

pub async fn register_user(req: RegistrationRequest) -> Result<User> {
    // Validate email
    if !is_valid_email(&req.email) {
        return Err(Error::InvalidEmail);
    }
    
    // Validate password strength
    if !is_strong_password(&req.password) {
        return Err(Error::WeakPassword);
    }
    
    // Check if email already exists
    if user_exists(&req.email).await? {
        return Err(Error::EmailAlreadyExists);
    }
    
    // Hash password
    let password_hash = hash_password(&req.password)?;
    
    // Create user
    let user = User {
        id: Uuid::new_v4(),
        email: req.email.clone(),
        password_hash,
        name: req.name,
        email_verified: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    // Save to database
    save_user(&user).await?;
    
    // Send verification email
    send_verification_email(&user).await?;
    
    // Link anonymous data if provided
    if let Some(anonymous_id) = req.anonymous_id {
        link_anonymous_data(&user.id, &anonymous_id).await?;
    }
    
    Ok(user)
}
```

**Login**:
```rust
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub device_id: String,
    pub device_name: String,
}

pub async fn login_user(req: LoginRequest) -> Result<LoginResponse> {
    // Find user by email
    let user = find_user_by_email(&req.email).await?
        .ok_or(Error::InvalidCredentials)?;
    
    // Verify password
    if !verify_password(&req.password, &user.password_hash)? {
        return Err(Error::InvalidCredentials);
    }
    
    // Check if email is verified
    if !user.email_verified {
        return Err(Error::EmailNotVerified);
    }
    
    // Generate session token (JWT)
    let token = generate_jwt_token(&user)?;
    
    // Register device
    register_device(&user.id, &req.device_id, &req.device_name).await?;
    
    // Load user data
    let license = load_user_license(&user.id).await?;
    let preferences = load_user_preferences(&user.id).await?;
    
    Ok(LoginResponse {
        user,
        token,
        license,
        preferences,
    })
}
```

**Password Reset**:
```rust
pub async fn request_password_reset(email: String) -> Result<()> {
    // Find user
    let user = find_user_by_email(&email).await?
        .ok_or(Error::UserNotFound)?;
    
    // Generate reset token
    let reset_token = generate_reset_token();
    let expires_at = Utc::now() + Duration::hours(24);
    
    // Save reset token
    save_reset_token(&user.id, &reset_token, expires_at).await?;
    
    // Send reset email
    send_password_reset_email(&user, &reset_token).await?;
    
    Ok(())
}

pub async fn reset_password(token: String, new_password: String) -> Result<()> {
    // Validate token
    let user_id = validate_reset_token(&token).await?;
    
    // Validate new password
    if !is_strong_password(&new_password) {
        return Err(Error::WeakPassword);
    }
    
    // Hash new password
    let password_hash = hash_password(&new_password)?;
    
    // Update password
    update_user_password(&user_id, &password_hash).await?;
    
    // Invalidate reset token
    invalidate_reset_token(&token).await?;
    
    // Revoke all sessions (force re-login)
    revoke_all_user_sessions(&user_id).await?;
    
    Ok(())
}
```

### 2. Social Login (OAuth)

**Google Sign-In**:
```rust
pub async fn google_signin(
    id_token: String,
    anonymous_id: Option<String>,
) -> Result<LoginResponse> {
    // Verify Google ID token
    let google_user = verify_google_token(&id_token).await?;
    
    // Check if user exists
    let user = if let Some(existing_user) = find_user_by_email(&google_user.email).await? {
        existing_user
    } else {
        // Create new user
        let new_user = User {
            id: Uuid::new_v4(),
            email: google_user.email.clone(),
            password_hash: String::new(), // No password for OAuth users
            name: Some(google_user.name),
            email_verified: true, // Google already verified
            oauth_provider: Some("google".to_string()),
            oauth_id: Some(google_user.sub),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        save_user(&new_user).await?;
        
        // Link anonymous data if provided
        if let Some(anonymous_id) = anonymous_id {
            link_anonymous_data(&new_user.id, &anonymous_id).await?;
        }
        
        new_user
    };
    
    // Generate session token
    let token = generate_jwt_token(&user)?;
    
    // Load user data
    let license = load_user_license(&user.id).await?;
    let preferences = load_user_preferences(&user.id).await?;
    
    Ok(LoginResponse {
        user,
        token,
        license,
        preferences,
    })
}
```

**GitHub Sign-In**:
```rust
pub async fn github_signin(
    code: String,
    anonymous_id: Option<String>,
) -> Result<LoginResponse> {
    // Exchange code for access token
    let access_token = exchange_github_code(&code).await?;
    
    // Get user info from GitHub
    let github_user = get_github_user(&access_token).await?;
    
    // Check if user exists
    let user = if let Some(existing_user) = find_user_by_github_id(&github_user.id).await? {
        existing_user
    } else {
        // Create new user
        let new_user = User {
            id: Uuid::new_v4(),
            email: github_user.email.clone(),
            password_hash: String::new(),
            name: Some(github_user.name),
            email_verified: true,
            oauth_provider: Some("github".to_string()),
            oauth_id: Some(github_user.id.to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        save_user(&new_user).await?;
        
        // Link anonymous data
        if let Some(anonymous_id) = anonymous_id {
            link_anonymous_data(&new_user.id, &anonymous_id).await?;
        }
        
        new_user
    };
    
    // Generate session token
    let token = generate_jwt_token(&user)?;
    
    // Load user data
    let license = load_user_license(&user.id).await?;
    let preferences = load_user_preferences(&user.id).await?;
    
    Ok(LoginResponse {
        user,
        token,
        license,
        preferences,
    })
}
```

**Apple Sign-In**:
```rust
pub async fn apple_signin(
    id_token: String,
    authorization_code: String,
    anonymous_id: Option<String>,
) -> Result<LoginResponse> {
    // Verify Apple ID token
    let apple_user = verify_apple_token(&id_token).await?;
    
    // Similar implementation to Google/GitHub
    // ...
    
    Ok(LoginResponse {
        user,
        token,
        license,
        preferences,
    })
}
```

### 3. Enterprise SSO (SAML/OAuth)

**SAML Authentication**:
```rust
pub async fn saml_signin(
    saml_response: String,
    organization_id: String,
) -> Result<LoginResponse> {
    // Validate SAML response
    let saml_user = validate_saml_response(&saml_response, &organization_id).await?;
    
    // Find or create user
    let user = find_or_create_sso_user(&saml_user, &organization_id).await?;
    
    // Generate session token
    let token = generate_jwt_token(&user)?;
    
    // Load enterprise license
    let license = load_organization_license(&organization_id).await?;
    
    Ok(LoginResponse {
        user,
        token,
        license,
        preferences: load_user_preferences(&user.id).await?,
    })
}
```

---

## Account Linking (Anonymous → Registered)

### Migration Strategy

```rust
pub async fn link_anonymous_account(
    user_id: Uuid,
    anonymous_id: String,
) -> Result<()> {
    // Load anonymous data
    let anonymous_data = load_anonymous_data(&anonymous_id).await?;
    
    // Migrate usage history
    migrate_usage_history(&user_id, &anonymous_data.usage_history).await?;
    
    // Migrate preferences
    migrate_preferences(&user_id, &anonymous_data.preferences).await?;
    
    // Migrate transfer history
    migrate_transfer_history(&user_id, &anonymous_data.transfer_history).await?;
    
    // Migrate local files (if any)
    migrate_local_files(&user_id, &anonymous_data.local_files).await?;
    
    // Mark anonymous account as linked
    mark_anonymous_account_linked(&anonymous_id, &user_id).await?;
    
    // Delete anonymous data after successful migration
    delete_anonymous_data(&anonymous_id).await?;
    
    Ok(())
}
```

### Data Migration

```rust
pub struct AnonymousData {
    pub anonymous_id: String,
    pub usage_history: UsageHistory,
    pub preferences: UserPreferences,
    pub transfer_history: Vec<TransferRecord>,
    pub local_files: Vec<LocalFile>,
}

pub struct UsageHistory {
    pub total_transfers: u64,
    pub bytes_transferred: u64,
    pub features_used: Vec<String>,
    pub first_use: DateTime<Utc>,
    pub last_use: DateTime<Utc>,
}

async fn migrate_usage_history(
    user_id: &Uuid,
    usage: &UsageHistory,
) -> Result<()> {
    // Merge with existing usage (if any)
    let existing_usage = load_user_usage(user_id).await?;
    
    let merged_usage = UsageHistory {
        total_transfers: existing_usage.total_transfers + usage.total_transfers,
        bytes_transferred: existing_usage.bytes_transferred + usage.bytes_transferred,
        features_used: merge_features(&existing_usage.features_used, &usage.features_used),
        first_use: std::cmp::min(existing_usage.first_use, usage.first_use),
        last_use: std::cmp::max(existing_usage.last_use, usage.last_use),
    };
    
    save_user_usage(user_id, &merged_usage).await?;
    
    Ok(())
}
```

---

## Session Management

### JWT Token Structure

```rust
pub struct JwtClaims {
    pub sub: String,        // User ID
    pub email: String,      // User email
    pub tier: String,       // License tier
    pub exp: i64,           // Expiration timestamp
    pub iat: i64,           // Issued at timestamp
    pub device_id: String,  // Device ID
}

pub fn generate_jwt_token(user: &User) -> Result<String> {
    let claims = JwtClaims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        tier: user.license_tier.to_string(),
        exp: (Utc::now() + Duration::days(30)).timestamp(),
        iat: Utc::now().timestamp(),
        device_id: get_device_id()?,
    };
    
    let secret = std::env::var("JWT_SECRET")?;
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?;
    
    Ok(token)
}

pub fn verify_jwt_token(token: &str) -> Result<JwtClaims> {
    let secret = std::env::var("JWT_SECRET")?;
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;
    
    Ok(token_data.claims)
}
```

### Session Storage

```rust
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub device_name: String,
    pub token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

pub async fn create_session(
    user_id: Uuid,
    device_id: String,
    device_name: String,
) -> Result<Session> {
    let session = Session {
        id: Uuid::new_v4(),
        user_id,
        device_id,
        device_name,
        token: generate_jwt_token(&user)?,
        created_at: Utc::now(),
        expires_at: Utc::now() + Duration::days(30),
        last_activity: Utc::now(),
    };
    
    // Save to database
    save_session(&session).await?;
    
    // Save to Redis for fast lookup
    cache_session(&session).await?;
    
    Ok(session)
}

pub async fn validate_session(token: &str) -> Result<Session> {
    // Verify JWT
    let claims = verify_jwt_token(token)?;
    
    // Check if session exists in cache
    if let Some(session) = get_cached_session(token).await? {
        // Update last activity
        update_session_activity(&session.id).await?;
        return Ok(session);
    }
    
    // Load from database
    let session = load_session_by_token(token).await?
        .ok_or(Error::InvalidSession)?;
    
    // Check expiration
    if session.expires_at < Utc::now() {
        return Err(Error::SessionExpired);
    }
    
    // Cache for future requests
    cache_session(&session).await?;
    
    Ok(session)
}
```

---

## Multi-Device Sync

### Device Registration

```rust
pub struct Device {
    pub id: String,
    pub user_id: Uuid,
    pub name: String,
    pub platform: String,      // "macos", "windows", "linux", "ios", "android"
    pub app_version: String,
    pub registered_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

pub async fn register_device(
    user_id: &Uuid,
    device_id: &str,
    device_name: &str,
) -> Result<Device> {
    // Check device limit based on license tier
    let license = load_user_license(user_id).await?;
    let device_count = count_user_devices(user_id).await?;
    
    if device_count >= license.max_devices {
        return Err(Error::DeviceLimitReached {
            max_devices: license.max_devices,
        });
    }
    
    // Register or update device
    let device = Device {
        id: device_id.to_string(),
        user_id: *user_id,
        name: device_name.to_string(),
        platform: get_platform(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        registered_at: Utc::now(),
        last_seen: Utc::now(),
    };
    
    save_device(&device).await?;
    
    Ok(device)
}
```

### Data Synchronization

```rust
pub struct SyncManager {
    user_id: Uuid,
    device_id: String,
    last_sync: DateTime<Utc>,
}

impl SyncManager {
    pub async fn sync_to_cloud(&self) -> Result<()> {
        // Sync usage data
        let usage = load_local_usage(&self.device_id).await?;
        upload_usage(&self.user_id, &usage).await?;
        
        // Sync preferences
        let preferences = load_local_preferences(&self.device_id).await?;
        upload_preferences(&self.user_id, &preferences).await?;
        
        // Sync transfer history
        let history = load_local_transfer_history(&self.device_id).await?;
        upload_transfer_history(&self.user_id, &history).await?;
        
        // Update last sync time
        self.update_last_sync().await?;
        
        Ok(())
    }
    
    pub async fn sync_from_cloud(&self) -> Result<()> {
        // Download usage data
        let usage = download_usage(&self.user_id).await?;
        save_local_usage(&self.device_id, &usage).await?;
        
        // Download preferences
        let preferences = download_preferences(&self.user_id).await?;
        save_local_preferences(&self.device_id, &preferences).await?;
        
        // Download transfer history
        let history = download_transfer_history(&self.user_id).await?;
        save_local_transfer_history(&self.device_id, &history).await?;
        
        // Update last sync time
        self.update_last_sync().await?;
        
        Ok(())
    }
    
    pub async fn sync_bidirectional(&self) -> Result<()> {
        // Merge local and cloud data
        let local_data = load_local_data(&self.device_id).await?;
        let cloud_data = download_data(&self.user_id).await?;
        
        let merged_data = merge_data(&local_data, &cloud_data, self.last_sync)?;
        
        // Upload merged data
        upload_data(&self.user_id, &merged_data).await?;
        
        // Save locally
        save_local_data(&self.device_id, &merged_data).await?;
        
        Ok(())
    }
}
```

---

## UI/UX Implementation

### Login Screen

```rust
pub fn render_login_screen() -> Html {
    html! {
        <div class="login-container">
            <h1>{"Welcome to QLTP"}</h1>
            
            <div class="login-options">
                // Social login buttons
                <button onclick={google_signin} class="btn-google">
                    <img src="/icons/google.svg" />
                    {"Continue with Google"}
                </button>
                
                <button onclick={github_signin} class="btn-github">
                    <img src="/icons/github.svg" />
                    {"Continue with GitHub"}
                </button>
                
                <button onclick={apple_signin} class="btn-apple">
                    <img src="/icons/apple.svg" />
                    {"Continue with Apple"}
                </button>
                
                <div class="divider">{"or"}</div>
                
                // Email/password form
                <form onsubmit={handle_login}>
                    <input
                        type="email"
                        placeholder="Email"
                        value={email}
                        oninput={update_email}
                    />
                    <input
                        type="password"
                        placeholder="Password"
                        value={password}
                        oninput={update_password}
                    />
                    <button type="submit" class="btn-primary">
                        {"Log In"}
                    </button>
                </form>
                
                <div class="links">
                    <a href="/forgot-password">{"Forgot password?"}</a>
                    <a href="/signup">{"Create account"}</a>
                </div>
                
                <div class="divider">{"or"}</div>
                
                // Anonymous option
                <button onclick={continue_as_guest} class="btn-secondary">
                    {"Continue as Guest"}
                </button>
            </div>
        </div>
    }
}
```

### Account Linking Prompt

```rust
pub fn render_account_linking_prompt() -> Html {
    html! {
        <div class="modal">
            <div class="modal-content">
                <h2>{"Create Account to Continue"}</h2>
                <p>
                    {"You've reached your free tier limit. "}
                    {"Create an account to:"}
                </p>
                <ul>
                    <li>{"Keep your usage history"}</li>
                    <li>{"Sync across devices"}</li>
                    <li>{"Upgrade to Pro for unlimited transfers"}</li>
                </ul>
                
                <div class="actions">
                    <button onclick={create_account} class="btn-primary">
                        {"Create Account"}
                    </button>
                    <button onclick={login_existing} class="btn-secondary">
                        {"I Have an Account"}
                    </button>
                </div>
                
                <p class="note">
                    {"Your data will be preserved when you create an account."}
                </p>
            </div>
        </div>
    }
}
```

---

## Complete User Flow Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    App Launch                            │
└─────────────────────────────────────────────────────────┘
                          ↓
                    Check Auth State
                          ↓
        ┌─────────────────┴─────────────────┐
        ↓                                    ↓
   No Account                          Has Account
   (Anonymous)                         (Registered)
        ↓                                    ↓
Generate Anonymous ID                  Show Login Screen
        ↓                                    ↓
Start Local Tracking              ┌──────────┴──────────┐
        ↓                         ↓                      ↓
Use Free Tier              Email/Password        Social Login
        ↓                         ↓                      ↓
Track Usage Locally          Authenticate          OAuth Flow
        ↓                         ↓                      ↓
Reach Limit                  Load User Data       Load User Data
        ↓                         ↓                      ↓
Show Upgrade Prompt          Sync from Cloud      Sync from Cloud
        ↓                         ↓                      ↓
"Create Account"             Register Device      Register Device
        ↓                         ↓                      ↓
Registration Form            Ready to Use         Ready to Use
        ↓                         ↓                      ↓
Link Anonymous Data          Multi-Device Sync    Multi-Device Sync
        ↓                         ↓                      ↓
Account Created              Cloud Backup         Cloud Backup
        ↓                         ↓                      ↓
Sync to Cloud                Periodic Sync        Periodic Sync
        ↓                         
Multi-Device Access          
```

---

## API Endpoints Summary

```http
# Registration
POST /api/v1/auth/register
POST /api/v1/auth/verify-email

# Login
POST /api/v1/auth/login
POST /api/v1/auth/google
POST /api/v1/auth/github
POST /api/v1/auth/apple
POST /api/v1/auth/saml

# Password Management
POST /api/v1/auth/forgot-password
POST /api/v1/auth/reset-password
POST /api/v1/auth/change-password

# Session Management
POST /api/v1/auth/logout
POST /api/v1/auth/refresh-token
GET  /api/v1/auth/sessions
DELETE /api/v1/auth/sessions/{id}

# Account Linking
POST /api/v1/auth/link-anonymous
POST /api/v1/auth/migrate-data

# Device Management
GET  /api/v1/devices
POST /api/v1/devices/register
DELETE /api/v1/devices/{id}

# Data Sync
POST /api/v1/sync/upload
GET  /api/v1/sync/download
POST /api/v1/sync/bidirectional
```

---

## Summary

**Complete authentication system** that supports:

✅ Anonymous usage (no friction)
✅ Email/password registration
✅ Social login (Google, GitHub, Apple)
✅ Account linking (anonymous → registered)
✅ Multi-device sync
✅ Enterprise SSO
✅ Session management
✅ Password reset
✅ Device management
✅ Data migration

**User experience optimized for**:
- Zero friction for new users (anonymous)
- Easy account creation when needed
- Seamless data migration
- Multi-device access
- Enterprise integration

**Ready to implement!** 🚀