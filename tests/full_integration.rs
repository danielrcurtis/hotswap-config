//! Full integration tests exercising all features together.

use hotswap_config::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct IntegrationConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    features: Features,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ServerConfig {
    port: u16,
    host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Features {
    enable_metrics: bool,
    enable_caching: bool,
}

#[tokio::test]
async fn test_file_loading_basic() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
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
    )
    .unwrap();

    let config = HotswapConfig::builder()
        .with_file(&config_path)
        .build::<IntegrationConfig>()
        .await
        .unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 8080);
    assert_eq!(cfg.server.host, "localhost");
    assert_eq!(cfg.database.max_connections, 10);
    assert!(!cfg.features.enable_metrics);
    assert!(cfg.features.enable_caching);
}

#[tokio::test]
async fn test_file_watching_with_validation() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
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
    )
    .unwrap();

    let config = HotswapConfig::builder()
        .with_file(&config_path)
        .with_validation(|cfg: &IntegrationConfig| {
            if cfg.server.port < 1024 {
                return Err(hotswap_config::error::ValidationError::invalid_field(
                    "port",
                    "must be >= 1024",
                ));
            }
            Ok(())
        })
        .with_file_watch(true)
        .build::<IntegrationConfig>()
        .await
        .unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 8080);

    // Update file
    fs::write(
        &config_path,
        r#"
server:
  port: 9090
  host: "0.0.0.0"

database:
  url: "postgresql://localhost/mydb"
  max_connections: 20

features:
  enable_metrics: true
  enable_caching: true
"#,
    )
    .unwrap();

    // Wait for file watcher to pick up change
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Manually reload to test validation
    config.reload().await.unwrap();

    let cfg = config.get();
    assert_eq!(cfg.server.port, 9090);
    assert_eq!(cfg.server.host, "0.0.0.0");
    assert_eq!(cfg.database.max_connections, 20);
    assert!(cfg.features.enable_metrics);
}

#[tokio::test]
async fn test_subscribers_notification() {
    let config = HotswapConfig::new(IntegrationConfig {
        server: ServerConfig {
            port: 8080,
            host: "localhost".to_string(),
        },
        database: DatabaseConfig {
            url: "postgresql://localhost/mydb".to_string(),
            max_connections: 10,
        },
        features: Features {
            enable_metrics: false,
            enable_caching: true,
        },
    });

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let _handle = config
        .subscribe(move || {
            counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .await;

    // Update config
    let new_config = IntegrationConfig {
        server: ServerConfig {
            port: 9090,
            host: "0.0.0.0".to_string(),
        },
        database: DatabaseConfig {
            url: "postgresql://localhost/mydb".to_string(),
            max_connections: 20,
        },
        features: Features {
            enable_metrics: true,
            enable_caching: true,
        },
    };

    config.update(new_config).await.unwrap();

    // Give subscriber time to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
}

// Note: Advanced features (partial updates, rollback, gradual rollout) are tested
// in their respective module test files. These integration tests focus on
// basic features working together.

#[cfg(feature = "metrics")]
#[tokio::test]
async fn test_metrics_integration() {
    use opentelemetry::global;

    let meter = global::meter("test");

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
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
    )
    .unwrap();

    let config = HotswapConfig::builder()
        .with_file(&config_path)
        .with_metrics(meter)
        .build::<IntegrationConfig>()
        .await
        .unwrap();

    // Perform operations that generate metrics
    config.reload().await.unwrap();

    let new_config = IntegrationConfig {
        server: ServerConfig {
            port: 9090,
            host: "0.0.0.0".to_string(),
        },
        database: DatabaseConfig {
            url: "postgresql://localhost/mydb".to_string(),
            max_connections: 20,
        },
        features: Features {
            enable_metrics: true,
            enable_caching: true,
        },
    };

    config.update(new_config).await.unwrap();

    // Metrics should be recorded (we can't easily verify values without introspection)
    // but this test ensures metrics don't cause crashes
}

#[tokio::test]
async fn test_validation_failure_preserves_old_config() {
    let _config = HotswapConfig::new(IntegrationConfig {
        server: ServerConfig {
            port: 8080,
            host: "localhost".to_string(),
        },
        database: DatabaseConfig {
            url: "postgresql://localhost/mydb".to_string(),
            max_connections: 10,
        },
        features: Features {
            enable_metrics: false,
            enable_caching: true,
        },
    });

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(
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
    )
    .unwrap();

    let config_with_validation = HotswapConfig::builder()
        .with_file(&config_path)
        .with_validation(|cfg: &IntegrationConfig| {
            if cfg.server.port < 1024 {
                return Err(hotswap_config::error::ValidationError::invalid_field(
                    "port",
                    "must be >= 1024",
                ));
            }
            Ok(())
        })
        .build::<IntegrationConfig>()
        .await
        .unwrap();

    // Try to update with invalid config
    let invalid_config = IntegrationConfig {
        server: ServerConfig {
            port: 80, // Invalid!
            host: "localhost".to_string(),
        },
        database: DatabaseConfig {
            url: "postgresql://localhost/mydb".to_string(),
            max_connections: 10,
        },
        features: Features {
            enable_metrics: false,
            enable_caching: true,
        },
    };

    let result = config_with_validation.update(invalid_config).await;
    assert!(result.is_err());

    // Original config should be preserved
    let cfg = config_with_validation.get();
    assert_eq!(cfg.server.port, 8080); // Still the old value
}

#[tokio::test]
async fn test_concurrent_reads_during_updates() {
    let config = std::sync::Arc::new(HotswapConfig::new(IntegrationConfig {
        server: ServerConfig {
            port: 8080,
            host: "localhost".to_string(),
        },
        database: DatabaseConfig {
            url: "postgresql://localhost/mydb".to_string(),
            max_connections: 10,
        },
        features: Features {
            enable_metrics: false,
            enable_caching: true,
        },
    }));

    let mut handles = vec![];

    // Spawn 10 reader tasks
    for _ in 0..10 {
        let cfg = config.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..100 {
                let data = cfg.get();
                assert!(data.server.port >= 8080 && data.server.port <= 8090);
                tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;
            }
        });
        handles.push(handle);
    }

    // Perform updates while readers are running
    for i in 0..10 {
        let new_config = IntegrationConfig {
            server: ServerConfig {
                port: 8080 + i,
                host: "localhost".to_string(),
            },
            database: DatabaseConfig {
                url: "postgresql://localhost/mydb".to_string(),
                max_connections: 10,
            },
            features: Features {
                enable_metrics: false,
                enable_caching: true,
            },
        };

        config.update(new_config).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Wait for all readers
    for handle in handles {
        handle.await.unwrap();
    }
}
