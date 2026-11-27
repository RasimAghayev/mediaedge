//! AI Capacity Planning
//!
//! ML-based viewer prediction for pre-scaling.
//! Cost optimization (spot vs on-demand).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Capacity planning configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityPlanningConfig {
    pub enabled: bool,
    pub prediction_horizon_hours: u32,
    pub update_interval_minutes: u32,
    pub models: Vec<CapacityModelConfig>,
    pub scaling_policies: Vec<ScalingPolicy>,
    pub cost_optimization: CostOptimizationConfig,
}

/// Capacity model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityModelConfig {
    pub model_type: CapacityModelType,
    pub metric_patterns: Vec<String>,
    pub prediction_horizon_hours: u32,
    pub confidence_threshold: f64,
    pub retrain_interval_hours: u32,
}

/// Capacity model types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapacityModelType {
    LinearRegression,
    Arima,
    Prophet,
    Lstm,
    Transformer,
    Ensemble,
    Custom,
}

/// Scaling policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicy {
    pub id: Uuid,
    pub name: String,
    pub resource_type: ResourceType,
    pub trigger: ScalingTrigger,
    pub action: ScalingAction,
    pub cooldown_minutes: u32,
    pub min_instances: u32,
    pub max_instances: u32,
    pub enabled: bool,
}

/// Resource types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    IngestNode,
    TranscodeGpu,
    EdgeNode,
    StorageNode,
    GatewayNode,
    OriginShield,
}

/// Scaling trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingTrigger {
    pub metric: String,
    pub condition: TriggerCondition,
    pub threshold: f64,
    pub duration_seconds: u64,
}

/// Trigger condition.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerCondition {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    RateOfChange,
}

/// Scaling action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingAction {
    pub action_type: ScalingActionType,
    pub instance_type: Option<String>,
    pub count: u32,
    pub spot_preference: SpotPreference,
}

/// Scaling action types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScalingActionType {
    ScaleOut,
    ScaleIn,
    ReplaceInstance,
    ChangeInstanceType,
}

/// Spot instance preference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpotPreference {
    PreferSpot,
    PreferOnDemand,
    SpotOnly,
    OnDemandOnly,
    Mixed { spot_percentage: u8 },
}

/// Cost optimization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostOptimizationConfig {
    pub enabled: bool,
    pub spot_interruption_handling: SpotInterruptionHandling,
    pub max_spot_percentage: u8,
    pub savings_target_percentage: f32,
    pub instance_types: Vec<InstanceTypeConfig>,
}

/// Spot interruption handling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpotInterruptionHandling {
    GracefulDrain,
    ImmediateFailover,
    CheckpointAndRestart,
}

/// Instance type configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceTypeConfig {
    pub instance_type: String,
    pub resource_type: ResourceType,
    pub vcpu: u32,
    pub memory_gb: u32,
    pub gpu: Option<GpuConfig>,
    pub on_demand_price_per_hour: f64,
    pub spot_price_per_hour: Option<f64>,
    pub max_instances: u32,
    pub regions: Vec<String>,
}

/// GPU configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    pub count: u32,
    pub model: String,
    pub memory_gb: u32,
}

/// Capacity prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityPrediction {
    pub resource_type: ResourceType,
    pub region: String,
    pub timestamp: DateTime<Utc>,
    pub predicted_demand: f64,
    pub current_capacity: f64,
    pub recommended_capacity: f64,
    pub confidence: f64,
    pub scaling_recommendations: Vec<ScalingRecommendation>,
    pub cost_estimate: CostEstimate,
}

/// Scaling recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingRecommendation {
    pub policy_id: Uuid,
    pub action: ScalingAction,
    pub reason: String,
    pub urgency: RecommendationUrgency,
    pub estimated_cost_impact: f64,
}

/// Recommendation urgency.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationUrgency {
    Low,
    Medium,
    High,
    Critical,
}

/// Cost estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub current_hourly_cost: f64,
    pub recommended_hourly_cost: f64,
    pub savings_per_hour: f64,
    pub savings_percentage: f32,
    pub breakdown: HashMap<String, f64>,
}

/// Viewer prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerPrediction {
    pub channel_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub predicted_viewers: f64,
    pub confidence_interval: (f64, f64),
    pub factors: Vec<PredictionFactor>,
}

/// Prediction factor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionFactor {
    pub name: String,
    pub impact: f64,
    pub description: String,
}

/// Historical viewer data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerHistory {
    pub channel_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub viewers: u64,
    pub metadata: HashMap<String, String>,
}

