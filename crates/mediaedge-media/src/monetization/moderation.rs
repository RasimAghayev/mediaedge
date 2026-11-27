//! AI Content Moderation
//!
//! AI-based content analysis (NSFW, copyright, quality).
//! Integrates with transcoding pipeline for real-time analysis.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Content moderation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationConfig {
    pub enabled: bool,
    pub providers: Vec<ModerationProviderConfig>,
    pub thresholds: ModerationThresholds,
    pub actions: ModerationActions,
    pub sampling: SamplingConfig,
}

/// Moderation provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationProviderConfig {
    pub provider: ModerationProvider,
    pub api_endpoint: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub enabled: bool,
    pub priority: u32,
}

/// Moderation providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModerationProvider {
    AwsRekognition,
    GoogleCloudVision,
    AzureContentModerator,
    Sightengine,
    Custom,
    Local, // Local model inference
}

/// Moderation thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationThresholds {
    pub nsfw: f32,
    pub violence: f32,
    pub hate_speech: f32,
    pub self_harm: f32,
    pub sexual_content: f32,
    pub copyright: f32,
    pub quality_min: f32,
    pub custom: HashMap<String, f32>,
}

impl Default for ModerationThresholds {
    fn default() -> Self {
        Self {
            nsfw: 0.7,
            violence: 0.6,
            hate_speech: 0.8,
            self_harm: 0.9,
            sexual_content: 0.7,
            copyright: 0.8,
            quality_min: 0.5,
            custom: HashMap::new(),
        }
    }
}

/// Moderation actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationActions {
    pub flag_only: Vec<String>,           // Categories to just flag
    pub reject: Vec<String>,              // Categories to reject upload
    pub quarantine: Vec<String>,          // Categories to quarantine
    pub blur: Vec<String>,                // Categories to blur
    pub mute_audio: Vec<String>,          // Categories to mute
    pub replace_with_slate: Vec<String>,  // Categories to replace
    pub notify: Vec<String>,              // Categories to notify
}

impl Default for ModerationActions {
    fn default() -> Self {
        Self {
            flag_only: vec!["low_quality".to_string()],
            reject: vec!["child_safety".to_string(), "extreme_violence".to_string()],
            quarantine: vec!["nsfw".to_string(), "hate_speech".to_string()],
            blur: vec!["partial_nudity".to_string()],
            mute_audio: vec!["profanity".to_string()],
            replace_with_slate: vec!["copyright_match".to_string()],
            notify: vec!["all".to_string()],
        }
    }
}

/// Sampling configuration for cost control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingConfig {
    pub enabled: bool,
    pub sample_rate: f32, // 0.0 to 1.0
    pub always_moderate: Vec<String>, // Content types to always moderate
    pub never_moderate: Vec<String>,  // Content types to never moderate
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_rate: 0.1, // 10% sampling
            always_moderate: vec!["live".to_string(), "user_upload".to_string()],
            never_moderate: vec!["trusted_partner".to_string()],
        }
    }
}

/// Moderation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationResult {
    pub content_id: Uuid,
    pub tenant_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub provider: ModerationProvider,
    pub model_version: String,
    pub categories: Vec<ModerationCategory>,
    pub overall_score: f32,
    pub decision: ModerationDecision,
    pub processing_time_ms: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Moderation category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationCategory {
    pub category: ModerationCategoryType,
    pub confidence: f32,
    pub severity: ModerationSeverity,
    pub details: HashMap<String, serde_json::Value>,
    pub timestamps: Vec<ModerationTimestamp>, // For video, timestamps of violations
}

/// Moderation category types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModerationCategoryType {
    Nsfw,
    Violence,
    HateSpeech,
    SelfHarm,
    SexualContent,
    ChildSafety,
    ExtremeViolence,
    Profanity,
    Copyright,
    Trademark,
    Piracy,
    Spam,
    Misleading,
    LowQuality,
    TechnicalIssue,
    Custom(String),
}

/// Moderation severity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModerationSeverity {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Moderation timestamp (for video).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationTimestamp {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: f32,
}

/// Moderation decision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModerationDecision {
    Approve,
    Flag,
    Quarantine,
    Reject,
    Blur,
    MuteAudio,
    ReplaceWithSlate,
    RequireReview,
}

/// Moderation job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationJob {
    pub id: Uuid,
    pub content_id: Uuid,
    pub tenant_id: Uuid,
    pub content_type: ContentType,
    pub source_url: String,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub result: Option<ModerationResult>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Content types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Video,
    Audio,
    Image,
    Text,
    LiveStream,
    UserUpload,
    TrustedPartner,
}

