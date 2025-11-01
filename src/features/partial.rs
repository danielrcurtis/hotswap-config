//! Partial configuration updates using JSON Patch.
//!
//! Allows surgical updates to configuration without reloading from files.

use crate::core::HotswapConfig;
use crate::error::{ConfigError, Result};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Extension trait for partial configuration updates.
///
/// Provides methods for applying JSON Patch operations and updating individual fields.
pub trait PartialUpdate<T> {
    /// Apply a JSON Patch to the configuration.
    ///
    /// The patch is applied to the serialized configuration, then validated
    /// and deserialized before atomically swapping.
    ///
    /// # Arguments
    ///
    /// * `patch` - JSON Patch document (array of operations)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The patch is malformed
    /// - Applying the patch fails
    /// - The result cannot be deserialized to T
    /// - Validation fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use hotswap_config::prelude::*;
    /// use hotswap_config::features::PartialUpdate;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// #[derive(Debug, Deserialize, Clone, serde::Serialize)]
    /// struct AppConfig {
    ///     port: u16,
    ///     host: String,
    /// }
    ///
    /// # async fn example(config: HotswapConfig<AppConfig>) -> Result<()> {
    /// // Change just the port
    /// let patch = json!([
    ///     { "op": "replace", "path": "/port", "value": 9090 }
    /// ]);
    ///
    /// config.apply_patch(patch).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn apply_patch(&self, patch: Value) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Update a single field in the configuration.
    ///
    /// This is a convenience method that creates a JSON Patch replace operation.
    ///
    /// # Arguments
    ///
    /// * `path` - JSON Pointer path to the field (e.g., "/port", "/database/host")
    /// * `value` - New value for the field (must be serializable)
    ///
    /// # Errors
    ///
    /// Returns an error if the path is invalid or validation fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use hotswap_config::prelude::*;
    /// use hotswap_config::features::PartialUpdate;
    /// use serde::Deserialize;
    ///
    /// #[derive(Debug, Deserialize, Clone, serde::Serialize)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// # async fn example(config: HotswapConfig<AppConfig>) -> Result<()> {
    /// config.update_field("/port", 9090).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn update_field<V: Serialize + Send>(
        &self,
        path: &str,
        value: V,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

impl<T> PartialUpdate<T> for HotswapConfig<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    async fn apply_patch(&self, patch: Value) -> Result<()> {
        // Get current config
        let current = self.get();

        // Serialize to JSON
        let mut current_json = serde_json::to_value(&*current)
            .map_err(|e| ConfigError::Other(format!("Failed to serialize config: {}", e)))?;

        // Parse patch - json_patch expects an array, deserialize it
        let patch: json_patch::Patch = serde_json::from_value(patch)
            .map_err(|e| ConfigError::Other(format!("Invalid JSON Patch: {}", e)))?;

        // Apply patch
        json_patch::patch(&mut current_json, &patch)
            .map_err(|e| ConfigError::Other(format!("Failed to apply patch: {}", e)))?;

        // Deserialize back to T
        let new_config: T = serde_json::from_value(current_json).map_err(|e| {
            ConfigError::DeserializationError(format!(
                "Failed to deserialize patched config: {}",
                e
            ))
        })?;

        // Use the normal update path (which handles validation and notifications)
        self.update(new_config).await
    }

    async fn update_field<V: Serialize + Send>(&self, path: &str, value: V) -> Result<()> {
        let value_json = serde_json::to_value(value)
            .map_err(|e| ConfigError::Other(format!("Failed to serialize value: {}", e)))?;

        let patch = serde_json::json!([
            { "op": "replace", "path": path, "value": value_json }
        ]);

        self.apply_patch(patch).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        port: u16,
        host: String,
        database: DatabaseConfig,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct DatabaseConfig {
        url: String,
        pool_size: u32,
    }

    #[tokio::test]
    async fn test_apply_patch_replace() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        let patch = json!([
            { "op": "replace", "path": "/port", "value": 9090 }
        ]);

        config.apply_patch(patch).await.unwrap();

        let updated = config.get();
        assert_eq!(updated.port, 9090);
        assert_eq!(updated.host, "localhost");
    }

    #[tokio::test]
    async fn test_apply_patch_nested() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        let patch = json!([
            { "op": "replace", "path": "/database/pool_size", "value": 20 }
        ]);

        config.apply_patch(patch).await.unwrap();

        let updated = config.get();
        assert_eq!(updated.database.pool_size, 20);
    }

    #[tokio::test]
    async fn test_apply_patch_multiple_ops() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        let patch = json!([
            { "op": "replace", "path": "/port", "value": 9090 },
            { "op": "replace", "path": "/host", "value": "0.0.0.0" }
        ]);

        config.apply_patch(patch).await.unwrap();

        let updated = config.get();
        assert_eq!(updated.port, 9090);
        assert_eq!(updated.host, "0.0.0.0");
    }

    #[tokio::test]
    async fn test_update_field() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        config.update_field("/port", 9090).await.unwrap();

        let updated = config.get();
        assert_eq!(updated.port, 9090);
    }

    #[tokio::test]
    async fn test_update_nested_field() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        config
            .update_field("/database/url", "postgres://remote/db")
            .await
            .unwrap();

        let updated = config.get();
        assert_eq!(updated.database.url, "postgres://remote/db");
    }

    #[tokio::test]
    async fn test_invalid_patch() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        // Invalid operation
        let patch = json!([
            { "op": "invalid", "path": "/port" }
        ]);

        let result = config.apply_patch(patch).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_path() {
        let initial = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
            database: DatabaseConfig {
                url: "postgres://localhost/db".to_string(),
                pool_size: 10,
            },
        };

        let config = HotswapConfig::new(initial);

        let result = config.update_field("/nonexistent", 123).await;
        assert!(result.is_err());
    }
}