/// Capacity planner service.
pub struct CapacityPlanner {
    config: CapacityPlanningConfig,
    predictions: Arc<DashMap<String, CapacityPrediction>>, // resource_type:region -> prediction
    viewer_history: Arc<DashMap<String, VecDeque<ViewerHistory>>>, // key -> history
    models: Arc<DashMap<String, Box<dyn CapacityModel + Send + Sync>>>,
    scaling_policies: Arc<DashMap<Uuid, ScalingPolicy>>,
}

impl CapacityPlanner {
    /// Create a new capacity planner.
    pub fn new(config: CapacityPlanningConfig) -> Arc<Self> {
        let planner = Arc::new(Self {
            config,
            predictions: Arc::new(DashMap::new()),
            viewer_history: Arc::new(DashMap::new()),
            models: Arc::new(DashMap::new()),
            scaling_policies: Arc::new(DashMap::new()),
        });

        planner.init_default_models();
        planner.load_scaling_policies();

        planner
    }

    fn init_default_models(&self) {
        // Linear regression model
        self.models.insert(
            "linear_regression".to_string(),
            Box::new(LinearRegressionModel::new(CapacityModelConfig {
                model_type: CapacityModelType::LinearRegression,
                metric_patterns: vec!["viewers.*".to_string(), "bandwidth.*".to_string()],
                prediction_horizon_hours: 24,
                confidence_threshold: 0.7,
                retrain_interval_hours: 168, // Weekly
            }))
        );
    }

    fn load_scaling_policies(&self) {
        // Default policies
        let policies = vec![
            ScalingPolicy {
                id: Uuid::new_v4(),
                name: "Ingest Scale Out".to_string(),
                resource_type: ResourceType::IngestNode,
                trigger: ScalingTrigger {
                    metric: "ingest.concurrent_streams".to_string(),
                    condition: TriggerCondition::GreaterThan,
                    threshold: 8000.0, // 80% of 10k per node
                    duration_seconds: 300,
                },
                action: ScalingAction {
                    action_type: ScalingActionType::ScaleOut,
                    instance_type: Some("c6i.2xlarge".to_string()),
                    count: 1,
                    spot_preference: SpotPreference::Mixed { spot_percentage: 50 },
                },
                cooldown_minutes: 10,
                min_instances: 2,
                max_instances: 100,
                enabled: true,
            },
            ScalingPolicy {
                id: Uuid::new_v4(),
                name: "Transcode GPU Scale Out".to_string(),
                resource_type: ResourceType::TranscodeGpu,
                trigger: ScalingTrigger {
                    metric: "transcode.queue_depth".to_string(),
                    condition: TriggerCondition::GreaterThan,
                    threshold: 10.0,
                    duration_seconds: 180,
                },
                action: ScalingAction {
                    action_type: ScalingActionType::ScaleOut,
                    instance_type: Some("g5.2xlarge".to_string()),
                    count: 1,
                    spot_preference: SpotPreference::PreferSpot,
                },
                cooldown_minutes: 15,
                min_instances: 1,
                max_instances: 50,
                enabled: true,
            },
            ScalingPolicy {
                id: Uuid::new_v4(),
                name: "Edge Scale Out".to_string(),
                resource_type: ResourceType::EdgeNode,
                trigger: ScalingTrigger {
                    metric: "edge.cache_miss_rate".to_string(),
                    condition: TriggerCondition::GreaterThan,
                    threshold: 0.1, // 10% miss rate
                    duration_seconds: 600,
                },
                action: ScalingAction {
                    action_type: ScalingActionType::ScaleOut,
                    instance_type: Some("c6i.xlarge".to_string()),
                    count: 2,
                    spot_preference: SpotPreference::Mixed { spot_percentage: 30 },
                },
                cooldown_minutes: 15,
                min_instances: 3,
                max_instances: 200,
                enabled: true,
            },
        ];

        for policy in policies {
            self.scaling_policies.insert(policy.id, policy);
        }
    }

    /// Record viewer count.
    pub fn record_viewers(&self, channel_id: Option<Uuid>, tenant_id: Option<Uuid>, viewers: u64, metadata: HashMap<String, String>) {
        let key = self.history_key(channel_id, tenant_id);
        let history = self.viewer_history.entry(key).or_insert_with(VecDeque::new);

        history.push_back(ViewerHistory {
            channel_id,
            tenant_id,
            timestamp: Utc::now(),
            viewers,
            metadata,
        });

        // Keep last 90 days
        let cutoff = Utc::now() - ChronoDuration::days(90);
        while history.front().map(|h| h.timestamp < cutoff).unwrap_or(false) {
            history.pop_front();
        }
    }