/// Job priority.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Urgent,
}

/// Job status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

/// Moderation provider client trait.
#[async_trait]
pub trait ModerationProviderClient: Send + Sync {
    async fn moderate_video(&self, url: &str, options: ModerationOptions) -> Result<ModerationResult>;
    async fn moderate_audio(&self, url: &str, options: ModerationOptions) -> Result<ModerationResult>;
    async fn moderate_image(&self, url: &str, options: ModerationOptions) -> Result<ModerationResult>;
    async fn moderate_text(&self, text: &str, options: ModerationOptions) -> Result<ModerationResult>;
    async fn check_copyright(&self, url: &str, fingerprint: Option<&str>) -> Result<CopyrightResult>;
}

/// Moderation options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationOptions {
    pub categories: Vec<ModerationCategoryType>,
    pub thresholds: ModerationThresholds,
    pub generate_timestamps: bool,
    pub language: Option<String>,
}

/// Copyright check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyrightResult {
    pub matches: Vec<CopyrightMatch>,
    pub overall_score: f32,
    pub action: CopyrightAction,
}

/// Copyright match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyrightMatch {
    pub content_id: String,
    pub title: String,
    pub owner: String,
    pub match_type: CopyrightMatchType,
    pub confidence: f32,
    pub timestamps: Vec<ModerationTimestamp>,
}

/// Copyright match type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopyrightMatchType {
    Audio,
    Video,
    Melody,
    Visual,
    Metadata,
}

/// Copyright action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopyrightAction {
    None,
    Monitor,
    Claim,
    Block,
    Mute,
    Replace,
}

/// Moderation service.
pub struct ModerationService {
    config: ModerationConfig,
    clients: Arc<DashMap<ModerationProvider, Arc<dyn ModerationProviderClient>>>,
    jobs: Arc<DashMap<Uuid, ModerationJob>>,
    job_queue: Arc<tokio::sync::Mutex<Vec<Uuid>>>,
    results_cache: Arc<DashMap<Uuid, ModerationResult>>, // content_hash -> result
}

impl ModerationService {
    /// Create a new moderation service.
    pub fn new(config: ModerationConfig) -> Arc<Self> {
        let service = Arc::new(Self {
            config,
            clients: Arc::new(DashMap::new()),
            jobs: Arc::new(DashMap::new()),
            job_queue: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            results_cache: Arc::new(DashMap::new()),
        });

        // Start job processor
        service.start_processor();

        service
    }

    /// Register a moderation provider client.
    pub fn register_provider(&self, provider: ModerationProvider, client: Arc<dyn ModerationProviderClient>) {
        self.clients.insert(provider, client);
    }

