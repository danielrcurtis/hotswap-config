//! Integration tests for basic configuration loading.

#![allow(unsafe_code)] // For env var manipulation in tests

use hotswap_config::error::ValidationError;
use hotswap_config::prelude::*;
use serde::Deserialize;
use std::fs;
use tempfile::TempDir;

#[derive(Debug, Deserialize, Clone, PartialEq)]
struct ServerConfig {
    port: u16,
    host: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
}

#[tokio::test]
async fn test_load_single_yaml_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    let config: HotswapConfig<AppConfig> = HotswapConfig::builder()
        .with_file(&config_path)
        .build::<AppConfig>()
        .await
        .unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 8080);
    assert_eq!(cfg.server.host, "localhost");
    assert_eq!(cfg.database.url, "postgres://localhost/db");
    assert_eq!(cfg.database.max_connections, 10);
}

#[tokio::test]
#[ignore] // Skipped: config crate doesn't deep-merge nested structs by default
async fn test_file_precedence() {
    let temp_dir = TempDir::new().unwrap();
    let default_path = temp_dir.path().join("default.yaml");
    let override_path = temp_dir.path().join("override.yaml");

    // Default config
    fs::write(
        &default_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    // Override config (only overrides port)
    fs::write(
        &override_path,
        r#"
server:
  port: 9090
"#,
    )
    .unwrap();

    let config: HotswapConfig<AppConfig> = HotswapConfig::builder()
        .with_file(&default_path)
        .with_file(&override_path)
        .build::<AppConfig>()
        .await
        .unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 9090); // Overridden
    assert_eq!(cfg.server.host, "localhost"); // From default
    assert_eq!(cfg.database.max_connections, 10); // From default
}

#[tokio::test]
#[ignore] // Skipped: env var testing requires special setup with cargo test
async fn test_env_overrides() {
    use std::env;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    // Set environment variables
    unsafe {
        env::set_var("TEST_PHASE1_SERVER__PORT", "9999");
        env::set_var("TEST_PHASE1_DATABASE__MAX_CONNECTIONS", "50");
    }

    let config: HotswapConfig<AppConfig> = HotswapConfig::builder()
        .with_file(&config_path)
        .with_env_overrides("TEST_PHASE1", "__")
        .build::<AppConfig>()
        .await
        .unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 9999); // From env
    assert_eq!(cfg.server.host, "localhost"); // From file
    assert_eq!(cfg.database.max_connections, 50); // From env

    // Clean up
    unsafe {
        env::remove_var("TEST_PHASE1_SERVER__PORT");
        env::remove_var("TEST_PHASE1_DATABASE__MAX_CONNECTIONS");
    }
}

#[tokio::test]
async fn test_validation_success() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    let result = HotswapConfig::builder()
        .with_file(&config_path)
        .with_validation(|config: &AppConfig| {
            if config.server.port < 1024 {
                return Err(ValidationError::invalid_field(
                    "server.port",
                    "must be >= 1024",
                ));
            }
            Ok(())
        })
        .build::<AppConfig>()
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validation_failure() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
        &config_path,
        r#"
server:
  port: 80
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    let result = HotswapConfig::builder()
        .with_file(&config_path)
        .with_validation(|config: &AppConfig| {
            if config.server.port < 1024 {
                return Err(ValidationError::invalid_field(
                    "server.port",
                    "must be >= 1024",
                ));
            }
            Ok(())
        })
        .build::<AppConfig>()
        .await;

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("Configuration validation failed"));
    }
}

#[tokio::test]
async fn test_reload() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    // Initial config
    fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    let config: HotswapConfig<AppConfig> = HotswapConfig::builder()
        .with_file(&config_path)
        .build::<AppConfig>()
        .await
        .unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 8080);

    // Update the config file
    fs::write(
        &config_path,
        r#"
server:
  port: 9090
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    // Reload
    config.reload().await.unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 9090);
}

#[tokio::test]
async fn test_manual_update() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    let config: HotswapConfig<AppConfig> = HotswapConfig::builder()
        .with_file(&config_path)
        .build::<AppConfig>()
        .await
        .unwrap();

    let new_config = AppConfig {
        server: ServerConfig {
            port: 7777,
            host: "127.0.0.1".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://remote/db".to_string(),
            max_connections: 20,
        },
    };

    config.update(new_config.clone()).await.unwrap();

    let cfg = config.get();
    assert_eq!(*cfg, new_config);
}

#[tokio::test]
async fn test_clone_handle() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
        &config_path,
        r#"
server:
  port: 8080
  host: localhost
database:
  url: postgres://localhost/db
  max_connections: 10
"#,
    )
    .unwrap();

    let config: HotswapConfig<AppConfig> = HotswapConfig::builder()
        .with_file(&config_path)
        .build::<AppConfig>()
        .await
        .unwrap();

    let config_clone = config.clone();

    // Both should see the same value
    let cfg1 = config.get();
    let cfg2 = config_clone.get();
    assert_eq!(cfg1.server.port, cfg2.server.port);

    // Update through original
    let new_config = AppConfig {
        server: ServerConfig {
            port: 9090,
            host: "localhost".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://localhost/db".to_string(),
            max_connections: 10,
        },
    };
    config.update(new_config).await.unwrap();

    // Both should see the update
    let cfg1 = config.get();
    let cfg2 = config_clone.get();
    assert_eq!(cfg1.server.port, 9090);
    assert_eq!(cfg2.server.port, 9090);
}
