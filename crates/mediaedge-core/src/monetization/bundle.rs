//! Bundle Support
//!
//! Streamer-to-streamer, telco, and aggregator bundles.
//! Partner subscription validation.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub bundle_type: BundleType,
    pub partner_id: Option<Uuid>,
    pub partner_bundle_id: Option<String>,
    pub included_content: Vec<BundleContent>,
    pub included_channels: Vec<Uuid>,
    pub pricing: BundlePricing,
    pub availability: BundleAvailability,
    pub status: BundleStatus,
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Bundle type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleType {
    StreamerBundle,    // Streamer-to-streamer
    TelcoBundle,       // Telco partnership
    AggregatorBundle,  // Aggregator (Amazon Channels, Roku, etc.)
    CorporateBundle,   // Enterprise/education
    PromotionalBundle, // Marketing bundle
}

/// Bundle content item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleContent {
    pub content_id: Uuid,
    pub content_type: BundleContentType,
    pub access_level: BundleAccessLevel,
    pub restrictions: BundleRestrictions,
}

/// Bundle content type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleContentType {
    Vod,
    LiveStream,
    FastChannel,
    Feature, // Platform feature
}

/// Bundle access level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleAccessLevel {
    Full,           // Full access
    Limited,        // Limited episodes/seasons
    PreviewOnly,    // Trailer/preview only
    AdSupported,    // With ads
}

/// Bundle restrictions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRestrictions {
    pub max_concurrent_streams: Option<u32>,
    pub max_devices: Option<u32>,
    pub download_enabled: bool,
    pub hd_enabled: bool,
    pub uhd_enabled: bool,
    pub territories: Vec<String>,
    pub platforms: Vec<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

impl Default for BundleRestrictions {
    fn default() -> Self {
        Self {
            max_concurrent_streams: None,
            max_devices: None,
            download_enabled: true,
            hd_enabled: true,
            uhd_enabled: false,
            territories: Vec::new(),
            platforms: Vec::new(),
            start_date: None,
            end_date: None,
        }
    }
}

/// Bundle pricing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePricing {
    pub model: BundlePricingModel,
    pub base_price_cents: i64,
    pub currency: String,
    pub billing_cycle: BillingCycle,
    pub trial_days: Option<u32>,
    pub partner_revenue_share: Option<f64>, // Percentage
    pub discounts: Vec<BundleDiscount>,
}

/// Bundle pricing model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundlePricingModel {
    FlatFee,
    PerSubscriber,
    RevenueShare,
    Hybrid,
}

/// Billing cycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BillingCycle {
    Monthly,
    Quarterly,
    Annual,
    OneTime,
}

/// Bundle discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDiscount {
    pub id: Uuid,
    pub name: String,
    pub discount_type: DiscountType,
    pub value: f64, // Percentage or fixed amount
    pub conditions: DiscountConditions,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

/// Discount type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscountType {
    Percentage,
    FixedAmount,
    FreeTrial,
}

/// Discount conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountConditions {
    pub new_subscribers_only: bool,
    pub promo_code: Option<String>,
    pub min_commitment_months: Option<u32>,
    pub partner_codes: Vec<String>,
}

/// Bundle availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAvailability {
    pub available_from: Option<DateTime<Utc>>,
    pub available_until: Option<DateTime<Utc>>,
    pub territories: Vec<String>,
    pub sales_channels: Vec<SalesChannel>,
}

/// Sales channels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SalesChannel {
    Direct,
    Partner,
    Marketplace,
    All,
}

/// Bundle status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

/// Bundle subscription (user's access to a bundle).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSubscription {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub bundle_id: Uuid,
    pub partner_subscription_id: Option<String>,
    pub status: BundleSubscriptionStatus,
    pub started_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub payment_method_id: Option<String>,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Bundle subscription status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleSubscriptionStatus {
    Active,
    PastDue,
    Canceled,
    Paused,
    Trialing,
    Expired,
}

/// Bundle service.
pub struct BundleService {
    bundles: Arc<DashMap<Uuid, Bundle>>,
    subscriptions: Arc<DashMap<Uuid, BundleSubscription>>, // subscription_id -> subscription
    user_subscriptions: Arc<DashMap<Uuid, Vec<Uuid>>>, // user_id -> subscription_ids
    partner_validators: Arc<DashMap<String, Box<dyn PartnerBundleValidator + Send + Sync>>>,
}

