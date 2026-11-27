//! TVOD (Transactional VOD) Storefront
//!
//! Rental/purchase entitlement windows, refund handling.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// TVOD product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvodProduct {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub content_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    pub trailer_url: Option<String>,
    pub rental_price_cents: Option<i64>,
    pub purchase_price_cents: Option<i64>,
    pub rental_window_hours: Option<u32>,    // How long to start watching
    pub rental_duration_hours: Option<u32>,  // How long to finish once started
    pub purchase_entitlement: PurchaseEntitlement,
    pub availability: TvodAvailability,
    pub metadata: HashMap<String, serde_json::Value>,
    pub status: TvodProductStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// TVOD product status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TvodProductStatus {
    Draft,
    Active,
    Hidden,
    Archived,
}

/// Purchase entitlement type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseEntitlement {
    Permanent,      // Own forever
    LimitedYears,   // Expires after N years
    Subscription,   // Requires active subscription
}

/// TVOD availability window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvodAvailability {
    pub available_from: Option<DateTime<Utc>>,
    pub available_until: Option<DateTime<Utc>>,
    pub territories: Vec<String>, // ISO country codes
    pub platforms: Vec<String>,   // web, iOS, Android, TV
}

/// TVOD transaction (rental or purchase).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvodTransaction {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub transaction_type: TvodTransactionType,
    pub status: TvodTransactionStatus,
    pub amount_cents: i64,
    pub currency: String,
    pub payment_method_id: Option<String>,
    pub provider_transaction_id: Option<String>,
    pub entitlement: TvodEntitlement,
    pub refund: Option<TvodRefund>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// TVOD transaction type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TvodTransactionType {
    Rental,
    Purchase,
}

/// TVOD transaction status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TvodTransactionStatus {
    Pending,
    Completed,
    Failed,
    Refunded,
    Cancelled,
}

/// TVOD entitlement (access window).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvodEntitlement {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub entitlement_type: TvodEntitlementType,
    pub starts_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub playback_started_at: Option<DateTime<Utc>>,
    pub playback_expires_at: Option<DateTime<Utc>>,
    pub status: TvodEntitlementStatus,
}

/// TVOD entitlement type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TvodEntitlementType {
    Rental,
    Purchase,
}

/// TVOD entitlement status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TvodEntitlementStatus {
    Active,
    Expired,
    Consumed,      // Rental fully watched
    Refunded,
    Revoked,
}

/// TVOD refund.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvodRefund {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub reason: RefundReason,
    pub amount_cents: i64,
    pub status: RefundStatus,
    pub provider_refund_id: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Refund reason.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefundReason {
    TechnicalIssue,
    ContentNotAsDescribed,
    AccidentalPurchase,
    CustomerRequest,
    PolicyException,
}

/// Refund status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefundStatus {
    Pending,
    Approved,
    Rejected,
    Processed,
    Failed,
}

/// TVOD storefront configuration.
#[derive(Debug, Clone)]
pub struct TvodStorefrontConfig {
    pub default_rental_window_hours: u32,
    pub default_rental_duration_hours: u32,
    pub refund_policy_days: u32,
    pub max_refunds_per_user: u32,
    pub auto_grant_entitlement: bool,
}

impl Default for TvodStorefrontConfig {
    fn default() -> Self {
        Self {
            default_rental_window_hours: 72,    // 3 days to start
            default_rental_duration_hours: 48,  // 48 hours to finish
            refund_policy_days: 7,
            max_refunds_per_user: 3,
            auto_grant_entitlement: true,
        }
    }
}

/// TVOD storefront service.
pub struct TvodStorefront {
    config: TvodStorefrontConfig,
    products: Arc<DashMap<Uuid, TvodProduct>>,
    transactions: Arc<DashMap<Uuid, TvodTransaction>>,
    entitlements: Arc<DashMap<Uuid, TvodEntitlement>>, // entitlement_id -> entitlement
    user_entitlements: Arc<DashMap<Uuid, Vec<Uuid>>>, // user_id -> entitlement_ids
}

