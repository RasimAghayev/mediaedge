//! Integration tests for mediaedge-config.

use mediaedge_config::{
    ConfigLoader, ConfigError, HotReloadState, hot_reload_routes,
    types::{GatewayConfig, ServiceConfig, ServerConfig, AuthConfig, AuthMode, JwtConfig},
    secrets::{SecretsManager, SecretsManagerConfig, EncryptedSecret, generate_age_keypair},
};
use tempfile::NamedTempFile;
use std::io::Write;
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode, HeaderMap, HeaderValue},
};
use tower::ServiceExt;

fn create_gateway_config_toml() -> String {
    r#"
service = "gateway"

[server]
host = "0.0.0.0"
port = 8080
request_timeout_secs = 30
max_body_size = 10485760
http2 = true

[auth]
default_mode = "jwt"

[auth.jwt]
algorithm = "HS256"
secret = "test-secret-key"
issuer = "mediaedge"
audience = "api"
clock_skew_secs = 60

[metrics]
enabled = true
path = "/metrics"

[metrics.otlp]
enabled = false
service_name = "mediaedge-gateway"
trace_sample_ratio = 0.1
metric_interval_secs = 60

[tracing]
enabled = true
level = "info"
format = "json"

[rate_limit]
enabled = true
requests_per_window = 1000
window_secs = 60
key_prefix = "rl:"

[protocols.rtmp]
enabled = true
port = 1935
chunk_size = 4096
ping_interval_secs = 30
max_stream_key_len = 128

[protocols.srt]
enabled = true
port = 8888
latency_ms = 120
encryption = false
stream_id_prefix = "stream/"

[protocols.rist]
enabled = true
port = 5000
profile = "simple"

[protocols.whip]
enabled = true
path = "/whip"
stun_servers = ["stun:stun.l.google.com:19302"]

[protocols.http]
hls_enabled = true
dash_enabled = true
segment_duration_secs = 6
playlist_window = 10

[admin]
enabled = true
path = "/api/v1/admin"
allowed_ips = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]
"#.to_string()
}

fn write_temp_config(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[tokio::test]
async fn test_load_gateway_config() {
    let file = write_temp_config(&create_gateway_config_toml());
    let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
    let config = loader.config();

    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.request_timeout_secs, 30);
    assert!(config.server.http2);

    assert_eq!(config.auth.default_mode, AuthMode::Jwt);
    assert!(config.auth.jwt.is_some());
    let jwt = config.auth.jwt.as_ref().unwrap();
    assert_eq!(jwt.algorithm, "HS256");
    assert_eq!(jwt.secret.as_deref(), Some("test-secret-key"));
    assert_eq!(jwt.issuer.as_deref(), Some("mediaedge"));
    assert_eq!(jwt.audience.as_deref(), Some("api"));
    assert_eq!(jwt.clock_skew_secs, 60);

    assert!(config.metrics.enabled);
    assert_eq!(config.metrics.path, "/metrics");
    assert!(!config.metrics.otlp.enabled);

    assert!(config.tracing.enabled);
    assert_eq!(config.tracing.level, "info");
    assert_eq!(config.tracing.format, "json");

    assert!(config.rate_limit.enabled);
    assert_eq!(config.rate_limit.requests_per_window, 1000);
    assert_eq!(config.rate_limit.window_secs, 60);

    assert!(config.protocols.rtmp.is_some());
    let rtmp = config.protocols.rtmp.as_ref().unwrap();
    assert!(rtmp.enabled);
    assert_eq!(rtmp.port, 1935);

    assert!(config.protocols.srt.is_some());
    assert!(config.protocols.rist.is_some());
    assert!(config.protocols.whip.is_some());
    assert!(config.protocols.http.is_some());

    assert!(config.admin.enabled);
    assert_eq!(config.admin.path, "/api/v1/admin");
}

#[tokio::test]
async fn test_env_override() {
    let file = write_temp_config(&create_gateway_config_toml());
    
    // Set env var to override port
    std::env::set_var("MEDIAEDGE_SERVER__PORT", "9090");
    
    let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
    assert_eq!(loader.config().server.port, 9090);
    
    std::env::remove_var("MEDIAEDGE_SERVER__PORT");
}

#[tokio::test]
async fn test_hot_reload() {
    let mut file = write_temp_config(&create_gateway_config_toml());
    let loader = Arc::new(ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap());
    
    let initial_version = loader.meta().version;
    assert_eq!(initial_version, 1);
    assert_eq!(loader.config().server.port, 8080);

    // Modify config file
    let new_config = create_gateway_config_toml().replace("port = 8080", "port = 9090");
    std::fs::write(file.path(), new_config).unwrap();

    // Reload
    let changed = loader.reload().unwrap();
    assert!(changed);
    assert_eq!(loader.meta().version, 2);
    assert_eq!(loader.config().server.port, 9090);
}

