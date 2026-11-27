use crate::{MediaEdgeClient, Result, models::*};

pub struct TenantsApi {
    client: MediaEdgeClient,
}

impl TenantsApi {
    pub fn new(client: MediaEdgeClient) -> Self {
        Self { client }
    }

    /// List tenants
    pub async fn list(&self, params: Option<ListParams>) -> Result<PaginatedResponse<Tenant>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
            if let Some(search) = p.search { query.push(("search", search)); }
        }
        self.client.get_with_query("/api/v1/tenants", &query).await
    }

    /// Get a single tenant
    pub async fn get(&self, id: Uuid) -> Result<Tenant> {
        self.client.get(&format!("/api/v1/tenants/{}", id)).await
    }

    /// Create a new tenant
    pub async fn create(&self, request: CreateTenantRequest) -> Result<Tenant> {
        self.client.post("/api/v1/tenants", &request).await
    }

    /// Update a tenant
    pub async fn update(&self, id: Uuid, request: UpdateTenantRequest) -> Result<Tenant> {
        self.client.patch(&format!("/api/v1/tenants/{}", id), &request).await
    }

    /// Delete a tenant
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.client.delete(&format!("/api/v1/tenants/{}", id)).await
    }

    /// Get tenant quotas
    pub async fn quotas(&self, id: Uuid) -> Result<TenantQuotas> {
        self.client.get(&format!("/api/v1/tenants/{}/quotas", id)).await
    }

    /// Update tenant quotas
    pub async fn update_quotas(&self, id: Uuid, quotas: TenantQuotas) -> Result<TenantQuotas> {
        self.client.patch(&format!("/api/v1/tenants/{}/quotas", id), &quotas).await
    }

    /// Get tenant users
    pub async fn users(&self, id: Uuid, params: Option<ListParams>) -> Result<PaginatedResponse<User>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query(&format!("/api/v1/tenants/{}/users", id), &query).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTenantRequest {
    pub name: Option<String>,
    pub quotas: Option<TenantQuotas>,
    pub branding: Option<TenantBranding>,
    pub settings: Option<TenantSettings>,
}

use crate::models::{ListParams, PaginatedResponse};
use uuid::Uuid;
use serde::{Serialize, Deserialize};