    /// Submit content for moderation.
    pub async fn submit_moderation(
        &self,
        content_id: Uuid,
        tenant_id: Uuid,
        content_type: ContentType,
        source_url: String,
        priority: JobPriority,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<Uuid> {
        // Check cache first
        let content_hash = self.compute_content_hash(&source_url);
        if let Some(cached) = self.results_cache.get(&content_hash) {
            // Return cached result
            let job = ModerationJob {
                id: Uuid::new_v4(),
                content_id,
                tenant_id,
                content_type,
                source_url,
                priority,
                status: JobStatus::Completed,
                result: Some(cached.clone()),
                created_at: Utc::now(),
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
                error: None,
                metadata,
            };
            self.jobs.insert(job.id, job.clone());
            return Ok(job.id);
        }

        // Check sampling
        if self.config.sampling.enabled && !self.should_moderate(&content_type) {
            // Auto-approve
            let result = ModerationResult {
                content_id,
                tenant_id,
                timestamp: Utc::now(),
                provider: ModerationProvider::Local,
                model_version: "sampling_skip".to_string(),
                categories: Vec::new(),
                overall_score: 1.0,
                decision: ModerationDecision::Approve,
                processing_time_ms: 0,
                metadata: HashMap::new(),
            };

            let job = ModerationJob {
                id: Uuid::new_v4(),
                content_id,
                tenant_id,
                content_type,
                source_url,
                priority,
                status: JobStatus::Completed,
                result: Some(result.clone()),
                created_at: Utc::now(),
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
                error: None,
                metadata,
            };
            self.jobs.insert(job.id, job.clone());
            return Ok(job.id);
        }

        let job = ModerationJob {
            id: Uuid::new_v4(),
            content_id,
            tenant_id,
            content_type,
            source_url,
            priority,
            status: JobStatus::Pending,
            result: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
            metadata,
        };

        self.jobs.insert(job.id, job.clone());

        // Add to queue
        let mut queue = self.job_queue.lock().await;
        queue.push(job.id);
        queue.sort_by_key(|id| {
            self.jobs.get(id).map(|j| std::cmp::Reverse(j.priority)).unwrap_or_default()
        });

        Ok(job.id)
    }

    fn should_moderate(&self, content_type: &ContentType) -> bool {
        let type_str = format!("{:?}", content_type).to_lowercase();

        if self.config.sampling.never_moderate.contains(&type_str) {
            return false;
        }
        if self.config.sampling.always_moderate.contains(&type_str) {
            return true;
        }

        // Random sampling
        use rand::Rng;
        rand::thread_rng().gen::<f32>() < self.config.sampling.sample_rate
    }

    fn compute_content_hash(&self, url: &str) -> Uuid {
        use blake3;
        let hash = blake3::hash(url.as_bytes());
        Uuid::from_slice(hash.as_bytes()).unwrap_or_default()
    }

    /// Get moderation job status.
    pub fn get_job(&self, job_id: Uuid) -> Option<ModerationJob> {
        self.jobs.get(&job_id).map(|j| j.clone())
    }

    /// Get moderation result for content.
    pub fn get_result(&self, content_id: Uuid) -> Option<ModerationResult> {
        self.jobs.iter()
            .find(|j| j.content_id == content_id && j.status == JobStatus::Completed)
            .and_then(|j| j.result.clone())
    }

    /// Start background job processor.
    fn start_processor(&self) {
        let jobs = self.jobs.clone();
        let queue = self.job_queue.clone();
        let clients = self.clients.clone();
        let config = self.config.clone();
        let results_cache = self.results_cache.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;

                let job_id = {
                    let mut q = queue.lock().await;
                    q.pop()
                };

                if let Some(job_id) = job_id {
                    Self::process_job(job_id, &jobs, &clients, &config, &results_cache).await;
                }
            }
        });
    }

    async fn process_job(
        job_id: Uuid,
        jobs: &DashMap<Uuid, ModerationJob>,
        clients: &DashMap<ModerationProvider, Arc<dyn ModerationProviderClient>>,
        config: &ModerationConfig,
        results_cache: &DashMap<Uuid, ModerationResult>,
    ) {
        let mut job = match jobs.get_mut(&job_id) {
            Some(j) => j.clone(),
            None => return,
        };

        job.status = JobStatus::Processing;
        job.started_at = Some(Utc::now());
        jobs.insert(job_id, job.clone());

        let start = std::time::Instant::now();

        // Select provider (highest priority enabled)
        let provider = config.providers.iter()
            .filter(|p| p.enabled)
            .min_by_key(|p| p.priority)
            .map(|p| p.provider);

        let result = if let Some(provider) = provider {
            if let Some(client) = clients.get(&provider) {
                let options = ModerationOptions {
                    categories: ModerationCategoryType::all(),
                    thresholds: config.thresholds.clone(),
                    generate_timestamps: true,
                    language: None,
                };

                match job.content_type {
                    ContentType::Video | ContentType::LiveStream => {
                        client.moderate_video(&job.source_url, options).await
                    }
                    ContentType::Audio => {
                        client.moderate_audio(&job.source_url, options).await
                    }
                    ContentType::Image => {
                        client.moderate_image(&job.source_url, options).await
                    }
                    ContentType::Text => {
                        // Would need text content
                        Err(anyhow::anyhow!("Text moderation requires content"))
                    }
                    _ => Err(anyhow::anyhow!("Unsupported content type")),
                }
            } else {
                Err(anyhow::anyhow!("Provider client not registered"))
            }
        } else {
            Err(anyhow::anyhow!("No enabled providers"))
        };

        let processing_time = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut moderation_result) => {
                moderation_result.processing_time_ms = processing_time;

                // Apply thresholds and determine decision
                moderation_result.decision = Self::determine_decision(&moderation_result, config);

                // Cache result
                let content_hash = Self::compute_content_hash_static(&job.source_url);
                results_cache.insert(content_hash, moderation_result.clone());

                job.result = Some(moderation_result);
                job.status = JobStatus::Completed;
            }
            Err(e) => {
                job.status = JobStatus::Failed;
                job.error = Some(e.to_string());
            }
        }

        job.completed_at = Some(Utc::now());
        jobs.insert(job_id, job);
    }

    fn determine_decision(result: &ModerationResult, config: &ModerationConfig) -> ModerationDecision {
        for category in &result.categories {
            let threshold = config.thresholds.custom.get(&format!("{:?}", category.category).to_lowercase())
                .or_else(|| Self::get_default_threshold(&config.thresholds, category.category))
                .copied()
                .unwrap_or(0.5);

            if category.confidence >= threshold {
                let category_str = format!("{:?}", category.category).to_lowercase();

                if config.actions.reject.contains(&category_str) {
                    return ModerationDecision::Reject;
                }
                if config.actions.quarantine.contains(&category_str) {
                    return ModerationDecision::Quarantine;
                }
                if config.actions.blur.contains(&category_str) {
                    return ModerationDecision::Blur;
                }
                if config.actions.mute_audio.contains(&category_str) {
                    return ModerationDecision::MuteAudio;
                }
                if config.actions.replace_with_slate.contains(&category_str) {
                    return ModerationDecision::ReplaceWithSlate;
                }
                if config.actions.flag_only.contains(&category_str) {
                    return ModerationDecision::Flag;
                }
            }
        }

        ModerationDecision::Approve
    }

    fn get_default_threshold(thresholds: &ModerationThresholds, category: ModerationCategoryType) -> Option<f32> {
        match category {
            ModerationCategoryType::Nsfw => Some(thresholds.nsfw),
            ModerationCategoryType::Violence => Some(thresholds.violence),
            ModerationCategoryType::HateSpeech => Some(thresholds.hate_speech),
            ModerationCategoryType::SelfHarm => Some(thresholds.self_harm),
            ModerationCategoryType::SexualContent => Some(thresholds.sexual_content),
            ModerationCategoryType::Copyright => Some(thresholds.copyright),
            ModerationCategoryType::LowQuality => Some(1.0 - thresholds.quality_min),
            _ => None,
        }
    }

    fn compute_content_hash_static(url: &str) -> Uuid {
        use blake3;
        let hash = blake3::hash(url.as_bytes());
        Uuid::from_slice(hash.as_bytes()).unwrap_or_default()
    }
}

