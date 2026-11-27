//! Monetization Manager
//!
//! Orchestrates all monetization services:
//! - Entitlement service
//! - Billing integration
//! - FAST channel scheduler
//! - TVOD storefront
//! - Bundle support

use std::sync::Arc;

use anyhow::Result;

use crate::monetization::{
    billing::BillingService, bundle::BundleService, entitlement::EntitlementService,
    fast::FastScheduler, tvod::TvodStorefront,
};

/// Monetization manager orchestrating all monetization services.
pub struct MonetizationManager {
    entitlement: Arc<EntitlementService>,
    billing: Arc<BillingService>,
    fast_scheduler: Arc<FastScheduler>,
    tvod: Arc<TvodStorefront>,
    bundles: Arc<BundleService>,
}

impl MonetizationManager {
    /// Create a new monetization manager.
    pub fn new() -> Self {
        Self {
            entitlement: Arc::new(EntitlementService::new(Default::default())),
            billing: Arc::new(BillingService::new(Default::default())),
            fast_scheduler: Arc::new(FastScheduler::new(Default::default())),
            tvod: Arc::new(TvodStorefront::new(Default::default())),
            bundles: Arc::new(BundleService::new()),
        }
    }

    /// Start all monetization services.
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting monetization services");
        Ok(())
    }

    /// Stop all monetization services.
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping monetization services");
        Ok(())
    }

    // Accessors
    pub fn entitlement(&self) -> &Arc<EntitlementService> {
        &self.entitlement
    }
    pub fn billing(&self) -> &Arc<BillingService> {
        &self.billing
    }
    pub fn fast_scheduler(&self) -> &Arc<FastScheduler> {
        &self.fast_scheduler
    }
    pub fn tvod(&self) -> &Arc<TvodStorefront> {
        &self.tvod
    }
    pub fn bundles(&self) -> &Arc<BundleService> {
        &self.bundles
    }
}

impl Default for MonetizationManager {
    fn default() -> Self {
        Self::new()
    }
}