impl TvodStorefront {
    /// Create a new TVOD storefront.
    pub fn new(config: TvodStorefrontConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            products: Arc::new(DashMap::new()),
            transactions: Arc::new(DashMap::new()),
            entitlements: Arc::new(DashMap::new()),
            user_entitlements: Arc::new(DashMap::new()),
        })
    }

    /// Create a TVOD product.
    pub async fn create_product(&self, mut product: TvodProduct) -> Result<TvodProduct> {
        product.id = Uuid::new_v4();
        product.created_at = Utc::now();
        product.updated_at = Utc::now();

        // Set defaults
        if product.rental_window_hours.is_none() {
            product.rental_window_hours = Some(self.config.default_rental_window_hours);
        }
        if product.rental_duration_hours.is_none() {
            product.rental_duration_hours = Some(self.config.default_rental_duration_hours);
        }

        self.products.insert(product.id, product.clone());
        Ok(product)
    }

    /// Update a TVOD product.
    pub async fn update_product(&self, product_id: Uuid, updates: TvodProductUpdate) -> Result<TvodProduct> {
        let mut product = self.products.get_mut(&product_id)
            .ok_or_else(|| anyhow::anyhow!("Product not found"))?;

        if let Some(title) = updates.title {
            product.title = title;
        }
        if let Some(description) = updates.description {
            product.description = Some(description);
        }
        if let Some(poster_url) = updates.poster_url {
            product.poster_url = Some(poster_url);
        }
        if let Some(rental_price) = updates.rental_price_cents {
            product.rental_price_cents = Some(rental_price);
        }
        if let Some(purchase_price) = updates.purchase_price_cents {
            product.purchase_price_cents = Some(purchase_price);
        }
        if let Some(rental_window) = updates.rental_window_hours {
            product.rental_window_hours = Some(rental_window);
        }
        if let Some(rental_duration) = updates.rental_duration_hours {
            product.rental_duration_hours = Some(rental_duration);
        }
        if let Some(availability) = updates.availability {
            product.availability = availability;
        }
        if let Some(status) = updates.status {
            product.status = status;
        }
        if let Some(metadata) = updates.metadata {
            product.metadata = metadata;
        }

        product.updated_at = Utc::now();
        Ok(product.clone())
    }

    /// Get a TVOD product.
    pub fn get_product(&self, product_id: Uuid) -> Option<TvodProduct> {
        self.products.get(&product_id).map(|p| p.clone())
    }

    /// List TVOD products for a tenant.
    pub fn list_products(&self, tenant_id: Uuid, status: Option<TvodProductStatus>) -> Vec<TvodProduct> {
        self.products.iter()
            .filter(|p| p.tenant_id == tenant_id)
            .filter(|p| status.map(|s| p.status == s).unwrap_or(true))
            .map(|p| p.clone())
            .collect()
    }

    /// Purchase/rent a product.
    pub async fn purchase(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        product_id: Uuid,
        transaction_type: TvodTransactionType,
        payment_method_id: Option<String>,
    ) -> Result<TvodTransaction> {
        let product = self.get_product(product_id)
            .ok_or_else(|| anyhow::anyhow!("Product not found"))?;

        // Validate availability
        self.validate_availability(&product)?;

        // Get price
        let price_cents = match transaction_type {
            TvodTransactionType::Rental => product.rental_price_cents
                .ok_or_else(|| anyhow::anyhow!("Product not available for rental"))?,
            TvodTransactionType::Purchase => product.purchase_price_cents
                .ok_or_else(|| anyhow::anyhow!("Product not available for purchase"))?,
        };

        // Create transaction
        let mut transaction = TvodTransaction {
            id: Uuid::new_v4(),
            tenant_id,
            user_id,
            product_id,
            transaction_type,
            status: TvodTransactionStatus::Pending,
            amount_cents: price_cents,
            currency: "USD".to_string(),
            payment_method_id,
            provider_transaction_id: None,
            entitlement: TvodEntitlement {
                id: Uuid::new_v4(),
                transaction_id: Uuid::new_v4(), // Will update after transaction created
                user_id,
                product_id,
                entitlement_type: match transaction_type {
                    TvodTransactionType::Rental => TvodEntitlementType::Rental,
                    TvodTransactionType::Purchase => TvodEntitlementType::Purchase,
                },
                starts_at: Utc::now(),
                expires_at: None,
                playback_started_at: None,
                playback_expires_at: None,
                status: TvodEntitlementStatus::Active,
            },
            refund: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
            completed_at: None,
        };

        // In production, process payment with provider
        // For now, simulate success
        transaction.status = TvodTransactionStatus::Completed;
        transaction.completed_at = Some(Utc::now());
        transaction.provider_transaction_id = Some(format!("sim_{}", Uuid::new_v4()));

        // Update entitlement with correct transaction ID
        transaction.entitlement.transaction_id = transaction.id;

        // Calculate entitlement windows
        self.calculate_entitlement_windows(&product, &mut transaction.entitlement);

        // Grant entitlement
        if self.config.auto_grant_entitlement {
            self.grant_entitlement(&transaction.entitlement).await?;
        }

        self.transactions.insert(transaction.id, transaction.clone());

        Ok(transaction)
    }

    /// Validate product availability.
    fn validate_availability(&self, product: &TvodProduct) -> Result<()> {
        if product.status != TvodProductStatus::Active {
            return Err(anyhow::anyhow!("Product not available"));
        }

        let now = Utc::now();
        if let Some(from) = product.availability.available_from {
            if now < from {
                return Err(anyhow::anyhow!("Product not yet available"));
            }
        }
        if let Some(until) = product.availability.available_until {
            if now > until {
                return Err(anyhow::anyhow!("Product no longer available"));
            }
        }

        Ok(())
    }

    /// Calculate entitlement time windows.
    fn calculate_entitlement_windows(&self, product: &TvodProduct, entitlement: &mut TvodEntitlement) {
        match entitlement.entitlement_type {
            TvodEntitlementType::Rental => {
                // Rental: window to start, then duration to finish
                let window_hours = product.rental_window_hours.unwrap_or(self.config.default_rental_window_hours);
                let duration_hours = product.rental_duration_hours.unwrap_or(self.config.default_rental_duration_hours);

                entitlement.expires_at = Some(entitlement.starts_at + ChronoDuration::hours(window_hours as i64));
                // playback_expires_at set when playback starts
            }
            TvodEntitlementType::Purchase => {
                match product.purchase_entitlement {
                    PurchaseEntitlement::Permanent => {
                        entitlement.expires_at = None;
                    }
                    PurchaseEntitlement::LimitedYears => {
                        // Default to 10 years if not specified
                        entitlement.expires_at = Some(entitlement.starts_at + ChronoDuration::days(3650));
                    }
                    PurchaseEntitlement::Subscription => {
                        // Tied to subscription - would check subscription status
                        entitlement.expires_at = None; // Placeholder
                    }
                }
            }
        }
    }

    /// Grant entitlement to user.
    async fn grant_entitlement(&self, entitlement: &TvodEntitlement) -> Result<()> {
        self.entitlements.insert(entitlement.id, entitlement.clone());

        // Index by user
        self.user_entitlements
            .entry(entitlement.user_id)
            .or_default()
            .push(entitlement.id);

        Ok(())
    }

    /// Start playback (activates rental duration).
    pub async fn start_playback(&self, user_id: Uuid, product_id: Uuid) -> Result<TvodEntitlement> {
        let entitlement_id = self.user_entitlements.get(&user_id)
            .and_then(|ids| {
                ids.iter().find(|id| {
                    self.entitlements.get(id)
                        .map(|e| e.product_id == product_id && e.status == TvodEntitlementStatus::Active)
                        .unwrap_or(false)
                }).copied()
            })
            .ok_or_else(|| anyhow::anyhow!("No active entitlement found"))?;

        let mut entitlement = self.entitlements.get_mut(&entitlement_id)
            .ok_or_else(|| anyhow::anyhow!("Entitlement not found"))?;

        if entitlement.playback_started_at.is_none() {
            entitlement.playback_started_at = Some(Utc::now());

            // For rentals, set playback expiration
            if entitlement.entitlement_type == TvodEntitlementType::Rental {
                if let Some(product) = self.get_product(product_id) {
                    let duration_hours = product.rental_duration_hours.unwrap_or(self.config.default_rental_duration_hours);
                    entitlement.playback_expires_at = Some(Utc::now() + ChronoDuration::hours(duration_hours as i64));
                }
            }
        }

        Ok(entitlement.clone())
    }

    /// Check if user has valid entitlement for product.
    pub fn check_entitlement(&self, user_id: Uuid, product_id: Uuid) -> Option<TvodEntitlement> {
        self.user_entitlements.get(&user_id)?
            .iter()
            .find_map(|entitlement_id| {
                self.entitlements.get(entitlement_id).and_then(|e| {
                    let entitlement = e.clone();
                    if entitlement.product_id != product_id {
                        return None;
                    }
                    if entitlement.status != TvodEntitlementStatus::Active {
                        return None;
                    }
                    let now = Utc::now();
                    if let Some(expires) = entitlement.expires_at {
                        if now > expires {
                            return None;
                        }
                    }
                    if let Some(playback_expires) = entitlement.playback_expires_at {
                        if now > playback_expires {
                            return None;
                        }
                    }
                    Some(entitlement)
                })
            })
    }

    /// Request a refund.
    pub async fn request_refund(
        &self,
        user_id: Uuid,
        transaction_id: Uuid,
        reason: RefundReason,
    ) -> Result<TvodRefund> {
        let transaction = self.transactions.get(&transaction_id)
            .ok_or_else(|| anyhow::anyhow!("Transaction not found"))?;

        if transaction.user_id != user_id {
            return Err(anyhow::anyhow!("Transaction not owned by user"));
        }

        // Check refund policy
        let now = Utc::now();
        let policy_window = ChronoDuration::days(self.config.refund_policy_days as i64);
        if now - transaction.created_at > policy_window {
            return Err(anyhow::anyhow!("Refund window expired"));
        }

        // Check refund eligibility
        match transaction.transaction_type {
            TvodTransactionType::Rental => {
                // Can refund if not started playback
                if transaction.entitlement.playback_started_at.is_some() {
                    return Err(anyhow::anyhow!("Cannot refund rental after playback started"));
                }
            }
            TvodTransactionType::Purchase => {
                // Check if content was consumed
                // This would require tracking playback
            }
        }

        // Check user refund limit
        let user_refunds = self.transactions.iter()
            .filter(|t| t.user_id == user_id && t.refund.is_some())
            .count();
        if user_refunds >= self.config.max_refunds_per_user as usize {
            return Err(anyhow::anyhow!("Maximum refunds per user exceeded"));
        }

        let refund = TvodRefund {
            id: Uuid::new_v4(),
            transaction_id,
            reason,
            amount_cents: transaction.amount_cents,
            status: RefundStatus::Pending,
            provider_refund_id: None,
            requested_at: Utc::now(),
            processed_at: None,
        };

        // Update transaction
        if let Some(mut t) = self.transactions.get_mut(&transaction_id) {
            t.refund = Some(refund.clone());
            t.status = TvodTransactionStatus::Refunded;
        }

        // Revoke entitlement
        if let Some(mut e) = self.entitlements.get_mut(&transaction.entitlement.id) {
            e.status = TvodEntitlementStatus::Refunded;
        }

        Ok(refund)
    }

    /// Process refund (approve/reject).
    pub async fn process_refund(&self, refund_id: Uuid, approve: bool) -> Result<TvodRefund> {
        // Find refund by ID
        let mut refund = None;
        for t in self.transactions.iter() {
            if let Some(r) = &t.refund {
                if r.id == refund_id {
                    refund = Some(r.clone());
                    break;
                }
            }
        }

        let mut refund = refund.ok_or_else(|| anyhow::anyhow!("Refund not found"))?;

        if approve {
            refund.status = RefundStatus::Approved;
            // In production, call payment provider refund API
            refund.status = RefundStatus::Processed;
            refund.processed_at = Some(Utc::now());
        } else {
            refund.status = RefundStatus::Rejected;
        }

        // Update in transaction
        for mut t in self.transactions.iter_mut() {
            if let Some(r) = &mut t.refund {
                if r.id == refund_id {
                    *r = refund.clone();
                    break;
                }
            }
        }

        Ok(refund)
    }
}

