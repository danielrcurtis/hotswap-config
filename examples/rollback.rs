//! Example demonstrating configuration rollback with version history.
//!
//! This example shows how to:
//! - Enable version history tracking
//! - Record configuration changes
//! - Rollback to previous versions
//! - Inspect version history
//!
//! Run with: cargo run --example rollback --features "yaml,rollback"

use hotswap_config::features::Rollback;
use hotswap_config::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AppConfig {
    version_name: String,
    port: u16,
    max_connections: u32,
    feature_enabled: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Rollback Example ===\n");

    // Create initial configuration
    let initial_config = AppConfig {
        version_name: "v1.0".to_string(),
        port: 8080,
        max_connections: 100,
        feature_enabled: false,
    };

    let config = HotswapConfig::new(initial_config);

    // Enable rollback support with max 10 versions
    let history = config.enable_history(10);
    println!("Enabled version history (max 10 versions)\n");

    // Wait for initial version to be recorded
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("Initial configuration (v1.0):");
    print_config(&config.get());
    println!("  History size: {}\n", history.len().await);

    // Make some configuration changes
    println!("--- Change 1: Upgrade to v1.1 ---");
    let v1_1 = AppConfig {
        version_name: "v1.1".to_string(),
        port: 8080,
        max_connections: 150,
        feature_enabled: false,
    };
    config.update(v1_1).await?;
    history
        .record(config.get(), Some("Increased max_connections".to_string()))
        .await;
    print_config(&config.get());
    println!("  History size: {}\n", history.len().await);

    println!("--- Change 2: Upgrade to v1.2 ---");
    let v1_2 = AppConfig {
        version_name: "v1.2".to_string(),
        port: 9090,
        max_connections: 150,
        feature_enabled: false,
    };
    config.update(v1_2).await?;
    history
        .record(config.get(), Some("Changed port to 9090".to_string()))
        .await;
    print_config(&config.get());
    println!("  History size: {}\n", history.len().await);

    println!("--- Change 3: Upgrade to v2.0 ---");
    let v2_0 = AppConfig {
        version_name: "v2.0".to_string(),
        port: 9090,
        max_connections: 200,
        feature_enabled: true,
    };
    config.update(v2_0).await?;
    history
        .record(
            config.get(),
            Some("Major version: enabled new feature".to_string()),
        )
        .await;
    print_config(&config.get());
    println!("  History size: {}\n", history.len().await);

    // Show version history
    println!("--- Version History ---");
    let versions = history.get_all().await;
    for version in &versions {
        println!(
            "Version {}: {} at {} - {}",
            version.version,
            version.config.version_name,
            version.timestamp.format("%H:%M:%S"),
            version.source.as_deref().unwrap_or("N/A")
        );
    }
    println!();

    // Rollback 1 step (from v2.0 to v1.2)
    println!("--- Rollback 1 step (v2.0 -> v1.2) ---");
    config.rollback(&history, 1).await?;
    println!("Rolled back!");
    print_config(&config.get());
    println!();

    // Rollback another step (from v1.2 to v1.1)
    println!("--- Rollback 1 more step (v1.2 -> v1.1) ---");
    config.rollback(&history, 1).await?;
    println!("Rolled back!");
    print_config(&config.get());
    println!();

    // Show recent versions
    println!("--- Recent 3 versions ---");
    let recent = history.get_recent(3).await;
    for version in recent {
        println!(
            "Version {}: {} - {}",
            version.version,
            version.config.version_name,
            version.source.as_deref().unwrap_or("N/A")
        );
    }
    println!();

    // Rollback to specific version (v2.0)
    println!("--- Rollback to specific version (v2.0) ---");
    // Find v2.0 version number
    let versions = history.get_all().await;
    let v2_version = versions
        .iter()
        .find(|v| v.config.version_name == "v2.0")
        .map(|v| v.version)
        .expect("v2.0 not found");

    config.rollback_to_version(&history, v2_version).await?;
    println!("Rolled back to version {}!", v2_version);
    print_config(&config.get());
    println!();

    println!("Example complete!");
    println!("\nKey benefits of rollback:");
    println!("  - Maintain version history automatically");
    println!("  - Rollback N steps or to specific version");
    println!("  - Bounded history size (oldest dropped)");
    println!("  - Audit trail with timestamps and descriptions");

    Ok(())
}

fn print_config(cfg: &AppConfig) {
    println!("  Version: {}", cfg.version_name);
    println!("  Port: {}", cfg.port);
    println!("  Max Connections: {}", cfg.max_connections);
    println!("  Feature Enabled: {}", cfg.feature_enabled);
}
