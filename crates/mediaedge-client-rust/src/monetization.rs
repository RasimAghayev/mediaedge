use crate::{MediaEdgeClient, Result, models::*};

pub struct MonetizationApi {
    client: MediaEdgeClient,
}

impl MonetizationApi {
    pub fn new(client: MediaEdgeClient) -> Self {
        Self { client }
    }

    // FAST Channels
    pub async fn fast_channels(&self, params: Option<ListParams>) -> Result<PaginatedResponse<FastChannel>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/monetization/fast-channels", &query).await
    }

    pub async fn fast_channel(&self, id: Uuid) -> Result<FastChannel> {
        self.client.get(&format!("/api/v1/monetization/fast-channels/{}", id)).await
    }

    pub async fn create_fast_channel(&self, request: CreateFastChannelRequest) -> Result<FastChannel> {
        self.client.post("/api/v1/monetization/fast-channels", &request).await
    }

    pub async fn update_fast_channel(&self, id: Uuid, request: UpdateFastChannelRequest) -> Result<FastChannel> {
        self.client.patch(&format!("/api/v1/monetization/fast-channels/{}", id), &request).await
    }

    pub async fn delete_fast_channel(&self, id: Uuid) -> Result<()> {
        self.client.delete(&format!("/api/v1/monetization/fast-channels/{}", id)).await
    }

    // TVOD Products
    pub async fn tvod_products(&self, params: Option<ListParams>) -> Result<PaginatedResponse<TvodProduct>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/monetization/tvod/products", &query).await
    }

    pub async fn tvod_product(&self, id: Uuid) -> Result<TvodProduct> {
        self.client.get(&format!("/api/v1/monetization/tvod/products/{}", id)).await
    }

    pub async fn create_tvod_product(&self, request: CreateTvodProductRequest) -> Result<TvodProduct> {
        self.client.post("/api/v1/monetization/tvod/products", &request).await
    }

    pub async fn update_tvod_product(&self, id: Uuid, request: UpdateTvodProductRequest) -> Result<TvodProduct> {
        self.client.patch(&format!("/api/v1/monetization/tvod/products/{}", id), &request).await
    }

    pub async fn delete_tvod_product(&self, id: Uuid) -> Result<()> {
        self.client.delete(&format!("/api/v1/monetization/tvod/products/{}", id)).await
    }

    // Bundles
    pub async fn bundles(&self, params: Option<ListParams>) -> Result<PaginatedResponse<Bundle>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/monetization/bundles", &query).await
    }

    pub async fn bundle(&self, id: Uuid) -> Result<Bundle> {
        self.client.get(&format!("/api/v1/monetization/bundles/{}", id)).await
    }

    pub async fn create_bundle(&self, request: CreateBundleRequest) -> Result<Bundle> {
        self.client.post("/api/v1/monetization/bundles", &request).await
    }

    pub async fn update_bundle(&self, id: Uuid, request: UpdateBundleRequest) -> Result<Bundle> {
        self.client.patch(&format!("/api/v1/monetization/bundles/{}", id), &request).await
    }

    pub async fn delete_bundle(&self, id: Uuid) -> Result<()> {
        self.client.delete(&format!("/api/v1/monetization/bundles/{}", id)).await
    }

    // Entitlements
    pub async fn check_entitlement(&self, request: EntitlementCheck) -> Result<EntitlementResult> {
        self.client.post("/api/v1/monetization/entitlements/check", &request).await
    }

    pub async fn list_entitlements(&self, user_id: Uuid, params: Option<ListParams>) -> Result<PaginatedResponse<Entitlement>> {
        let mut query = Vec::new();
        query.push(("user_id", user_id.to_string()));
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/monetization/entitlements", &query).await
    }

    // Billing
    pub async fn invoices(&self, params: Option<ListParams>) -> Result<PaginatedResponse<Invoice>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/monetization/billing/invoices", &query).await
    }

    pub async fn invoice(&self, id: Uuid) -> Result<Invoice> {
        self.client.get(&format!("/api/v1/monetization/billing/invoices/{}", id)).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFastChannelRequest {
    pub name: String,
    pub slug: Option<String>,
    pub schedule: FastChannelSchedule,
    pub ad_breaks: AdBreakConfig,
    pub output_formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFastChannelRequest {
    pub name: Option<String>,
    pub schedule: Option<FastChannelSchedule>,
    pub ad_breaks: Option<AdBreakConfig>,
    pub output_formats: Option<Vec<String>>,
    pub status: Option<FastChannelStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTvodProductRequest {
    pub title: String,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    pub trailer_url: Option<String>,
    pub content_id: Uuid,
    pub rental_price_cents: Option<u32>,
    pub purchase_price_cents: Option<u32>,
    pub rental_duration_hours: Option<u32>,
    pub purchase_availability: Option<AvailabilityWindow>,
    pub rental_availability: Option<AvailabilityWindow>,
    pub territories: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTvodProductRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    pub rental_price_cents: Option<u32>,
    pub purchase_price_cents: Option<u32>,
    pub status: Option<TvodStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBundleRequest {
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: BundleType,
    pub pricing: BundlePricing,
    pub included_products: Vec<BundleProduct>,
    pub partner_validation: Option<PartnerValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBundleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub pricing: Option<BundlePricing>,
    pub included_products: Option<Vec<BundleProduct>>,
    pub status: Option<BundleStatus>,
}

use crate::models::{ListParams, PaginatedResponse};
use uuid::Uuid;
use serde::{Serialize, Deserialize};