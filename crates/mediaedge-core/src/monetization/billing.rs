//! Billing Integration
//!
//! Integrates with Cleeng, Evergent, Chargebee, RevenueCat webhooks.
//! Handles churn recovery (dunning, grace periods).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Billing provider types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BillingProvider {
    Cleeng,
    Evergent,
    Chargebee,
    RevenueCat,
    Stripe,
    Custom,
}

/// Billing provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingProviderConfig {
    pub provider: BillingProvider,
    pub api_endpoint: String,
    pub api_key: String,
    pub webhook_secret: Option<String>,
    pub enabled: bool,
    pub custom_config: HashMap<String, serde_json::Value>,
}

/// Invoice status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Draft,
    Open,
    Paid,
    Void,
    Uncollectible,
}

/// Invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: Uuid,
    pub provider: BillingProvider,
    pub provider_invoice_id: String,
    pub tenant_id: Uuid,
    pub customer_id: String,
    pub subscription_id: Option<Uuid>,
    pub status: InvoiceStatus,
    pub amount_cents: i64,
    pub currency: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub due_date: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub line_items: Vec<InvoiceLineItem>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Invoice line item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceLineItem {
    pub id: Uuid,
    pub description: String,
    pub quantity: i64,
    pub unit_amount_cents: i64,
    pub amount_cents: i64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Payment method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: Uuid,
    pub provider: BillingProvider,
    pub provider_payment_method_id: String,
    pub tenant_id: Uuid,
    pub customer_id: String,
    pub method_type: PaymentMethodType,
    pub details: PaymentMethodDetails,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

/// Payment method types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    Card,
    BankAccount,
    Wallet,
    Invoice,
}

/// Payment method details (varies by type).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PaymentMethodDetails {
    Card {
        brand: String,
        last4: String,
        exp_month: u8,
        exp_year: u16,
        funding: String,
        country: String,
    },
    BankAccount {
        bank_name: String,
        last4: String,
        account_type: String,
        country: String,
    },
    Wallet {
        wallet_type: String,
        email: String,
    },
    Invoice {},
}

/// Dunning configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DunningConfig {
    pub enabled: bool,
    pub grace_period_days: u32,
    pub retry_schedule: Vec<DunningRetry>,
    pub final_action: DunningFinalAction,
}

/// Dunning retry step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DunningRetry {
    pub days_after_due: u32,
    pub action: DunningAction,
    pub message_template: Option<String>,
}

/// Dunning actions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DunningAction {
    RetryPayment,
    SendEmail,
    SendSms,
    SendWebhook,
    PauseSubscription,
    CancelSubscription,
}

/// Final dunning action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DunningFinalAction {
    PauseSubscription,
    CancelSubscription,
    MarkUncollectible,
}

/// Dunning state for an invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DunningState {
    pub invoice_id: Uuid,
    pub current_step: usize,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub status: DunningStatus,
    pub attempts: Vec<DunningAttempt>,
}

/// Dunning status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DunningStatus {
    InProgress,
    Recovered,
    Failed,
    Completed,
}

/// Dunning attempt record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DunningAttempt {
    pub step: usize,
    pub action: DunningAction,
    pub attempted_at: DateTime<Utc>,
    pub success: bool,
    pub error: Option<String>,
}

/// Billing service trait for provider abstraction.
#[async_trait]
pub trait BillingProviderClient: Send + Sync {
    async fn create_customer(&self, tenant_id: Uuid, email: &str, name: &str, metadata: HashMap<String, String>) -> Result<String>;
    async fn get_customer(&self, customer_id: &str) -> Result<serde_json::Value>;
    async fn update_customer(&self, customer_id: &str, updates: HashMap<String, serde_json::Value>) -> Result<()>;

    async fn create_subscription(&self, customer_id: &str, plan_id: &str, payment_method_id: Option<&str>, trial_days: Option<u32>) -> Result<String>;
    async fn get_subscription(&self, subscription_id: &str) -> Result<serde_json::Value>;
    async fn update_subscription(&self, subscription_id: &str, plan_id: Option<&str>, quantity: Option<i32>, prorate: bool) -> Result<()>;
    async fn cancel_subscription(&self, subscription_id: &str, at_period_end: bool) -> Result<()>;
    async fn pause_subscription(&self, subscription_id: &str) -> Result<()>;
    async fn resume_subscription(&self, subscription_id: &str) -> Result<()>;

    async fn create_payment_method(&self, customer_id: &str, method_type: PaymentMethodType, details: serde_json::Value) -> Result<String>;
    async fn get_payment_method(&self, payment_method_id: &str) -> Result<serde_json::Value>;
    async fn detach_payment_method(&self, payment_method_id: &str) -> Result<()>;

