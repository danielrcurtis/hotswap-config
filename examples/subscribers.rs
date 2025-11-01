//! Example demonstrating the subscriber/notification system.
//!
//! This example shows how to:
//! - Subscribe to configuration changes
//! - Receive notifications on updates
//! - Unsubscribe by dropping handles
//!
//! Run with: cargo run --example subscribers --features yaml

use hotswap_config::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    port: u16,
    feature_flags: FeatureFlags,
}

#[derive(Debug, Deserialize, Clone)]
struct FeatureFlags {
    new_ui: bool,
    beta_features: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Subscriber/Notification Example ===\n");

    // Create initial configuration
    let initial_config = AppConfig {
        port: 8080,
        feature_flags: FeatureFlags {
            new_ui: false,
            beta_features: false,
        },
    };

    let config = HotswapConfig::new(initial_config);

    // Track notification counts
    let notifications = Arc::new(AtomicUsize::new(0));

    // Subscribe multiple handlers
    println!("Subscribing multiple handlers...\n");

    let notifications_clone = Arc::clone(&notifications);
    let handle1 = config
        .subscribe(move || {
            let count = notifications_clone.fetch_add(1, Ordering::SeqCst) + 1;
            println!(
                "[Handler 1] Configuration changed! (notification #{})",
                count
            );
        })
        .await;

    let handle2 = config
        .subscribe(|| {
            println!("[Handler 2] Detected configuration update");
        })
        .await;

    let handle3 = config
        .subscribe(|| {
            println!("[Handler 3] Configuration notification received");
        })
        .await;

    println!("Subscribed 3 handlers\n");

    // Make some configuration updates
    println!("--- Update 1: Changing port to 9090 ---");
    let new_config = AppConfig {
        port: 9090,
        feature_flags: FeatureFlags {
            new_ui: false,
            beta_features: false,
        },
    };
    config.update(new_config).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("\n--- Update 2: Enabling new_ui feature ---");
    let new_config = AppConfig {
        port: 9090,
        feature_flags: FeatureFlags {
            new_ui: true,
            beta_features: false,
        },
    };
    config.update(new_config).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Unsubscribe handler 2
    println!("\n--- Unsubscribing Handler 2 ---");
    drop(handle2);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("\n--- Update 3: Enabling beta_features ---");
    let new_config = AppConfig {
        port: 9090,
        feature_flags: FeatureFlags {
            new_ui: true,
            beta_features: true,
        },
    };
    config.update(new_config).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Show current state
    let cfg = config.get();
    println!("\nFinal configuration:");
    println!("  Port: {}", cfg.port);
    println!("  New UI: {}", cfg.feature_flags.new_ui);
    println!("  Beta Features: {}", cfg.feature_flags.beta_features);

    let total_notifications = notifications.load(Ordering::SeqCst);
    println!("\nTotal notifications received: {}", total_notifications);

    // Clean up remaining handles
    drop(handle1);
    drop(handle3);

    println!("\nAll handlers unsubscribed");
    println!("Example complete!");

    Ok(())
}
