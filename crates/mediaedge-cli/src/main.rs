//! MediaEdge CLI (mediaedgectl)
//!
//! Command-line interface for managing MediaEdge resources.

use clap::{Parser, Subcommand};
use mediaedge_client_rust::{MediaEdgeClient, models::*};
use anyhow::{Result, Context};
use std::path::PathBuf;
use dirs;
use toml;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "mediaedgectl", version, about = "MediaEdge CLI - Manage your MediaEdge deployment")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// API endpoint URL
    #[arg(long, env = "MEDIAEDGE_API_URL", default_value = "http://localhost:8080")]
    api_url: String,

    /// Access token
    #[arg(long, env = "MEDIAEDGE_ACCESS_TOKEN")]
    access_token: Option<String>,

    /// Config file path
    #[arg(long, env = "MEDIAEDGE_CONFIG")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream management
    #[command(subcommand)]
    Stream(StreamCommands),

    /// Cluster management
    #[command(subcommand)]
    Cluster(ClusterCommands),

    /// Tenant management
    #[command(subcommand)]
    Tenant(TenantCommands),

    /// Monetization management
    #[command(subcommand)]
    Monetization(MonetizationCommands),

    /// Authentication
    #[command(subcommand)]
    Auth(AuthCommands),

    /// Metrics and monitoring
    #[command(subcommand)]
    Metrics(MetricsCommands),

    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
enum StreamCommands {
    /// List streams
    List {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
        #[arg(long)]
        search: Option<String>,
    },
    /// Get stream details
    Get { id: String },
    /// Create a new stream
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        slug: Option<String>,
        #[arg(long, value_enum, default_value = "rtmp")]
        protocol: IngestProtocol,
        #[arg(long)]
        transcoding_profile: Option<String>,
    },
    /// Update a stream
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        transcoding_profile: Option<String>,
    },
    /// Delete a stream
    Delete { id: String },
    /// Start a stream
    Start { id: String },
    /// Stop a stream
    Stop { id: String },
    /// Get stream metrics
    Metrics {
        id: String,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
    },
    /// Get playback URLs
    Playback { id: String },
}

#[derive(Subcommand)]
enum ClusterCommands {
    /// List nodes
    Nodes {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    /// Get node details
    Node { id: String },
    /// List broadcasts
    Broadcasts {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    /// Get cluster health
    Health,
    /// Get federation status
    Federation,
    /// Get mesh status
    Mesh,
    /// Trigger config sync
    SyncConfig,
}

#[derive(Subcommand)]
enum TenantCommands {
    /// List tenants
    List {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
        #[arg(long)]
        search: Option<String>,
    },
    /// Get tenant details
    Get { id: String },
    /// Create a new tenant
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        slug: Option<String>,
    },
    /// Update a tenant
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a tenant
    Delete { id: String },
    /// Get tenant quotas
    Quotas { id: String },
    /// Update tenant quotas
    UpdateQuotas {
        id: String,
        #[arg(long)]
        max_streams: Option<u32>,
        #[arg(long)]
        max_concurrent_viewers: Option<u64>,
        #[arg(long)]
        max_storage_gb: Option<u64>,
    },
}

#[derive(Subcommand)]
enum MonetizationCommands {
    /// FAST Channels
    #[command(subcommand)]
    Fast(FastCommands),
    /// TVOD Products
    #[command(subcommand)]
    Tvod(TvodCommands),
    /// Bundles
    #[command(subcommand)]
    Bundle(BundleCommands),
    /// Entitlements
    #[command(subcommand)]
    Entitlement(EntitlementCommands),
    /// Billing
    #[command(subcommand)]
    Billing(BillingCommands),
}

#[derive(Subcommand)]
enum FastCommands {
    List {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    Get { id: String },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        slug: Option<String>,
    },
    Update { id: String },
    Delete { id: String },
}

#[derive(Subcommand)]
enum TvodCommands {
    List {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    Get { id: String },
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        content_id: String,
    },
    Update { id: String },
    Delete { id: String },
}

#[derive(Subcommand)]
enum BundleCommands {
    List {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    Get { id: String },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        bundle_type: BundleType,
    },
    Update { id: String },
    Delete { id: String },
}

#[derive(Subcommand)]
enum EntitlementCommands {
    Check {
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        content_id: String,
        #[arg(long, value_enum)]
        content_type: ContentType,
    },
    List {
        #[arg(long)]
        user_id: String,
    },
}

#[derive(Subcommand)]
enum BillingCommands {
    Invoices {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    Get { id: String },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Login
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
        #[arg(long)]
        mfa_code: Option<String>,
    },
    /// Refresh token
    Refresh,
    /// Get current user
    Me,
    /// List users (admin)
    Users {
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "20")]
        per_page: u32,
    },
    /// Invite user (admin)
    Invite {
        #[arg(long)]
        email: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        roles: Vec<String>,
        #[arg(long)]
        tenant_id: String,
    },
}

