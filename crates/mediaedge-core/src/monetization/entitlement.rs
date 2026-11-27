//! Entitlement Service
//!
//! Per-user, per-title access decisions with TTL caching.
//! Supports partner-issued subscription acceptance.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Entitlement decision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementDecision {
    Granted,
    Denied,
    Unknown,
}

/// Entitlement source.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementSource {
    Internal,      // MediaEdge-managed subscription
    Partner,       // Partner-issued (Cleeng, Evergent, etc.)
    Bundle,        // Bundle entitlement
    Promotional,   // Promotional/trial
    Corporate,     // Corporate/SSO
}

/// Entitlement scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementScope {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub content_id: Option<Uuid>,      // Specific content (VOD, live stream)
    pub channel_id: Option<Uuid>,      // FAST channel
    pub bundle_id: Option<Uuid>,       // Bundle
    pub features: Vec<String>,         // Feature flags
}

/// Entitlement result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementResult {
    pub decision: EntitlementDecision,
    pub source: EntitlementSource,
    pub scope: EntitlementScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub evaluated_at: DateTime<Utc>,
}

/// Subscription entitlement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionEntitlement {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub plan_id: String,
    pub status: SubscriptionStatus,
    pub source: EntitlementSource,
    pub partner_subscription_id: Option<String>,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub trial_end: Option<DateTime<Utc>>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Subscription status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    PastDue,
    Canceled,
    Paused,
    Trialing,
    Incomplete,
}

/// Bundle entitlement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEntitlement {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub bundle_id: Uuid,
    pub partner_bundle_id: Option<String>,
    pub status: BundleStatus,
    pub included_content: Vec<Uuid>,
    pub included_channels: Vec<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Bundle status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleStatus {
    Active,
    Expired,
    Revoked,
}

/// Entitlement service configuration.
#[derive(Debug, Clone)]
pub struct EntitlementServiceConfig {
    pub cache_ttl: Duration,
    pub default_ttl: Duration,
    pub partner_providers: Vec<PartnerProviderConfig>,
}

impl Default for EntitlementServiceConfig {
    fn default() -> Self {
        Self {
            cache_ttl: Duration::from_secs(300), // 5 minutes
            default_ttl: Duration::from_secs(3600), // 1 hour
            partner_providers: Vec::new(),
        }
    }
}

/// Partner provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartnerProviderConfig {
    pub name: String,
    pub provider_type: PartnerProviderType,
    pub api_endpoint: String,
    pub api_key: String,
    pub webhook_secret: Option<String>,
    pub enabled: bool,
}

/// Partner provider types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartnerProviderType {
    Cleeng,
    Evergent,
    Chargebee,
    RevenueCat,
    Custom,
}

/// Entitlement service.
pub struct EntitlementService {
    config: EntitlementServiceConfig,
    cache: Arc<DashMap<String, EntitlementResult>>, // cache_key -> result
    subscriptions: Arc<DashMap<Uuid, SubscriptionEntitlement>>, // subscription_id -> entitlement
    bundles: Arc<DashMap<Uuid, BundleEntitlement>>, // bundle_id -> entitlement
}