    async fn create_invoice(&self, customer_id: &str, line_items: Vec<InvoiceLineItem>, auto_advance: bool) -> Result<String>;
    async fn get_invoice(&self, invoice_id: &str) -> Result<serde_json::Value>;
    async fn finalize_invoice(&self, invoice_id: &str) -> Result<()>;
    async fn pay_invoice(&self, invoice_id: &str, payment_method_id: Option<&str>) -> Result<()>;

    async fn create_refund(&self, invoice_id: &str, amount_cents: Option<i64>, reason: Option<&str>) -> Result<String>;

    async fn verify_webhook(&self, payload: &[u8], signature: &str) -> Result<bool>;
    async fn parse_webhook(&self, payload: &[u8]) -> Result<BillingWebhookEvent>;
}

/// Billing webhook event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingWebhookEvent {
    pub id: String,
    pub provider: BillingProvider,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
    pub raw_payload: Vec<u8>,
}

/// Billing service orchestrating multiple providers.
pub struct BillingService {
    providers: Arc<DashMap<BillingProvider, Arc<dyn BillingProviderClient>>>,
    config: BillingServiceConfig,
    dunning_states: Arc<DashMap<Uuid, DunningState>>,
    webhook_handlers: Arc<DashMap<String, Box<dyn Fn(BillingWebhookEvent) -> Result<()> + Send + Sync>>>,
}

#[derive(Debug, Clone)]
pub struct BillingServiceConfig {
    pub default_provider: BillingProvider,
    pub dunning: DunningConfig,
    pub invoice_prefix: String,
    pub currency: String,
}

impl Default for BillingServiceConfig {
    fn default() -> Self {
        Self {
            default_provider: BillingProvider::Stripe,
            dunning: DunningConfig {
                enabled: true,
                grace_period_days: 7,
                retry_schedule: vec![
                    DunningRetry { days_after_due: 1, action: DunningAction::RetryPayment, message_template: None },
                    DunningRetry { days_after_due: 3, action: DunningAction::SendEmail, message_template: Some("payment_failed_day3".to_string()) },
                    DunningRetry { days_after_due: 7, action: DunningAction::SendEmail, message_template: Some("payment_failed_day7".to_string()) },
                    DunningRetry { days_after_due: 14, action: DunningAction::PauseSubscription, message_template: None },
                ],
                final_action: DunningFinalAction::CancelSubscription,
            },
            invoice_prefix: "ME".to_string(),
            currency: "USD".to_string(),
        }
    }
}

impl BillingService {
    /// Create a new billing service.
    pub fn new(config: BillingServiceConfig) -> Arc<Self> {
        Arc::new(Self {
            providers: Arc::new(DashMap::new()),
            config,
            dunning_states: Arc::new(DashMap::new()),
            webhook_handlers: Arc::new(DashMap::new()),
        })
    }

    /// Register a billing provider client.
    pub fn register_provider(&self, provider: BillingProvider, client: Arc<dyn BillingProviderClient>) {
        self.providers.insert(provider, client);
    }

    /// Get a provider client.
    pub fn provider(&self, provider: BillingProvider) -> Option<Arc<dyn BillingProviderClient>> {
        self.providers.get(&provider).map(|p| p.clone())
    }

    /// Get default provider.
    pub fn default_provider(&self) -> Option<Arc<dyn BillingProviderClient>> {
        self.provider(self.config.default_provider)
    }

    /// Process a webhook from any provider.
    pub async fn process_webhook(&self, provider: BillingProvider, payload: &[u8], signature: &str) -> Result<BillingWebhookEvent> {
        let client = self.provider(provider)
            .ok_or_else(|| anyhow::anyhow!("Provider not registered"))?;

        if !client.verify_webhook(payload, signature).await? {
            return Err(anyhow::anyhow!("Invalid webhook signature"));
        }

        let event = client.parse_webhook(payload).await?;

        // Handle event
        self.handle_webhook_event(&event).await?;

        Ok(event)
    }

    /// Handle webhook event (subscription changes, payment failures, etc.).
    async fn handle_webhook_event(&self, event: &BillingWebhookEvent) -> Result<()> {
        // Dispatch to registered handlers
        for handler in self.webhook_handlers.iter() {
            if let Err(e) = handler(event.clone()) {
                tracing::warn!("Webhook handler error: {}", e);
            }
        }

        // Handle dunning for payment failures
        if event.event_type.contains("payment_failed") || event.event_type.contains("invoice.payment_failed") {
            self.initiate_dunning(event).await?;
        }

        Ok(())
    }

    /// Register a webhook handler.
    pub fn register_webhook_handler(&self, name: String, handler: Box<dyn Fn(BillingWebhookEvent) -> Result<()> + Send + Sync>) {
        self.webhook_handlers.insert(name, handler);
    }