#[derive(Subcommand)]
enum MetricsCommands {
    /// Query Prometheus
    Query {
        query: String,
        #[arg(long)]
        time: Option<String>,
    },
    /// Query range
    QueryRange {
        #[arg(long)]
        metric: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: String,
        #[arg(long)]
        step: Option<String>,
    },
    /// Stream metrics
    Stream {
        id: String,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
    },
    /// Cluster metrics
    Cluster,
    /// Tenant metrics
    Tenant { id: String },
    /// Anomaly alerts
    Anomalies,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current config
    Show,
    /// Set config value
    Set { key: String, value: String },
    /// Initialize config file
    Init,
}

#[derive(Debug, Deserialize, Serialize)]
struct ConfigFile {
    api_url: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    default_tenant: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config
    let config = load_config(cli.config.as_deref()).await?;

    // Determine API URL and token
    let api_url = cli.api_url
        .or(config.api_url)
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    let access_token = cli.access_token.or(config.access_token);

    // Create client
    let mut client = MediaEdgeClient::new(&api_url)?;
    if let Some(token) = access_token {
        client.set_access_token(token);
    }

    // Execute command
    match cli.command {
        Commands::Stream(cmd) => handle_stream(cmd, &client).await?,
        Commands::Cluster(cmd) => handle_cluster(cmd, &client).await?,
        Commands::Tenant(cmd) => handle_tenant(cmd, &client).await?,
        Commands::Monetization(cmd) => handle_monetization(cmd, &client).await?,
        Commands::Auth(cmd) => handle_auth(cmd, &client).await?,
        Commands::Metrics(cmd) => handle_metrics(cmd, &client).await?,
        Commands::Config(cmd) => handle_config(cmd, &config).await?,
    }

    Ok(())
}

async fn load_config(path: Option<&PathBuf>) -> Result<ConfigFile> {
    let config_path = path.map(|p| p.to_path_buf())
        .or_else(|| dirs::config_dir().map(|d| d.join("mediaedge").join("config.toml")));

    if let Some(path) = config_path {
        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            return Ok(toml::from_str(&content)?);
        }
    }
    Ok(ConfigFile::default())
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            api_url: None,
            access_token: None,
            refresh_token: None,
            default_tenant: None,
        }
    }
}

// Stream handlers
async fn handle_stream(cmd: StreamCommands, client: &MediaEdgeClient) -> Result<()> {
    use mediaedge_client_rust::prelude::*;
    use uuid::Uuid;

    match cmd {
        StreamCommands::List { page, per_page, search } => {
            let params = ListParams { page: Some(page), per_page: Some(per_page), search, ..Default::default() };
            let result = client.streams().list(Some(params)).await?;
            print_table(result.data, |s: &Stream| vec![
                s.id.to_string(),
                s.name.clone(),
                s.slug.clone(),
                format!("{:?}", s.status),
                format!("{:?}", s.ingest_protocol),
                s.created_at.to_rfc3339(),
            ], &["ID", "Name", "Slug", "Status", "Protocol", "Created"])?;
        }
        StreamCommands::Get { id } => {
            let stream = client.streams().get(Uuid::parse_str(&id)?).await?;
            print_json(&stream)?;
        }
        StreamCommands::Create { name, slug, protocol, transcoding_profile } => {
            let req = CreateStreamRequest {
                name,
                slug,
                ingest_protocol: protocol,
                transcoding_profile,
                recording_enabled: None,
                dvr_enabled: None,
            };
            let stream = client.streams().create(req).await?;
            print_json(&stream)?;
        }
        StreamCommands::Update { id, name, transcoding_profile } => {
            let req = UpdateStreamRequest { name, transcoding_profile, recording_enabled: None, dvr_enabled: None };
            let stream = client.streams().update(Uuid::parse_str(&id)?, req).await?;
            print_json(&stream)?;
        }
        StreamCommands::Delete { id } => {
            client.streams().delete(Uuid::parse_str(&id)?).await?;
            println!("Stream deleted");
        }
        StreamCommands::Start { id } => {
            let stream = client.streams().start(Uuid::parse_str(&id)?).await?;
            print_json(&stream)?;
        }
        StreamCommands::Stop { id } => {
            let stream = client.streams().stop(Uuid::parse_str(&id)?).await?;
            print_json(&stream)?;
        }
        StreamCommands::Metrics { id, start, end } => {
            let params = MetricsQueryParams {
                start: start.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                end: end.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                interval: None,
            };
            let metrics = client.streams().metrics(Uuid::parse_str(&id)?, Some(params)).await?;
            print_table(metrics, |m: &StreamMetrics| vec![
                m.timestamp.to_rfc3339(),
                m.viewers_current.to_string(),
                m.viewers_peak.to_string(),
                format!("{} kbps", m.bitrate_in / 1000),
                format!("{} kbps", m.bitrate_out / 1000),
            ], &["Time", "Current Viewers", "Peak Viewers", "Bitrate In", "Bitrate Out"])?;
        }
        StreamCommands::Playback { id } => {
            let urls = client.streams().playback_urls(Uuid::parse_str(&id)?).await?;
            print_json(&urls)?;
        }
    }
    Ok(())
}