impl ModerationCategoryType {
    fn all() -> Vec<Self> {
        vec![
            Self::Nsfw,
            Self::Violence,
            Self::HateSpeech,
            Self::SelfHarm,
            Self::SexualContent,
            Self::ChildSafety,
            Self::ExtremeViolence,
            Self::Profanity,
            Self::Copyright,
            Self::Trademark,
            Self::Piracy,
            Self::Spam,
            Self::Misleading,
            Self::LowQuality,
            Self::TechnicalIssue,
        ]
    }
}

/// AWS Rekognition client (simplified).
pub struct RekognitionClient {
    config: ModerationProviderConfig,
    http_client: reqwest::Client,
}

impl RekognitionClient {
    pub fn new(config: ModerationProviderConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModerationProviderClient for RekognitionClient {
    async fn moderate_video(&self, url: &str, options: ModerationOptions) -> Result<ModerationResult> {
        // Call AWS Rekognition Video Moderation
        Ok(ModerationResult {
            content_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            provider: ModerationProvider::AwsRekognition,
            model_version: "rekognition-v1".to_string(),
            categories: Vec::new(),
            overall_score: 1.0,
            decision: ModerationDecision::Approve,
            processing_time_ms: 0,
            metadata: HashMap::new(),
        })
    }

    async fn moderate_audio(&self, url: &str, options: ModerationOptions) -> Result<ModerationResult> {
        Ok(ModerationResult::default())
    }

    async fn moderate_image(&self, url: &str, options: ModerationOptions) -> Result<ModerationResult> {
        Ok(ModerationResult::default())
    }

    async fn moderate_text(&self, text: &str, options: ModerationOptions) -> Result<ModerationResult> {
        Ok(ModerationResult::default())
    }

    async fn check_copyright(&self, url: &str, fingerprint: Option<&str>) -> Result<CopyrightResult> {
        Ok(CopyrightResult {
            matches: Vec::new(),
            overall_score: 0.0,
            action: CopyrightAction::None,
        })
    }
}

impl Default for ModerationResult {
    fn default() -> Self {
        Self {
            content_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            provider: ModerationProvider::Local,
            model_version: "default".to_string(),
            categories: Vec::new(),
            overall_score: 1.0,
            decision: ModerationDecision::Approve,
            processing_time_ms: 0,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moderation_thresholds() {
        let thresholds = ModerationThresholds::default();
        assert_eq!(thresholds.nsfw, 0.7);
        assert_eq!(thresholds.violence, 0.6);
    }

    #[test]
    fn test_moderation_decision() {
        assert_eq!(ModerationDecision::Approve as i32, 0);
        assert_eq!(ModerationDecision::Reject as i32, 3);
    }
}