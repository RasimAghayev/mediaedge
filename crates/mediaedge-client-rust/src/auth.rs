use crate::{MediaEdgeClient, Result, models::*};

pub struct AuthApi {
    client: MediaEdgeClient,
}

impl AuthApi {
    pub fn new(client: MediaEdgeClient) -> Self {
        Self { client }
    }

    /// Login with email/password
    pub async fn login(&self, request: LoginRequest) -> Result<LoginResponse> {
        self.client.post("/api/v1/auth/login", &request).await
    }

    /// Refresh access token
    pub async fn refresh(&self, refresh_token: &str) -> Result<LoginResponse> {
        #[derive(Serialize)]
        struct RefreshRequest { refresh_token: String }
        self.client.post("/api/v1/auth/refresh", &RefreshRequest { refresh_token: refresh_token.to_string() }).await
    }

    /// Logout (revoke tokens)
    pub async fn logout(&self) -> Result<()> {
        self.client.post("/api/v1/auth/logout", &()).await
    }

    /// Get current user
    pub async fn me(&self) -> Result<User> {
        self.client.get("/api/v1/auth/me").await
    }

    /// Update current user
    pub async fn update_me(&self, request: UpdateUserRequest) -> Result<User> {
        self.client.patch("/api/v1/auth/me", &request).await
    }

    /// Change password
    pub async fn change_password(&self, request: ChangePasswordRequest) -> Result<()> {
        self.client.post("/api/v1/auth/password", &request).await
    }

    /// Setup TOTP
    pub async fn setup_totp(&self) -> Result<TotpSetupResponse> {
        self.client.post("/api/v1/auth/mfa/totp/setup", &()).await
    }

    /// Verify TOTP
    pub async fn verify_totp(&self, code: &str) -> Result<()> {
        #[derive(Serialize)]
        struct VerifyRequest { code: String }
        self.client.post("/api/v1/auth/mfa/totp/verify", &VerifyRequest { code: code.to_string() }).await
    }

    /// Setup WebAuthn
    pub async fn setup_webauthn(&self) -> Result<WebAuthnSetupResponse> {
        self.client.post("/api/v1/auth/mfa/webauthn/setup", &()).await
    }

    /// List users (admin)
    pub async fn users(&self, params: Option<ListParams>) -> Result<PaginatedResponse<User>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
            if let Some(search) = p.search { query.push(("search", search)); }
        }
        self.client.get_with_query("/api/v1/users", &query).await
    }

    /// Invite user (admin)
    pub async fn invite_user(&self, request: InviteUserRequest) -> Result<User> {
        self.client.post("/api/v1/users/invite", &request).await
    }

    /// Get user (admin)
    pub async fn user(&self, id: Uuid) -> Result<User> {
        self.client.get(&format!("/api/v1/users/{}", id)).await
    }

    /// Update user (admin)
    pub async fn update_user(&self, id: Uuid, request: UpdateUserRequest) -> Result<User> {
        self.client.patch(&format!("/api/v1/users/{}", id), &request).await
    }

    /// Delete user (admin)
    pub async fn delete_user(&self, id: Uuid) -> Result<()> {
        self.client.delete(&format!("/api/v1/users/{}", id)).await
    }

    /// Start OIDC login
    pub async fn oidc_start(&self, provider: &str, return_url: &str) -> Result<OidcStartResponse> {
        #[derive(Serialize)]
        struct OidcRequest { provider: String, return_url: String }
        self.client.post("/api/v1/auth/oidc/start", &OidcRequest { provider: provider.to_string(), return_url: return_url.to_string() }).await
    }

    /// OIDC callback
    pub async fn oidc_callback(&self, provider: &str, code: &str, state: &str) -> Result<LoginResponse> {
        #[derive(Serialize)]
        struct OidcCallback { code: String, state: String }
        self.client.post(&format!("/api/v1/auth/oidc/{}/callback", provider), &OidcCallback { code: code.to_string(), state: state.to_string() }).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub roles: Option<Vec<String>>,
    pub status: Option<UserStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_code_url: String,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnSetupResponse {
    pub challenge: String,
    pub rp_id: String,
    pub user_id: String,
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcStartResponse {
    pub auth_url: String,
    pub state: String,
}

use crate::models::{ListParams, PaginatedResponse};
use uuid::Uuid;
use serde::{Serialize, Deserialize};