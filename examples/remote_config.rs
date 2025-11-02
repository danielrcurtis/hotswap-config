//! Example demonstrating remote HTTP configuration loading.
//!
//! This example shows how to load configuration from a remote HTTP endpoint
//! with authentication, error handling, and fallback behavior.

use hotswap_config::prelude::*;
#[cfg(feature = "remote")]
use hotswap_config::sources::HttpSource;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    features: FeaturesConfig,
}

#[derive(Debug, Deserialize, Clone)]
struct ServerConfig {
    port: u16,
    host: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
struct FeaturesConfig {
    enable_metrics: bool,
    enable_caching: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Remote Configuration Example ===\n");

    // Example 1: Basic HTTP source
    println!("Example 1: Basic HTTP configuration source");
    println!("-------------------------------------------");

    // NOTE: This is a mock endpoint. In a real application, replace with your config server URL.
    let http_source = HttpSource::builder()
        .with_url("https://config.example.com/api/config")
        .with_auth_token("your-secret-token")
        .with_timeout(Duration::from_secs(10))
        .with_priority(250) // Higher than files, lower than env vars
        .build()?;

    println!("✓ HTTP source configured");
    println!("  URL: https://config.example.com/api/config");
    println!("  Auth: Bearer token");
    println!("  Timeout: 10s");
    println!("  Priority: 250");
    println!();

    // Example 2: Hybrid configuration (local files + remote)
    println!("Example 2: Hybrid configuration (local + remote)");
    println!("------------------------------------------------");

    // Create a temporary config file for demonstration
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().join("default.yaml");
    std::fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: "localhost"

database:
  url: "postgresql://localhost/mydb"
  max_connections: 10

features:
  enable_metrics: false
  enable_caching: true
"#,
    )?;

    // Build configuration with both file and remote sources
    // Remote source has higher priority, so it can override file values
    let config = HotswapConfig::builder()
        .with_file(&config_path) // Priority 100
        // .with_source(http_source) // Priority 250 - would override file values
        .with_env_overrides("APP", "__") // Priority 300 - overrides everything
        .build::<AppConfig>()
        .await?;

    println!("✓ Configuration loaded successfully");
    println!();

    // Example 3: Using the configuration
    println!("Example 3: Accessing configuration values");
    println!("------------------------------------------");

    let cfg = config.get();
    println!("Server Configuration:");
    println!("  Host: {}", cfg.server.host);
    println!("  Port: {}", cfg.server.port);
    println!();

    println!("Database Configuration:");
    println!("  URL: {}", cfg.database.url);
    println!("  Max Connections: {}", cfg.database.max_connections);
    println!();

    println!("Feature Flags:");
    println!("  Metrics Enabled: {}", cfg.features.enable_metrics);
    println!("  Caching Enabled: {}", cfg.features.enable_caching);
    println!();

    // Example 4: Error handling and resilience
    println!("Example 4: Error handling best practices");
    println!("-----------------------------------------");

    println!("✓ Remote sources cache last-known-good configuration");
    println!("✓ On network errors, the cached config is used");
    println!("✓ Parse errors still fail (to prevent silent corruption)");
    println!("✓ HTTP errors (4xx, 5xx) keep last good config");
    println!();

    // Example 5: Configuration precedence
    println!("Example 5: Configuration precedence order");
    println!("-----------------------------------------");
    println!("1. Environment variables (priority 300) - highest");
    println!("2. Remote HTTP source (priority 250)");
    println!("3. Local files (priority 100-200)");
    println!("4. Default values (priority 0) - lowest");
    println!();

    println!("Try overriding with environment variables:");
    println!("  export APP_SERVER__PORT=9090");
    println!("  export APP_FEATURES__ENABLE_METRICS=true");
    println!();

    // Example 6: Basic auth alternative
    println!("Example 6: Using Basic authentication");
    println!("--------------------------------------");

    let _http_source_basic = HttpSource::builder()
        .with_url("https://config.example.com/api/config")
        .with_basic_auth("username", "password")
        .build()?;

    println!("✓ HTTP source with Basic auth configured");
    println!();

    println!("=== Example Complete ===");
    println!();
    println!("Note: This example uses a local file because the remote endpoint");
    println!("is mock. In production, uncomment the .with_source(http_source)");
    println!("line to enable remote configuration fetching.");

    Ok(())
}