#[tokio::test]
async fn test_config_subscription() {
    let file = write_temp_config(&create_gateway_config_toml());
    let loader = Arc::new(ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap());
    let mut sub = loader.subscribe();

    // No change initially
    assert!(sub.check().is_none());

    // Modify and reload
    let new_config = create_gateway_config_toml().replace("port = 8080", "port = 9090");
    std::fs::write(file.path(), new_config).unwrap();
    loader.reload().unwrap();

    // Subscription should detect change
    let new_config = sub.check().unwrap();
    assert_eq!(new_config.server.port, 9090);
}

#[tokio::test]
async fn test_hot_reload_endpoint() {
    let file = write_temp_config(&create_gateway_config_toml());
    let loader = Arc::new(ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap());
    let state = HotReloadState {
        loader: loader.clone(),
        service_name: "gateway".to_string(),
    };

    let app = hot_reload_routes().with_state(state);

    // Test reload endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/config-reload")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], 1);
}

#[tokio::test]
async fn test_hot_reload_endpoint_with_auth() {
    let file = write_temp_config(&create_gateway_config_toml());
    let loader = Arc::new(ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap());
    
    // Set admin token
    std::env::set_var("MEDIAEDGE_ADMIN_TOKEN", "secret-token");
    
    let state = HotReloadState {
        loader: loader.clone(),
        service_name: "gateway".to_string(),
    };

    let app = hot_reload_routes().with_state(state);

    // Request without auth should fail
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/config-reload")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Request with auth should succeed
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer secret-token"));
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/config-reload")
                .header("authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    std::env::remove_var("MEDIAEDGE_ADMIN_TOKEN");
}

#[tokio::test]
async fn test_config_version_endpoint() {
    let file = write_temp_config(&create_gateway_config_toml());
    let loader = Arc::new(ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap());
    let state = HotReloadState {
        loader: loader.clone(),
        service_name: "gateway".to_string(),
    };

    let app = hot_reload_routes().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/config-version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["service"], "gateway");
}

#[tokio::test]
async fn test_validation_error() {
    let mut file = NamedTempFile::new().unwrap();
    let config = r#"
service = "gateway"

[server]
host = "0.0.0.0"
port = 99999  # Invalid port > 65535

[auth]
default_mode = "none"

[metrics]
enabled = true

[tracing]
enabled = true

[rate_limit]
enabled = true
"#;
    file.write_all(config.as_bytes()).unwrap();
    file.flush().unwrap();

    let result = ConfigLoader::<GatewayConfig>::from_file(file.path());
    assert!(result.is_err());
    
    match result.unwrap_err() {
        ConfigError::Validation(msg) => assert!(msg.contains("port")),
        _ => panic!("Expected validation error"),
    }
}

#[tokio::test]
async fn test_service_config_discriminant() {
    let gateway = ServiceConfig::Gateway(GatewayConfig::default());
    assert_eq!(gateway.service_name(), "gateway");
}

#[tokio::test]
async fn test_age_encryption() {
    let (identity, recipient) = generate_age_keypair().unwrap();
    assert!(identity.starts_with("AGE-SECRET-KEY-"));
    assert!(recipient.starts_with("age1"));

    let config = SecretsManagerConfig {
        default_method: "age".to_string(),
        age: Some(mediaedge_config::secrets::AgeConfig {
            recipient: recipient.clone(),
            identity: Some(identity),
            identity_file: None,
        }),
        kms: None,
        rotation_interval_hours: 0,
        cache_ttl_secs: 300,
    };

    let manager = SecretsManager::new(config).await.unwrap();
    
    let secret = manager.encrypt_secret("test-key", "super-secret-value").await.unwrap();
    assert_eq!(secret.method, "age");
    assert!(!secret.is_plaintext());

    let decrypted = manager.decrypt_age(&secret).await.unwrap();
    assert_eq!(decrypted, "super-secret-value");
}

#[tokio::test]
async fn test_plaintext_secret() {
    let secret = EncryptedSecret::plaintext("test-value");
    assert_eq!(secret.method, "plaintext");
    assert!(secret.is_plaintext());
    
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &secret.payload).unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), "test-value");
}

#[tokio::test]
async fn test_secrets_manager_cache() {
    let config = SecretsManagerConfig::default();
    let manager = SecretsManager::new(config).await.unwrap();

    // Set env var for testing
    std::env::set_var("MEDIAEDGE_SECRET_TEST_KEY", "env-value");
    
    let value = manager.get_secret("test_key").await.unwrap();
    assert_eq!(value, "env-value");

    let stats = manager.cache_stats().await;
    assert_eq!(stats.0, 1);

    std::env::remove_var("MEDIAEDGE_SECRET_TEST_KEY");
}

#[tokio::test]
async fn test_generate_json_schema() {
    let schema = mediaedge_config::generate_json_schema::<GatewayConfig>().unwrap();
    let json: serde_json::Value = serde_json::from_str(&schema).unwrap();
    assert!(json["type"] == "object");
    assert!(json["properties"]["server"].is_object());
}