// Cluster handlers
async fn handle_cluster(cmd: ClusterCommands, client: &MediaEdgeClient) -> Result<()> {
    use mediaedge_client_rust::prelude::*;
    use uuid::Uuid;

    match cmd {
        ClusterCommands::Nodes { page, per_page } => {
            let params = ListParams { page: Some(page), per_page: Some(per_page), ..Default::default() };
            let result = client.cluster().nodes(Some(params)).await?;
            print_table(result.data, |n: &ClusterNode| vec![
                n.id.to_string(),
                n.name.clone(),
                n.endpoint.clone(),
                n.region.clone(),
                format!("{:?}", n.status),
                format!("{:.1}%", n.load.cpu_percent),
                n.capacity.max_streams.to_string(),
            ], &["ID", "Name", "Endpoint", "Region", "Status", "CPU%", "Max Streams"])?;
        }
        ClusterCommands::Node { id } => {
            let node = client.cluster().node(Uuid::parse_str(&id)?).await?;
            print_json(&node)?;
        }
        ClusterCommands::Broadcasts { page, per_page } => {
            let params = ListParams { page: Some(page), per_page: Some(per_page), ..Default::default() };
            let result = client.cluster().broadcasts(Some(params)).await?;
            print_table(result.data, |b: &Broadcast| vec![
                b.id.to_string(),
                b.stream_id.to_string(),
                b.owner_node_id.to_string(),
                format!("{:?}", b.status),
                b.viewers.to_string(),
            ], &["ID", "Stream ID", "Owner Node", "Status", "Viewers"])?;
        }
        ClusterCommands::Health => {
            let health = client.cluster().health().await?;
            print_json(&health)?;
        }
        ClusterCommands::Federation => {
            let fed = client.cluster().federation().await?;
            print_json(&fed)?;
        }
        ClusterCommands::Mesh => {
            let mesh = client.cluster().mesh().await?;
            print_json(&mesh)?;
        }
        ClusterCommands::SyncConfig => {
            let result = client.cluster().sync_config().await?;
            print_json(&result)?;
        }
    }
    Ok(())
}

// Tenant handlers
async fn handle_tenant(cmd: TenantCommands, client: &MediaEdgeClient) -> Result<()> {
    use mediaedge_client_rust::prelude::*;
    use uuid::Uuid;

    match cmd {
        TenantCommands::List { page, per_page, search } => {
            let params = ListParams { page: Some(page), per_page: Some(per_page), search, ..Default::default() };
            let result = client.tenants().list(Some(params)).await?;
            print_table(result.data, |t: &Tenant| vec![
                t.id.to_string(),
                t.name.clone(),
                t.slug.clone(),
                format!("{:?}", t.status),
                t.quotas.max_streams.to_string(),
                t.quotas.max_concurrent_viewers.to_string(),
            ], &["ID", "Name", "Slug", "Status", "Max Streams", "Max Viewers"])?;
        }
        TenantCommands::Get { id } => {
            let tenant = client.tenants().get(Uuid::parse_str(&id)?).await?;
            print_json(&tenant)?;
        }
        TenantCommands::Create { name, slug } => {
            let req = CreateTenantRequest { name, slug, quotas: None, branding: None, settings: None };
            let tenant = client.tenants().create(req).await?;
            print_json(&tenant)?;
        }
        TenantCommands::Update { id, name } => {
            let req = UpdateTenantRequest { name, quotas: None, branding: None, settings: None };
            let tenant = client.tenants().update(Uuid::parse_str(&id)?, req).await?;
            print_json(&tenant)?;
        }
        TenantCommands::Delete { id } => {
            client.tenants().delete(Uuid::parse_str(&id)?).await?;
            println!("Tenant deleted");
        }
        TenantCommands::Quotas { id } => {
            let quotas = client.tenants().quotas(Uuid::parse_str(&id)?).await?;
            print_json(&quotas)?;
        }
        TenantCommands::UpdateQuotas { id, max_streams, max_concurrent_viewers, max_storage_gb } => {
            let current = client.tenants().quotas(Uuid::parse_str(&id)?).await?;
            let quotas = TenantQuotas {
                max_streams: max_streams.unwrap_or(current.max_streams),
                max_concurrent_viewers: max_concurrent_viewers.unwrap_or(current.max_concurrent_viewers),
                max_storage_gb: max_storage_gb.unwrap_or(current.max_storage_gb),
                max_bandwidth_gbps: current.max_bandwidth_gbps,
                max_transcoding_hours: current.max_transcoding_hours,
                custom_domains: current.custom_domains,
            };
            let updated = client.tenants().update_quotas(Uuid::parse_str(&id)?, quotas).await?;
            print_json(&updated)?;
        }
    }
    Ok(())
}