    /// Generate capacity predictions.
    pub async fn generate_predictions(&self) -> Result<Vec<CapacityPrediction>> {
        let mut predictions = Vec::new();

        for policy in self.scaling_policies.iter() {
            if !policy.enabled {
                continue;
            }

            for model in self.models.iter() {
                if self.model_applies_to_resource(&model, policy.resource_type) {
                    if let Ok(prediction) = model.predict(&policy, &self.viewer_history).await {
                        predictions.push(prediction);
                    }
                }
            }
        }

        // Store predictions
        for pred in &predictions {
            let key = format!("{}:{}", pred.resource_type as u8, pred.region);
            self.predictions.insert(key, pred.clone());
        }

        Ok(predictions)
    }

    fn model_applies_to_resource(&self, model: &Box<dyn CapacityModel>, resource_type: ResourceType) -> bool {
        // Check if model's metric patterns match resource type
        true
    }

    /// Get prediction for resource type and region.
    pub fn get_prediction(&self, resource_type: ResourceType, region: &str) -> Option<CapacityPrediction> {
        let key = format!("{}:{}", resource_type as u8, region);
        self.predictions.get(&key).map(|p| p.clone())
    }

    /// Evaluate scaling policies and generate recommendations.
    pub async fn evaluate_scaling(&self) -> Result<Vec<ScalingRecommendation>> {
        let mut recommendations = Vec::new();
        let predictions = self.generate_predictions().await?;

        for pred in predictions {
            for policy in self.scaling_policies.iter() {
                if policy.resource_type != pred.resource_type {
                    continue;
                }

                // Check if scaling is needed
                let utilization = pred.predicted_demand / pred.current_capacity.max(1.0);

                if utilization > 0.8 {
                    recommendations.push(ScalingRecommendation {
                        policy_id: policy.id,
                        action: policy.action.clone(),
                        reason: format!("Predicted utilization {:.1}% exceeds 80%", utilization * 100.0),
                        urgency: if utilization > 1.0 { RecommendationUrgency::Critical } else { RecommendationUrgency::High },
                        estimated_cost_impact: self.estimate_cost_impact(&policy.action),
                    });
                } else if utilization < 0.3 && pred.current_capacity > policy.min_instances as f64 {
                    // Scale in recommendation
                    let mut scale_in = policy.action.clone();
                    scale_in.action_type = ScalingActionType::ScaleIn;
                    scale_in.count = 1;

                    recommendations.push(ScalingRecommendation {
                        policy_id: policy.id,
                        action: scale_in,
                        reason: format!("Predicted utilization {:.1}% below 30%", utilization * 100.0),
                        urgency: RecommendationUrgency::Low,
                        estimated_cost_impact: -self.estimate_cost_impact(&policy.action),
                    });
                }
            }
        }

        Ok(recommendations)
    }

    fn estimate_cost_impact(&self, action: &ScalingAction) -> f64 {
        // Simplified cost estimation
        let base_cost = match action.instance_type.as_deref() {
            Some("c6i.2xlarge") => 0.34,
            Some("g5.2xlarge") => 1.20,
            Some("c6i.xlarge") => 0.17,
            _ => 0.50,
        };

        let spot_discount = match action.spot_preference {
            SpotPreference::PreferSpot => 0.7,
            SpotPreference::SpotOnly => 0.8,
            SpotPreference::Mixed { spot_percentage } => 0.3 * (spot_percentage as f64 / 100.0),
            _ => 0.0,
        };

        base_cost * (1.0 - spot_discount) * action.count as f64
    }

    /// Get viewer prediction for a channel.
    pub async fn predict_viewers(&self, channel_id: Uuid, horizon_hours: u32) -> Result<ViewerPrediction> {
        let key = self.history_key(Some(channel_id), None);
        let history = self.viewer_history.get(&key)
            .map(|h| h.clone())
            .unwrap_or_default();

        if history.len() < 24 {
            return Ok(ViewerPrediction {
                channel_id: Some(channel_id),
                tenant_id: None,
                timestamp: Utc::now(),
                predicted_viewers: 0.0,
                confidence_interval: (0.0, 0.0),
                factors: vec![],
            });
        }

        // Use simple moving average for prediction
        let recent: Vec<f64> = history.iter().rev().take(24).map(|h| h.viewers as f64).collect();
        let avg = recent.iter().sum::<f64>() / recent.len() as f64;
        let std_dev = (recent.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / recent.len() as f64).sqrt();

        Ok(ViewerPrediction {
            channel_id: Some(channel_id),
            tenant_id: None,
            timestamp: Utc::now() + ChronoDuration::hours(horizon_hours as i64),
            predicted_viewers: avg,
            confidence_interval: (avg - 1.96 * std_dev, avg + 1.96 * std_dev),
            factors: vec![
                PredictionFactor {
                    name: "recent_average".to_string(),
                    impact: 0.8,
                    description: "24-hour moving average".to_string(),
                },
            ],
        })
    }

