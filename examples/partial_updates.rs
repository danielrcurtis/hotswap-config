//! Example demonstrating partial configuration updates using JSON Patch.
//!
//! This example shows how to:
//! - Apply JSON Patch operations to configuration
//! - Update individual fields without full reload
//! - Perform multiple atomic updates
//!
//! Run with: cargo run --example partial_updates --features "yaml,partial-updates"

use hotswap_config::features::PartialUpdate;
use hotswap_config::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    features: FeatureFlags,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ServerConfig {
    port: u16,
    host: String,
    workers: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct DatabaseConfig {
    url: String,
    pool_size: u32,
    timeout_seconds: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FeatureFlags {
    caching: bool,
    rate_limiting: bool,
    metrics: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Partial Updates (JSON Patch) Example ===\n");

    // Create initial configuration
    let initial_config = AppConfig {
        server: ServerConfig {
            port: 8080,
            host: "localhost".to_string(),
            workers: 4,
        },
        database: DatabaseConfig {
            url: "postgres://localhost/mydb".to_string(),
            pool_size: 10,
            timeout_seconds: 30,
        },
        features: FeatureFlags {
            caching: false,
            rate_limiting: false,
            metrics: false,
        },
    };

    let config = HotswapConfig::new(initial_config);

    println!("Initial configuration:");
    print_config(&config.get());

    // Example 1: Update a single field
    println!("\n--- Example 1: Update port using update_field() ---");
    config.update_field("/server/port", 9090).await?;
    println!("Changed server port to 9090");
    print_config(&config.get());

    // Example 2: Update multiple fields with a patch
    println!("\n--- Example 2: Apply multi-field patch ---");
    let patch = json!([
        { "op": "replace", "path": "/server/host", "value": "0.0.0.0" },
        { "op": "replace", "path": "/server/workers", "value": 8 }
    ]);
    config.apply_patch(patch).await?;
    println!("Applied patch: changed host to 0.0.0.0 and workers to 8");
    print_config(&config.get());

    // Example 3: Update nested fields
    println!("\n--- Example 3: Update nested database fields ---");
    config.update_field("/database/pool_size", 20).await?;
    config.update_field("/database/timeout_seconds", 60).await?;
    println!("Updated database pool_size to 20 and timeout to 60s");
    print_config(&config.get());

    // Example 4: Toggle feature flags
    println!("\n--- Example 4: Enable feature flags ---");
    let patch = json!([
        { "op": "replace", "path": "/features/caching", "value": true },
        { "op": "replace", "path": "/features/metrics", "value": true }
    ]);
    config.apply_patch(patch).await?;
    println!("Enabled caching and metrics features");
    print_config(&config.get());

    // Example 5: Comprehensive update
    println!("\n--- Example 5: Comprehensive configuration update ---");
    let patch = json!([
        { "op": "replace", "path": "/server/port", "value": 8443 },
        { "op": "replace", "path": "/database/url", "value": "postgres://prod-db/mydb" },
        { "op": "replace", "path": "/features/rate_limiting", "value": true }
    ]);
    config.apply_patch(patch).await?;
    println!("Applied production-ready configuration");
    print_config(&config.get());

    println!("\nExample complete!");
    println!("\nKey benefits of partial updates:");
    println!("  - No need to reload entire config from files");
    println!("  - Surgical updates to specific fields");
    println!("  - Multiple fields updated atomically");
    println!("  - Full validation still applied");

    Ok(())
}

fn print_config(cfg: &AppConfig) {
    println!("  Server:");
    println!("    Host: {}", cfg.server.host);
    println!("    Port: {}", cfg.server.port);
    println!("    Workers: {}", cfg.server.workers);
    println!("  Database:");
    println!("    URL: {}", cfg.database.url);
    println!("    Pool Size: {}", cfg.database.pool_size);
    println!("    Timeout: {}s", cfg.database.timeout_seconds);
    println!("  Features:");
    println!("    Caching: {}", cfg.features.caching);
    println!("    Rate Limiting: {}", cfg.features.rate_limiting);
    println!("    Metrics: {}", cfg.features.metrics);
}