// Monetization handlers (stubs)
async fn handle_monetization(cmd: MonetizationCommands, _client: &MediaEdgeClient) -> Result<()> {
    println!("Monetization command: {:?}", cmd);
    Ok(())
}

// Auth handlers
async fn handle_auth(cmd: AuthCommands, client: &MediaEdgeClient) -> Result<()> {
    use mediaedge_client_rust::prelude::*;
    use uuid::Uuid;

    match cmd {
        AuthCommands::Login { email, password, mfa_code } => {
            let req = LoginRequest { email, password, mfa_code };
            let resp = client.auth().login(req).await?;
            print_json(&resp)?;
            println!("\nSet MEDIAEDGE_ACCESS_TOKEN={}", resp.access_token);
        }
        AuthCommands::Me => {
            let user = client.auth().me().await?;
            print_json(&user)?;
        }
        AuthCommands::Users { page, per_page } => {
            let params = ListParams { page: Some(page), per_page: Some(per_page), ..Default::default() };
            let result = client.auth().users(Some(params)).await?;
            print_table(result.data, |u: &User| vec![
                u.id.to_string(),
                u.name.clone(),
                u.email.clone(),
                u.roles.join(","),
                format!("{:?}", u.status),
            ], &["ID", "Name", "Email", "Roles", "Status"])?;
        }
        AuthCommands::Invite { email, name, roles, tenant_id } => {
            let req = InviteUserRequest { email, name, roles, tenant_id: Uuid::parse_str(&tenant_id)? };
            let user = client.auth().invite_user(req).await?;
            print_json(&user)?;
        }
        _ => println!("Auth command not fully implemented: {:?}", cmd),
    }
    Ok(())
}

// Metrics handlers (stubs)
async fn handle_metrics(cmd: MetricsCommands, _client: &MediaEdgeClient) -> Result<()> {
    println!("Metrics command: {:?}", cmd);
    Ok(())
}

// Config handlers
async fn handle_config(cmd: ConfigCommands, config: &ConfigFile) -> Result<()> {
    match cmd {
        ConfigCommands::Show => {
            print_json(config)?;
        }
        ConfigCommands::Init => {
            let config_dir = dirs::config_dir()
                .context("Could not find config directory")?
                .join("mediaedge");
            tokio::fs::create_dir_all(&config_dir).await?;
            let config_path = config_dir.join("config.toml");
            let default_config = ConfigFile::default();
            let content = toml::to_string_pretty(&default_config)?;
            tokio::fs::write(&config_path, content).await?;
            println!("Config initialized at: {}", config_path.display());
        }
        ConfigCommands::Set { key, value } => {
            println!("Set {} = {}", key, value);
        }
    }
    Ok(())
}

// Helper functions
use tabled::{Table, Tabled};
use chrono::{DateTime, Utc};

fn print_table<T, F>(items: Vec<T>, row_fn: F, headers: &[&str]) -> Result<()>
where
    F: Fn(&T) -> Vec<String>,
{
    if items.is_empty() {
        println!("No results");
        return Ok(());
    }

    let mut table = Table::new(items.iter().map(|item| {
        let row = row_fn(item);
        Row::new(row)
    }));

    table.with(tabled::settings::Style::modern());
    println!("{}", table);
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[derive(Tabled)]
struct Row {
    #[tabled(rename = "")]
    cols: Vec<String>,
}

impl Row {
    fn new(cols: Vec<String>) -> Self {
        Self { cols }
    }
}

// Need to add these imports
use mediaedge_client_rust::models::{ListParams, MetricsQueryParams};
use chrono::DateTime;