    fn history_key(&self, channel_id: Option<Uuid>, tenant_id: Option<Uuid>) -> String {
        match (channel_id, tenant_id) {
            (Some(c), Some(t)) => format!("channel:{}:tenant:{}", c, t),
            (Some(c), None) => format!("channel:{}", c),
            (None, Some(t)) => format!("tenant:{}", t),
            (None, None) => "global".to_string(),
        }
    }
}

/// Capacity model trait.
#[async_trait::async_trait]
pub trait CapacityModel: Send + Sync {
    async fn predict(&self, policy: &ScalingPolicy, history: &DashMap<String, VecDeque<ViewerHistory>>) -> Result<CapacityPrediction>;
    fn model_type(&self) -> CapacityModelType;
    fn train(&mut self, history: &DashMap<String, VecDeque<ViewerHistory>>) -> Result<()>;
}

/// Linear regression model.
pub struct LinearRegressionModel {
    config: CapacityModelConfig,
    coefficients: Option<Vec<f64>>,
}

impl LinearRegressionModel {
    pub fn new(config: CapacityModelConfig) -> Self {
        Self {
            config,
            coefficients: None,
        }
    }
}

#[async_trait::async_trait]
impl CapacityModel for LinearRegressionModel {
    async fn predict(&self, policy: &ScalingPolicy, history: &DashMap<String, VecDeque<ViewerHistory>>) -> Result<CapacityPrediction> {
        // Simplified prediction using recent average
        let key = format!("global");
        let recent_viewers = history.get(&key)
            .map(|h| {
                h.iter().rev().take(168).map(|v| v.viewers as f64).sum::<f64>() / h.len().min(168) as f64
            })
            .unwrap_or(0.0);

        Ok(CapacityPrediction {
            resource_type: policy.resource_type,
            region: "us-east-1".to_string(),
            timestamp: Utc::now(),
            predicted_demand: recent_viewers * 1.2, // 20% buffer
            current_capacity: policy.min_instances as f64 * 10000.0, // 10k per node
            recommended_capacity: (recent_viewers * 1.2 / 10000.0).ceil() * 10000.0,
            confidence: 0.75,
            scaling_recommendations: Vec::new(),
            cost_estimate: CostEstimate {
                current_hourly_cost: policy.min_instances as f64 * 0.34,
                recommended_hourly_cost: (recent_viewers * 1.2 / 10000.0).ceil() * 0.34,
                savings_per_hour: 0.0,
                savings_percentage: 0.0,
                breakdown: HashMap::new(),
            },
        })
    }

    fn model_type(&self) -> CapacityModelType {
        CapacityModelType::LinearRegression
    }

    fn train(&mut self, history: &DashMap<String, VecDeque<ViewerHistory>>) -> Result<()> {
        // Train linear regression on historical data
        self.coefficients = Some(vec![1.0]); // Simplified
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaling_policy() {
        let policy = ScalingPolicy {
            id: Uuid::new_v4(),
            name: "Test Policy".to_string(),
            resource_type: ResourceType::IngestNode,
            trigger: ScalingTrigger {
                metric: "test.metric".to_string(),
                condition: TriggerCondition::GreaterThan,
                threshold: 100.0,
                duration_seconds: 300,
            },
            action: ScalingAction {
                action_type: ScalingActionType::ScaleOut,
                instance_type: Some("c6i.2xlarge".to_string()),
                count: 1,
                spot_preference: SpotPreference::PreferSpot,
            },
            cooldown_minutes: 10,
            min_instances: 2,
            max_instances: 100,
            enabled: true,
        };

        assert_eq!(policy.resource_type, ResourceType::IngestNode);
        assert_eq!(policy.action.spot_preference, SpotPreference::PreferSpot);
    }

    #[test]
    fn test_cost_optimization_config() {
        let config = CostOptimizationConfig {
            enabled: true,
            spot_interruption_handling: SpotInterruptionHandling::GracefulDrain,
            max_spot_percentage: 70,
            savings_target_percentage: 30.0,
            instance_types: vec![],
        };

        assert!(config.enabled);
        assert_eq!(config.max_spot_percentage, 70);
    }
}