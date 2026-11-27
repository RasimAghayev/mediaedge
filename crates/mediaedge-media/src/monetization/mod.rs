//! Media Monetization Manager
//!
//! Orchestrates SSAI and Content Moderation services.

use std::sync::Arc;

use anyhow::Result;

use crate::monetization::{moderation::ModerationService, ssai::SsaiManager};

/// Media monetization manager.
pub struct MonetizationManager {
    ssai: Arc<SsaiManager>,
    moderation: Arc<ModerationService>,
}

impl MonetizationManager {
    /// Create a new monetization manager.
    pub fn new() -> Self {
        Self {
            ssai: Arc::new(SsaiManager::new()),
            moderation: Arc::new(ModerationService::new(Default::default())),
        }
    }

    /// Start all monetization services.
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting media monetization services");
        Ok(())
    }

    /// Stop all monetization services.
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping media monetization services");
        Ok(())
    }

    // Accessors
    pub fn ssai(&self) -> &Arc<SsaiManager> {
        &self.ssai
    }
    pub fn moderation(&self) -> &Arc<ModerationService> {
        &self.moderation
    }
}

impl Default for MonetizationManager {
    fn default() -> Self {
        Self::new()
    }
}
