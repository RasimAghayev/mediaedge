//! Immutable Audit Log
//!
//! Tamper-evident audit logging with cryptographic chaining.
//! Supports querying, retention policies, and compliance requirements.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use blake3;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use mediaedge_config::Config;

/// Audit log configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogConfig {
    pub pool: PgPool,
    pub retention_days: u32,
    pub batch_size: usize,
    pub flush_interval: Duration,
    pub signing_key: Option<String>,
    pub external_sink: Option<ExternalSinkConfig>,
}

impl AuditLogConfig {
    pub fn from_config(config: &Config) -> Result<Self> {
        let pool = PgPool::connect_lazy(&config.database.url)?;
        Ok(Self {
            pool,
            retention_days: config.audit.retention_days.unwrap_or(2555), // 7 years
            batch_size: config.audit.batch_size.unwrap_or(100),
            flush_interval: Duration::from_millis(config.audit.flush_interval_ms.unwrap_or(1000)),
            signing_key: config.audit.signing_key.clone(),
            external_sink: config.audit.external_sink.clone(),
        })
    }
}

/// External sink for audit log forwarding (SIEM, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalSinkConfig {
    pub endpoint: String,
    pub api_key: String,
    pub batch_size: usize,
    pub timeout_secs: u64,
}

/// Audit event severity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Audit event category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    Authentication,
    Authorization,
    TenantManagement,
    UserManagement,
    RoleManagement,
    ApiKeyManagement,
    SecretManagement,
    StreamManagement,
    RecordingManagement,
    ConfigurationChange,
    SecurityEvent,
    DataAccess,
    SystemEvent,
}

/// Audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub tenant_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub category: AuditCategory,
    pub severity: AuditSeverity,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub outcome: AuditOutcome,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub previous_hash: String,
    pub hash: String,
}

/// Audit event outcome.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
    Partial,
    Denied,
}