impl EntitlementService {
    /// Create a new entitlement service.
    pub fn new(config: EntitlementServiceConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            cache: Arc::new(DashMap::new()),
            subscriptions: Arc::new(DashMap::new()),
            bundles: Arc::new(DashMap::new()),
        })
    }

    /// Check entitlement for user/content.
    pub async fn check_entitlement(&self, scope: EntitlementScope) -> EntitlementResult {
        let cache_key = self.cache_key(&scope);

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.expires_at.map(|e| e > Utc::now()).unwrap_or(false) {
                return cached.clone();
            }
        }

        // Evaluate entitlement
        let result = self.evaluate_entitlement(scope.clone()).await;

        // Cache result
        self.cache.insert(cache_key, result.clone());

        result
    }

    /// Evaluate entitlement from all sources.
    async fn evaluate_entitlement(&self, scope: EntitlementScope) -> EntitlementResult {
        // 1. Check internal subscriptions
        for sub in self.subscriptions.iter() {
            if sub.tenant_id == scope.tenant_id && sub.user_id == scope.user_id {
                if sub.status == SubscriptionStatus::Active
                    && sub.current_period_end > Utc::now()
                {
                    if self.content_covered_by_subscription(&sub, &scope) {
                        return EntitlementResult {
                            decision: EntitlementDecision::Granted,
                            source: EntitlementSource::Internal,
                            scope: scope.clone(),
                            expires_at: Some(sub.current_period_end),
                            metadata: sub.metadata.clone(),
                            evaluated_at: Utc::now(),
                        };
                    }
                }
            }
        }

        // 2. Check bundles
        for bundle in self.bundles.iter() {
            if bundle.tenant_id == scope.tenant_id && bundle.user_id == scope.user_id {
                if bundle.status == BundleStatus::Active
                    && bundle.expires_at.map(|e| e > Utc::now()).unwrap_or(true)
                {
                    if bundle.included_content.contains(&scope.content_id.unwrap_or_default())
                        || bundle.included_channels.contains(&scope.channel_id.unwrap_or_default())
                    {
                        return EntitlementResult {
                            decision: EntitlementDecision::Granted,
                            source: EntitlementSource::Bundle,
                            scope: scope.clone(),
                            expires_at: bundle.expires_at,
                            metadata: HashMap::new(),
                            evaluated_at: Utc::now(),
                        };
                    }
                }
            }
        }

        // 3. Check partner providers (async calls)
        for provider in &self.config.partner_providers {
            if provider.enabled {
                if let Ok(result) = self.check_partner_entitlement(provider, &scope).await {
                    if result.decision == EntitlementDecision::Granted {
                        return result;
                    }
                }
            }
        }

        // Default: denied
        EntitlementResult {
            decision: EntitlementDecision::Denied,
            source: EntitlementSource::Internal,
            scope,
            expires_at: None,
            metadata: HashMap::new(),
            evaluated_at: Utc::now(),
        }
    }

    fn content_covered_by_subscription(&self, sub: &SubscriptionEntitlement, scope: &EntitlementScope) -> bool {
        // Check if subscription plan covers the requested content
        // This would integrate with a plan/catalog service
        if let Some(content_id) = scope.content_id {
            // Check if content is in subscription plan
            sub.metadata.get("content_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(&content_id.to_string())))
                .unwrap_or(true) // Default to true if no restrictions
        } else if let Some(channel_id) = scope.channel_id {
            // Check if channel is included
            sub.metadata.get("channel_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(&channel_id.to_string())))
                .unwrap_or(true)
        } else {
            true // General subscription access
        }
    }

    /// Check entitlement with partner provider.
    async fn check_partner_entitlement(
        &self,
        provider: &PartnerProviderConfig,
        scope: &EntitlementScope,
    ) -> Result<EntitlementResult> {
        // This would make HTTP calls to partner APIs
        // For now, return denied
        Ok(EntitlementResult {
            decision: EntitlementDecision::Denied,
            source: EntitlementSource::Partner,
            scope: scope.clone(),
            expires_at: None,
            metadata: HashMap::new(),
            evaluated_at: Utc::now(),
        })
    }

    /// Add/update subscription entitlement.
    pub async fn upsert_subscription(&self, sub: SubscriptionEntitlement) {
        self.subscriptions.insert(sub.id, sub);
        self.invalidate_user_cache(sub.tenant_id, sub.user_id);
    }

    /// Remove subscription entitlement.
    pub async fn remove_subscription(&self, subscription_id: Uuid) {
        if let Some((_, sub)) = self.subscriptions.remove(&subscription_id) {
            self.invalidate_user_cache(sub.tenant_id, sub.user_id);
        }
    }

    /// Add/update bundle entitlement.
    pub async fn upsert_bundle(&self, bundle: BundleEntitlement) {
        self.bundles.insert(bundle.id, bundle.clone());
        self.invalidate_user_cache(bundle.tenant_id, bundle.user_id);
    }

    /// Remove bundle entitlement.
    pub async fn remove_bundle(&self, bundle_id: Uuid) {
        if let Some((_, bundle)) = self.bundles.remove(&bundle_id) {
            self.invalidate_user_cache(bundle.tenant_id, bundle.user_id);
        }
    }

    /// Invalidate cache for a user.
    fn invalidate_user_cache(&self, tenant_id: Uuid, user_id: Uuid) {
        let prefix = format!("{}:{}", tenant_id, user_id);
        self.cache.retain(|k, _| !k.starts_with(&prefix));
    }

    fn cache_key(&self, scope: &EntitlementScope) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            scope.tenant_id,
            scope.user_id,
            scope.content_id.map(|c| c.to_string()).unwrap_or_else(|| "*".to_string()),
            scope.channel_id.map(|c| c.to_string()).unwrap_or_else(|| "*".to_string()),
            scope.bundle_id.map(|c| c.to_string()).unwrap_or_else(|| "*".to_string()),
        )
    }

    /// Get user's active subscriptions.
    pub async fn get_user_subscriptions(&self, tenant_id: Uuid, user_id: Uuid) -> Vec<SubscriptionEntitlement> {
        self.subscriptions.iter()
            .filter(|s| s.tenant_id == tenant_id && s.user_id == user_id)
            .map(|s| s.clone())
            .collect()
    }

    /// Get user's active bundles.
    pub async fn get_user_bundles(&self, tenant_id: Uuid, user_id: Uuid) -> Vec<BundleEntitlement> {
        self.bundles.iter()
            .filter(|b| b.tenant_id == tenant_id && b.user_id == user_id)
            .map(|b| b.clone())
            .collect()
    }

    /// Handle partner webhook (subscription updates).
    pub async fn handle_partner_webhook(
        &self,
        provider: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        // Parse webhook payload based on provider
        // Update subscription/bundle state
        tracing::info!("Received partner webhook from {}", provider);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_entitlement_check() {
        let service = EntitlementService::new(EntitlementServiceConfig::default());

        let scope = EntitlementScope {
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            content_id: Some(Uuid::new_v4()),
            channel_id: None,
            bundle_id: None,
            features: vec![],
        };

        let result = service.check_entitlement(scope).await;
        assert_eq!(result.decision, EntitlementDecision::Denied);
    }

    #[test]
    fn test_subscription_status() {
        assert_eq!(SubscriptionStatus::Active as i32, 0);
        assert_eq!(SubscriptionStatus::Canceled as i32, 2);
    }
}