impl BundleService {
    /// Create a new bundle service.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            bundles: Arc::new(DashMap::new()),
            subscriptions: Arc::new(DashMap::new()),
            user_subscriptions: Arc::new(DashMap::new()),
            partner_validators: Arc::new(DashMap::new()),
        })
    }

    /// Create a new bundle.
    pub async fn create_bundle(&self, mut bundle: Bundle) -> Result<Bundle> {
        bundle.id = Uuid::new_v4();
        bundle.created_at = Utc::now();
        bundle.updated_at = Utc::now();

        self.bundles.insert(bundle.id, bundle.clone());
        Ok(bundle)
    }

    /// Update a bundle.
    pub async fn update_bundle(&self, bundle_id: Uuid, updates: BundleUpdate) -> Result<Bundle> {
        let mut bundle = self.bundles.get_mut(&bundle_id)
            .ok_or_else(|| anyhow::anyhow!("Bundle not found"))?;

        if let Some(name) = updates.name {
            bundle.name = name;
        }
        if let Some(description) = updates.description {
            bundle.description = Some(description);
        }
        if let Some(logo_url) = updates.logo_url {
            bundle.logo_url = Some(logo_url);
        }
        if let Some(included_content) = updates.included_content {
            bundle.included_content = included_content;
        }
        if let Some(included_channels) = updates.included_channels {
            bundle.included_channels = included_channels;
        }
        if let Some(pricing) = updates.pricing {
            bundle.pricing = pricing;
        }
        if let Some(availability) = updates.availability {
            bundle.availability = availability;
        }
        if let Some(status) = updates.status {
            bundle.status = status;
        }
        if let Some(metadata) = updates.metadata {
            bundle.metadata = metadata;
        }

        bundle.updated_at = Utc::now();
        Ok(bundle.clone())
    }

    /// Get a bundle.
    pub fn get_bundle(&self, bundle_id: Uuid) -> Option<Bundle> {
        self.bundles.get(&bundle_id).map(|b| b.clone())
    }

    /// List bundles for a tenant.
    pub fn list_bundles(&self, tenant_id: Uuid, status: Option<BundleStatus>) -> Vec<Bundle> {
        self.bundles.iter()
            .filter(|b| b.tenant_id == tenant_id)
            .filter(|b| status.map(|s| b.status == s).unwrap_or(true))
            .map(|b| b.clone())
            .collect()
    }

    /// Subscribe user to bundle.
    pub async fn subscribe(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        bundle_id: Uuid,
        partner_subscription_id: Option<String>,
        payment_method_id: Option<String>,
    ) -> Result<BundleSubscription> {
        let bundle = self.get_bundle(bundle_id)
            .ok_or_else(|| anyhow::anyhow!("Bundle not found"))?;

        if bundle.status != BundleStatus::Active {
            return Err(anyhow::anyhow!("Bundle not available"));
        }

        // Validate partner subscription if provided
        if let Some(partner_id) = bundle.partner_id {
            if let Some(partner_sub_id) = &partner_subscription_id {
                self.validate_partner_subscription(partner_id, partner_sub_id).await?;
            }
        }

        let now = Utc::now();
        let period_end = match bundle.pricing.billing_cycle {
            BillingCycle::Monthly => now + chrono::Duration::days(30),
            BillingCycle::Quarterly => now + chrono::Duration::days(90),
            BillingCycle::Annual => now + chrono::Duration::days(365),
            BillingCycle::OneTime => now + chrono::Duration::days(36500), // ~100 years
        };

        let subscription = BundleSubscription {
            id: Uuid::new_v4(),
            tenant_id,
            user_id,
            bundle_id,
            partner_subscription_id,
            status: BundleSubscriptionStatus::Active,
            started_at: now,
            expires_at: None,
            cancelled_at: None,
            payment_method_id,
            current_period_start: now,
            current_period_end: period_end,
            metadata: HashMap::new(),
        };

        self.subscriptions.insert(subscription.id, subscription.clone());
        self.user_subscriptions
            .entry(user_id)
            .or_default()
            .push(subscription.id);

        Ok(subscription)
    }

    /// Cancel bundle subscription.
    pub async fn cancel_subscription(&self, subscription_id: Uuid, at_period_end: bool) -> Result<()> {
        let mut subscription = self.subscriptions.get_mut(&subscription_id)
            .ok_or_else(|| anyhow::anyhow!("Subscription not found"))?;

        if at_period_end {
            subscription.status = BundleSubscriptionStatus::Canceled;
            subscription.cancelled_at = Some(Utc::now());
            // Will expire at period end
        } else {
            subscription.status = BundleSubscriptionStatus::Canceled;
            subscription.cancelled_at = Some(Utc::now());
            subscription.expires_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Get user's active bundle subscriptions.
    pub fn get_user_subscriptions(&self, user_id: Uuid) -> Vec<BundleSubscription> {
        self.user_subscriptions.get(&user_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.subscriptions.get(id).map(|s| s.clone()))
                    .filter(|s| s.status == BundleSubscriptionStatus::Active)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if user has access to content via bundle.
    pub fn check_bundle_access(&self, user_id: Uuid, content_id: Uuid) -> bool {
        let subscriptions = self.get_user_subscriptions(user_id);
        for sub in subscriptions {
            if let Some(bundle) = self.get_bundle(sub.bundle_id) {
                if bundle.status != BundleStatus::Active {
                    continue;
                }

                // Check if content is in bundle
                for content in &bundle.included_content {
                    if content.content_id == content_id {
                        // Check restrictions
                        if self.check_restrictions(&content.restrictions) {
                            return true;
                        }
                    }
                }

                // Check channels
                if bundle.included_channels.contains(&content_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Validate bundle restrictions.
    fn check_restrictions(&self, restrictions: &BundleRestrictions) -> bool {
        let now = Utc::now();

        if let Some(start) = restrictions.start_date {
            if now < start {
                return false;
            }
        }
        if let Some(end) = restrictions.end_date {
            if now > end {
                return false;
            }
        }

        // Territory/platform checks would require context
        // For now, return true if date restrictions pass
        true
    }

    /// Register a partner bundle validator.
    pub fn register_partner_validator(&self, partner_name: String, validator: Box<dyn PartnerBundleValidator + Send + Sync>) {
        self.partner_validators.insert(partner_name, validator);
    }

    /// Validate partner subscription.
    async fn validate_partner_subscription(&self, partner_id: Uuid, partner_subscription_id: &str) -> Result<()> {
        // Find validator for partner
        // In production, would look up partner by ID and use their validator
        // For now, check if any validator can handle it
        for validator in self.partner_validators.iter() {
            if validator.can_validate(partner_id) {
                return validator.validate(partner_subscription_id).await;
            }
        }

        Err(anyhow::anyhow!("No validator for partner"))
    }

    /// Get bundle analytics.
    pub fn get_bundle_analytics(&self, bundle_id: Uuid) -> BundleAnalytics {
        let subscriptions: Vec<BundleSubscription> = self.subscriptions.iter()
            .filter(|s| s.bundle_id == bundle_id)
            .map(|s| s.clone())
            .collect();

        let active = subscriptions.iter().filter(|s| s.status == BundleSubscriptionStatus::Active).count();
        let trial = subscriptions.iter().filter(|s| s.status == BundleSubscriptionStatus::Trialing).count();
        let canceled = subscriptions.iter().filter(|s| s.status == BundleSubscriptionStatus::Canceled).count();
        let past_due = subscriptions.iter().filter(|s| s.status == BundleSubscriptionStatus::PastDue).count();

        BundleAnalytics {
            bundle_id,
            total_subscriptions: subscriptions.len(),
            active_subscriptions: active,
            trial_subscriptions: trial,
            canceled_subscriptions: canceled,
            past_due_subscriptions: past_due,
            churn_rate: if subscriptions.is_empty() { 0.0 } else { canceled as f64 / subscriptions.len() as f64 },
        }
    }
}

/// Partner bundle validator trait.
#[async_trait::async_trait]
pub trait PartnerBundleValidator: Send + Sync {
    fn can_validate(&self, partner_id: Uuid) -> bool;
    async fn validate(&self, partner_subscription_id: &str) -> Result<PartnerValidationResult>;
}

/// Partner validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartnerValidationResult {
    pub valid: bool,
    pub subscriber_id: Option<String>,
    pub subscription_status: Option<String>,
    pub entitled_content: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Bundle analytics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAnalytics {
    pub bundle_id: Uuid,
    pub total_subscriptions: usize,
    pub active_subscriptions: usize,
    pub trial_subscriptions: usize,
    pub canceled_subscriptions: usize,
    pub past_due_subscriptions: usize,
    pub churn_rate: f64,
}

/// Bundle update.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BundleUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub included_content: Option<Vec<BundleContent>>,
    pub included_channels: Option<Vec<Uuid>>,
    pub pricing: Option<BundlePricing>,
    pub availability: Option<BundleAvailability>,
    pub status: Option<BundleStatus>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl Default for BundleAvailability {
    fn default() -> Self {
        Self {
            available_from: None,
            available_until: None,
            territories: Vec::new(),
            sales_channels: vec![SalesChannel::All],
        }
    }
}

impl Default for BundlePricing {
    fn default() -> Self {
        Self {
            model: BundlePricingModel::FlatFee,
            base_price_cents: 0,
            currency: "USD".to_string(),
            billing_cycle: BillingCycle::Monthly,
            trial_days: None,
            partner_revenue_share: None,
            discounts: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_creation() {
        let bundle = Bundle {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Premium Bundle".to_string(),
            description: Some("All premium content".to_string()),
            logo_url: None,
            bundle_type: BundleType::TelcoBundle,
            partner_id: None,
            partner_bundle_id: None,
            included_content: vec![],
            included_channels: vec![],
            pricing: BundlePricing::default(),
            availability: BundleAvailability::default(),
            status: BundleStatus::Active,
            metadata: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(bundle.bundle_type, BundleType::TelcoBundle);
        assert_eq!(bundle.status, BundleStatus::Active);
    }

    #[test]
    fn test_bundle_pricing() {
        let pricing = BundlePricing {
            model: BundlePricingModel::RevenueShare,
            base_price_cents: 999,
            currency: "USD".to_string(),
            billing_cycle: BillingCycle::Monthly,
            trial_days: Some(7),
            partner_revenue_share: Some(0.3),
            discounts: vec![],
        };

        assert_eq!(pricing.model, BundlePricingModel::RevenueShare);
        assert_eq!(pricing.partner_revenue_share, Some(0.3));
    }
}