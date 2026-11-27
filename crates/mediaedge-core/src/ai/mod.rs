//! AI Manager
//!
//! Orchestrates all AI services:
//! - AI Scheduling (Amazon Bedrock AgentCore)
//! - Capacity Planning
//! - (Anomaly detection is in mediaedge-observability)

use std::sync::Arc;

use anyhow::Result;

use crate::ai::{
    capacity::CapacityPlanner,
    scheduler::AiSchedulingService,
};

/// AI manager orchestrating all AI services.
pub struct AiManager {
    scheduler: Arc<AiSchedulingService>,
    capacity_planner: Arc<CapacityPlanner>,
}

impl AiManager {
    /// Create a new AI manager.
    pub fn new() -> Self {
        Self {
            scheduler: Arc::new(AiSchedulingService::new(Default::default())),
            capacity_planner: Arc::new(CapacityPlanner::new(Default::default())),
        }
    }

    /// Start all AI services.
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting AI services");
        Ok(())
    }

    /// Stop all AI services.
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping AI services");
        Ok(())
    }

    // Accessors
    pub fn scheduler(&self) -> &Arc<AiSchedulingService> { &self.scheduler }
    pub fn capacity_planner(&self) -> &Arc<CapacityPlanner> { &self.capacity_planner }
}

impl Default for AiManager {
    fn default() -> Self {
        Self::new()
    }
}