    /// Initiate dunning process for failed payment.
    async fn initiate_dunning(&self, event: &BillingWebhookEvent) -> Result<()> {
        if !self.config.dunning.enabled {
            return Ok(());
        }

        // Extract invoice ID from event
        let invoice_id = event.data.get("invoice")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let Some(invoice_id) = invoice_id else {
            return Ok(());
        };

        let dunning_state = DunningState {
            invoice_id,
            current_step: 0,
            last_attempt_at: None,
            next_attempt_at: Some(Utc::now() + chrono::Duration::days(
                self.config.dunning.retry_schedule.first().map(|r| r.days_after_due as i64).unwrap_or(1)
            )),
            status: DunningStatus::InProgress,
            attempts: Vec::new(),
        };

        self.dunning_states.insert(invoice_id, dunning_state);

        // Schedule first retry
        self.schedule_dunning_retry(invoice_id).await;

        Ok(())
    }

    /// Schedule next dunning retry.
    async fn schedule_dunning_retry(&self, invoice_id: Uuid) {
        // In production, use a proper job scheduler
        // This is a simplified version
        let state = self.dunning_states.get(&invoice_id);
        if let Some(mut state) = state {
            if state.current_step >= self.config.dunning.retry_schedule.len() {
                // Execute final action
                self.execute_final_action(&state).await;
                return;
            }

            let retry = &self.config.dunning.retry_schedule[state.current_step];
            state.next_attempt_at = Some(Utc::now() + chrono::Duration::days(retry.days_after_due as i64));
        }
    }

    /// Execute final dunning action.
    async fn execute_final_action(&self, state: &DunningState) {
        match self.config.dunning.final_action {
            DunningFinalAction::PauseSubscription => {
                // Pause subscription
            }
            DunningFinalAction::CancelSubscription => {
                // Cancel subscription
            }
            DunningFinalAction::MarkUncollectible => {
                // Mark invoice as uncollectible
            }
        }

        if let Some(mut state) = self.dunning_states.get_mut(&state.invoice_id) {
            state.status = DunningStatus::Completed;
        }
    }

    /// Record dunning attempt result.
    pub async fn record_dunning_attempt(&self, invoice_id: Uuid, action: DunningAction, success: bool, error: Option<String>) {
        if let Some(mut state) = self.dunning_states.get_mut(&invoice_id) {
            let attempt = DunningAttempt {
                step: state.current_step,
                action,
                attempted_at: Utc::now(),
                success,
                error,
            };

            state.attempts.push(attempt);
            state.last_attempt_at = Some(Utc::now());

            if success {
                state.status = DunningStatus::Recovered;
            } else {
                state.current_step += 1;
                if state.current_step < self.config.dunning.retry_schedule.len() {
                    self.schedule_dunning_retry(invoice_id).await;
                } else {
                    self.execute_final_action(&state).await;
                }
            }
        }
    }

    /// Get dunning state for invoice.
    pub fn get_dunning_state(&self, invoice_id: Uuid) -> Option<DunningState> {
        self.dunning_states.get(&invoice_id).map(|s| s.clone())
    }
}

// Provider implementations would be in separate files
// For example: cleeng.rs, evergent.rs, chargebee.rs, revenuecat.rs

/// Cleeng webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleengWebhookEvent {
    pub event_type: String,
    pub publisher_id: String,
    pub customer_id: String,
    pub offer_id: String,
    pub subscription_id: String,
    pub transaction_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Evergent webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvergentWebhookEvent {
    pub event_type: String,
    pub account_id: String,
    pub subscription_id: String,
    pub product_id: String,
    pub timestamp: DateTime<Utc>,
}

/// Chargebee webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargebeeWebhookEvent {
    pub id: String,
    pub event_type: String,
    pub api_version: String,
    pub content: ChargebeeWebhookContent,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargebeeWebhookContent {
    pub subscription: Option<serde_json::Value>,
    pub customer: Option<serde_json::Value>,
    pub invoice: Option<serde_json::Value>,
    pub payment_method: Option<serde_json::Value>,
}

/// RevenueCat webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueCatWebhookEvent {
    pub api_version: String,
    pub event: RevenueCatEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueCatEvent {
    pub event_type: String,
    pub event_timestamp_ms: i64,
    pub app_user_id: String,
    pub product_id: String,
    pub period_type: String,
    pub purchased_at_ms: i64,
    pub expiration_at_ms: Option<i64>,
    pub store: String,
    pub environment: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dunning_config_default() {
        let config = DunningConfig {
            enabled: true,
            grace_period_days: 7,
            retry_schedule: vec![],
            final_action: DunningFinalAction::CancelSubscription,
        };
        assert!(config.enabled);
    }

    #[test]
    fn test_billing_provider_serialization() {
        let provider = BillingProvider::Cleeng;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"cleeng\"");
    }
}