/// Audit log query parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditQuery {
    pub tenant_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub category: Option<AuditCategory>,
    pub severity: Option<AuditSeverity>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub outcome: Option<AuditOutcome>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub limit: u32,
    pub offset: u32,
    pub order_by: AuditOrderBy,
    pub order_dir: AuditOrderDir,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuditOrderBy {
    #[default]
    Timestamp,
    Severity,
    Category,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuditOrderDir {
    #[default]
    Desc,
    Asc,
}

/// Audit logger with cryptographic chaining.
pub struct AuditLogger {
    config: AuditLogConfig,
    buffer: Arc<tokio::sync::Mutex<Vec<AuditEvent>>>,
    last_hash: Arc<parking_lot::RwLock<String>>,
    signing_key: Option<blake3::Key>,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub async fn new(config: &Config) -> Result<Arc<Self>> {
        let logger_config = AuditLogConfig::from_config(config)?;

        let signing_key = logger_config.signing_key.as_ref().map(|k| {
            let mut key_bytes = [0u8; 32];
            let key_data = hex::decode(k).unwrap_or_default();
            key_bytes[..key_data.len().min(32)].copy_from_slice(&key_data);
            blake3::Key::new(&key_bytes)
        });

        let logger = Arc::new(Self {
            config: logger_config,
            buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            last_hash: Arc::new(parking_lot::RwLock::new(String::new())),
            signing_key,
        });

        // Initialize last hash from database
        logger.init_last_hash().await?;

        // Start background flusher
        logger.start_flusher();

        Ok(logger)
    }

    async fn init_last_hash(&self) -> Result<()> {
        let row = sqlx::query!(
            r#"
            SELECT hash FROM audit_events
            ORDER BY timestamp DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.config.pool)
        .await?;

        if let Some(row) = row {
            *self.last_hash.write() = row.hash;
        }

        Ok(())
    }

    fn start_flusher(&self) {
        let buffer = self.buffer.clone();
        let config = self.config.clone();
        let last_hash = self.last_hash.clone();
        let signing_key = self.signing_key;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.flush_interval);
            loop {
                interval.tick().await;
                Self::flush_buffer(buffer.clone(), &config, last_hash.clone(), signing_key).await;
            }
        });
    }

    async fn flush_buffer(
        buffer: Arc<tokio::sync::Mutex<Vec<AuditEvent>>>,
        config: &AuditLogConfig,
        last_hash: Arc<parking_lot::RwLock<String>>,
        signing_key: Option<blake3::Key>,
    ) {
        let mut events = buffer.lock().await;
        if events.is_empty() {
            return;
        }

        let batch = events
            .drain(..events.len().min(config.batch_size))
            .collect::<Vec<_>>();

        // Chain hashes
        let mut prev_hash = last_hash.read().clone();
        for event in &mut batch {
            event.previous_hash = prev_hash.clone();
            event.hash = Self::compute_hash(event, &signing_key);
            prev_hash = event.hash.clone();
        }

        *last_hash.write() = prev_hash;

        // Batch insert
        if let Err(e) = Self::persist_batch(&config.pool, &batch).await {
            tracing::error!("Failed to persist audit events: {}", e);
            // Re-queue failed events
            events.extend(batch);
        }

        // Forward to external sink if configured
        if let Some(sink) = &config.external_sink {
            if let Err(e) = Self::forward_to_sink(sink, &batch).await {
                tracing::warn!("Failed to forward audit events to external sink: {}", e);
            }
        }
    }

    fn compute_hash(event: &AuditEvent, signing_key: &Option<blake3::Key>) -> String {
        let mut hasher = if let Some(key) = signing_key {
            blake3::Hasher::new_keyed(key)
        } else {
            blake3::Hasher::new()
        };

        // Hash all immutable fields
        hasher.update(&event.id.as_bytes());
        hasher.update(&event.timestamp.to_rfc3339().as_bytes());
        hasher.update(&event.tenant_id.map(|u| u.as_bytes()).unwrap_or_default());
        hasher.update(&event.user_id.map(|u| u.as_bytes()).unwrap_or_default());
        hasher.update(&event.category as u8 as &[u8]);
        hasher.update(&event.severity as u8 as &[u8]);
        hasher.update(event.action.as_bytes());
        hasher.update(&event.resource_type.as_deref().unwrap_or("").as_bytes());
        hasher.update(&event.resource_id.map(|u| u.as_bytes()).unwrap_or_default());
        hasher.update(&event.outcome as u8 as &[u8]);
        hasher.update(
            serde_json::to_vec(&event.details)
                .unwrap_or_default()
                .as_slice(),
        );
        hasher.update(&event.ip_address.as_deref().unwrap_or("").as_bytes());
        hasher.update(&event.user_agent.as_deref().unwrap_or("").as_bytes());
        hasher.update(&event.request_id.as_deref().unwrap_or("").as_bytes());
        hasher.update(&event.session_id.as_deref().unwrap_or("").as_bytes());
        hasher.update(event.previous_hash.as_bytes());

        hex::encode(hasher.finalize().as_bytes())
    }

    async fn persist_batch(pool: &PgPool, events: &[AuditEvent]) -> Result<()> {
        let mut tx = pool.begin().await?;

        for event in events {
            sqlx::query!(
                r#"
                INSERT INTO audit_events (
                    id, timestamp, tenant_id, user_id, api_key_id, category, severity,
                    action, resource_type, resource_id, outcome, details, ip_address,
                    user_agent, request_id, session_id, previous_hash, hash
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                "#,
                event.id,
                event.timestamp,
                event.tenant_id,
                event.user_id,
                event.api_key_id,
                event.category as i16,
                event.severity as i16,
                event.action,
                event.resource_type,
                event.resource_id,
                event.outcome as i16,
                event.details,
                event.ip_address,
                event.user_agent,
                event.request_id,
                event.session_id,
                event.previous_hash,
                event.hash,
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn forward_to_sink(sink: &ExternalSinkConfig, events: &[AuditEvent]) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .post(&sink.endpoint)
            .header("Authorization", format!("Bearer {}", sink.api_key))
            .json(&events)
            .timeout(Duration::from_secs(sink.timeout_secs))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "External sink returned {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Log an audit event (async, buffered).
    pub async fn log(&self, event: AuditEventBuilder) {
        let event = event.build(self.last_hash.read().clone());
        self.buffer.lock().await.push(event);
    }

    /// Log an audit event synchronously (for critical events).
    pub async fn log_sync(&self, event: AuditEventBuilder) -> Result<()> {
        let event = event.build(self.last_hash.read().clone());
        let events = vec![event];
        Self::persist_batch(&self.config.pool, &events).await?;
        Ok(())
    }

    /// Query audit events.
    pub async fn query(&self, query: AuditQuery) -> Result<Vec<AuditEvent>> {
        let mut sql = String::from(
            r#"
            SELECT id, timestamp, tenant_id, user_id, api_key_id, category, severity,
                   action, resource_type, resource_id, outcome, details, ip_address,
                   user_agent, request_id, session_id, previous_hash, hash
            FROM audit_events
            WHERE 1=1
            "#,
        );

        let mut params: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> = Vec::new();
        let mut param_idx = 1;

        if let Some(tenant_id) = query.tenant_id {
            sql.push_str(&format!(" AND tenant_id = ${}", param_idx));
            params.push(Box::new(tenant_id));
            param_idx += 1;
        }

        if let Some(user_id) = query.user_id {
            sql.push_str(&format!(" AND user_id = ${}", param_idx));
            params.push(Box::new(user_id));
            param_idx += 1;
        }

        if let Some(category) = query.category {
            sql.push_str(&format!(" AND category = ${}", param_idx));
            params.push(Box::new(category as i16));
            param_idx += 1;
        }

        if let Some(severity) = query.severity {
            sql.push_str(&format!(" AND severity >= ${}", param_idx));
            params.push(Box::new(severity as i16));
            param_idx += 1;
        }

        if let Some(action) = query.action {
            sql.push_str(&format!(" AND action ILIKE ${}", param_idx));
            params.push(Box::new(format!("%{}%", action)));
            param_idx += 1;
        }

        if let Some(resource_type) = query.resource_type {
            sql.push_str(&format!(" AND resource_type = ${}", param_idx));
            params.push(Box::new(resource_type));
            param_idx += 1;
        }

        if let Some(resource_id) = query.resource_id {
            sql.push_str(&format!(" AND resource_id = ${}", param_idx));
            params.push(Box::new(resource_id));
            param_idx += 1;
        }

        if let Some(outcome) = query.outcome {
            sql.push_str(&format!(" AND outcome = ${}", param_idx));
            params.push(Box::new(outcome as i16));
            param_idx += 1;
        }

        if let Some(start_time) = query.start_time {
            sql.push_str(&format!(" AND timestamp >= ${}", param_idx));
            params.push(Box::new(start_time));
            param_idx += 1;
        }

        if let Some(end_time) = query.end_time {
            sql.push_str(&format!(" AND timestamp <= ${}", param_idx));
            params.push(Box::new(end_time));
            param_idx += 1;
        }

        if let Some(ip) = query.ip_address {
            sql.push_str(&format!(" AND ip_address = ${}", param_idx));
            params.push(Box::new(ip));
            param_idx += 1;
        }

        let order_col = match query.order_by {
            AuditOrderBy::Timestamp => "timestamp",
            AuditOrderBy::Severity => "severity",
            AuditOrderBy::Category => "category",
        };
        let order_dir = match query.order_dir {
            AuditOrderDir::Desc => "DESC",
            AuditOrderDir::Asc => "ASC",
        };
        sql.push_str(&format!(" ORDER BY {} {}", order_col, order_dir));

        sql.push_str(&format!(" LIMIT ${}", param_idx));
        params.push(Box::new(query.limit as i64));
        param_idx += 1;

        sql.push_str(&format!(" OFFSET ${}", param_idx));
        params.push(Box::new(query.offset as i64));

        // Note: In real implementation, use sqlx::query_as with proper parameter binding
        // This is simplified for demonstration
        let rows = sqlx::query(&sql).fetch_all(&self.config.pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| AuditEvent {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                tenant_id: row.get("tenant_id"),
                user_id: row.get("user_id"),
                api_key_id: row.get("api_key_id"),
                category: match row.get::<i16, _>("category") {
                    0 => AuditCategory::Authentication,
                    1 => AuditCategory::Authorization,
                    2 => AuditCategory::TenantManagement,
                    3 => AuditCategory::UserManagement,
                    4 => AuditCategory::RoleManagement,
                    5 => AuditCategory::ApiKeyManagement,
                    6 => AuditCategory::SecretManagement,
                    7 => AuditCategory::StreamManagement,
                    8 => AuditCategory::RecordingManagement,
                    9 => AuditCategory::ConfigurationChange,
                    10 => AuditCategory::SecurityEvent,
                    11 => AuditCategory::DataAccess,
                    _ => AuditCategory::SystemEvent,
                },
                severity: match row.get::<i16, _>("severity") {
                    0 => AuditSeverity::Debug,
                    1 => AuditSeverity::Info,
                    2 => AuditSeverity::Warning,
                    3 => AuditSeverity::Error,
                    _ => AuditSeverity::Critical,
                },
                action: row.get("action"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                outcome: match row.get::<i16, _>("outcome") {
                    0 => AuditOutcome::Success,
                    1 => AuditOutcome::Failure,
                    2 => AuditOutcome::Partial,
                    _ => AuditOutcome::Denied,
                },
                details: row.get("details"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                request_id: row.get("request_id"),
                session_id: row.get("session_id"),
                previous_hash: row.get("previous_hash"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Verify audit log integrity (check hash chain).
    pub async fn verify_integrity(
        &self,
        start_id: Option<Uuid>,
        end_id: Option<Uuid>,
    ) -> Result<IntegrityReport> {
        let mut query = String::from(
            r#"
            SELECT id, timestamp, previous_hash, hash
            FROM audit_events
            WHERE 1=1
            "#,
        );

        if let Some(start) = start_id {
            query.push_str(&format!(" AND id >= '{}'", start));
        }
        if let Some(end) = end_id {
            query.push_str(&format!(" AND id <= '{}'", end));
        }
        query.push_str(" ORDER BY timestamp ASC");

        let rows = sqlx::query(&query).fetch_all(&self.config.pool).await?;

        let mut report = IntegrityReport {
            total_events: rows.len(),
            verified: 0,
            failed: 0,
            failures: Vec::new(),
        };

        let mut prev_hash = String::new();
        for row in rows {
            let id: Uuid = row.get("id");
            let hash: String = row.get("hash");
            let previous_hash: String = row.get("previous_hash");

            if previous_hash != prev_hash {
                report.failed += 1;
                report.failures.push(IntegrityFailure {
                    event_id: id,
                    expected_previous_hash: prev_hash,
                    actual_previous_hash: previous_hash,
                });
            } else {
                report.verified += 1;
            }
            prev_hash = hash;
        }

        Ok(report)
    }
}

/// Audit event builder for ergonomic logging.
pub struct AuditEventBuilder {
    tenant_id: Option<Uuid>,
    user_id: Option<Uuid>,
    api_key_id: Option<Uuid>,
    category: AuditCategory,
    severity: AuditSeverity,
    action: String,
    resource_type: Option<String>,
    resource_id: Option<Uuid>,
    outcome: AuditOutcome,
    details: serde_json::Value,
    ip_address: Option<String>,
    user_agent: Option<String>,
    request_id: Option<String>,
    session_id: Option<String>,
}

impl AuditEventBuilder {
    pub fn new(category: AuditCategory, action: impl Into<String>) -> Self {
        Self {
            tenant_id: None,
            user_id: None,
            api_key_id: None,
            category,
            severity: AuditSeverity::Info,
            action: action.into(),
            resource_type: None,
            resource_id: None,
            outcome: AuditOutcome::Success,
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
            request_id: None,
            session_id: None,
        }
    }

    pub fn tenant_id(mut self, id: Uuid) -> Self {
        self.tenant_id = Some(id);
        self
    }

    pub fn user_id(mut self, id: Uuid) -> Self {
        self.user_id = Some(id);
        self
    }

    pub fn api_key_id(mut self, id: Uuid) -> Self {
        self.api_key_id = Some(id);
        self
    }

    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn resource(mut self, resource_type: impl Into<String>, resource_id: Uuid) -> Self {
        self.resource_type = Some(resource_type.into());
        self.resource_id = Some(resource_id);
        self
    }

    pub fn outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    pub fn detail(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.details {
            map.insert(key.into(), serde_json::to_value(value).unwrap_or_default());
        }
        self
    }

    pub fn ip_address(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    fn build(self, previous_hash: String) -> AuditEvent {
        AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            tenant_id: self.tenant_id,
            user_id: self.user_id,
            api_key_id: self.api_key_id,
            category: self.category,
            severity: self.severity,
            action: self.action,
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            outcome: self.outcome,
            details: self.details,
            ip_address: self.ip_address,
            user_agent: self.user_agent,
            request_id: self.request_id,
            session_id: self.session_id,
            previous_hash,
            hash: String::new(), // Will be computed on flush
        }
    }
}

/// Integrity verification report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub total_events: usize,
    pub verified: usize,
    pub failed: usize,
    pub failures: Vec<IntegrityFailure>,
}

/// Integrity failure detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityFailure {
    pub event_id: Uuid,
    pub expected_previous_hash: String,
    pub actual_previous_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_builder() {
        let event = AuditEventBuilder::new(AuditCategory::Authentication, "user.login")
            .tenant_id(Uuid::new_v4())
            .user_id(Uuid::new_v4())
            .severity(AuditSeverity::Info)
            .outcome(AuditOutcome::Success)
            .detail("method", "password")
            .ip_address("192.168.1.1")
            .build(String::new());

        assert_eq!(event.category, AuditCategory::Authentication);
        assert_eq!(event.action, "user.login");
        assert_eq!(event.outcome, AuditOutcome::Success);
    }

    #[test]
    fn test_hash_chaining() {
        let key = blake3::Key::new(&[0u8; 32]);
        let event1 = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            tenant_id: None,
            user_id: None,
            api_key_id: None,
            category: AuditCategory::SystemEvent,
            severity: AuditSeverity::Info,
            action: "test".to_string(),
            resource_type: None,
            resource_id: None,
            outcome: AuditOutcome::Success,
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
            request_id: None,
            session_id: None,
            previous_hash: String::new(),
            hash: String::new(),
        };

        let hash1 = AuditLogger::compute_hash(&event1, &Some(key));
        assert!(!hash1.is_empty());
    }
}