/// TVOD product update.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TvodProductUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    pub rental_price_cents: Option<i64>,
    pub purchase_price_cents: Option<i64>,
    pub rental_window_hours: Option<u32>,
    pub rental_duration_hours: Option<u32>,
    pub availability: Option<TvodAvailability>,
    pub status: Option<TvodProductStatus>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tvod_product_creation() {
        let product = TvodProduct {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            content_id: Uuid::new_v4(),
            title: "Test Movie".to_string(),
            description: None,
            poster_url: None,
            trailer_url: None,
            rental_price_cents: Some(299),
            purchase_price_cents: Some(999),
            rental_window_hours: Some(72),
            rental_duration_hours: Some(48),
            purchase_entitlement: PurchaseEntitlement::Permanent,
            availability: TvodAvailability {
                available_from: None,
                available_until: None,
                territories: vec!["US".to_string(), "CA".to_string()],
                platforms: vec!["web".to_string(), "ios".to_string()],
            },
            metadata: HashMap::new(),
            status: TvodProductStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(product.rental_price_cents, Some(299));
        assert_eq!(product.purchase_entitlement, PurchaseEntitlement::Permanent);
    }

    #[test]
    fn test_entitlement_windows() {
        let config = TvodStorefrontConfig::default();
        let storefront = TvodStorefront::new(config);

        let mut entitlement = TvodEntitlement {
            id: Uuid::new_v4(),
            transaction_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            entitlement_type: TvodEntitlementType::Rental,
            starts_at: Utc::now(),
            expires_at: None,
            playback_started_at: None,
            playback_expires_at: None,
            status: TvodEntitlementStatus::Active,
        };

        let product = TvodProduct {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            content_id: Uuid::new_v4(),
            title: "Test".to_string(),
            description: None,
            poster_url: None,
            trailer_url: None,
            rental_price_cents: Some(299),
            purchase_price_cents: Some(999),
            rental_window_hours: Some(72),
            rental_duration_hours: Some(48),
            purchase_entitlement: PurchaseEntitlement::Permanent,
            availability: TvodAvailability::default(),
            metadata: HashMap::new(),
            status: TvodProductStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        storefront.calculate_entitlement_windows(&product, &mut entitlement);

        assert!(entitlement.expires_at.is_some());
        let window = entitlement.expires_at.unwrap() - entitlement.starts_at;
        assert_eq!(window.num_hours(), 72);
    }
}