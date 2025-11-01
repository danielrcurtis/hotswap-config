//! Builder for constructing HotswapConfig instances.

use crate::core::HotswapConfig;
use crate::error::{ConfigError, Result};
use serde::de::DeserializeOwned;

/// Builder for constructing a `HotswapConfig` instance.
///
/// Provides a fluent interface for configuring all aspects of configuration loading.
///
/// # Examples
///
/// ```rust,no_run
/// use hotswap_config::prelude::*;
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, Clone)]
/// struct AppConfig {
///     port: u16,
/// }
///
/// # async fn example() -> Result<()> {
/// let config = HotswapConfig::builder()
///     .with_file("config/default.yaml")
///     .with_file("config/production.yaml")
///     .with_env_overrides("APP", "__")
///     .build::<AppConfig>()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct HotswapConfigBuilder {
    // TODO: Add fields for sources, validators, etc.
}

impl HotswapConfigBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {}
    }

    /// Add a file source with automatic format detection.
    ///
    /// Supported formats: YAML (.yaml, .yml), TOML (.toml), JSON (.json)
    pub fn with_file(self, _path: impl Into<std::path::PathBuf>) -> Self {
        // TODO: Implement
        self
    }

    /// Add environment variable source with custom prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Prefix for environment variables (e.g., "APP")
    /// * `separator` - Separator for nested keys (e.g., "__" for APP_DB__HOST)
    pub fn with_env_overrides(self, _prefix: &str, _separator: &str) -> Self {
        // TODO: Implement
        self
    }

    /// Build the configuration handle.
    ///
    /// This performs the initial load from all sources.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The configuration type (must implement `DeserializeOwned`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Initial configuration load fails
    /// - Deserialization fails
    /// - Validation fails
    pub async fn build<T>(self) -> Result<HotswapConfig<T>>
    where
        T: DeserializeOwned + Clone + Send + Sync + 'static,
    {
        // TODO: Implement actual loading
        // For now, return a placeholder error
        Err(ConfigError::Other("Not yet implemented".to_string()))
    }
}

impl Default for HotswapConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HotswapConfig<T> {
    /// Create a new builder for constructing a configuration handle.
    pub fn builder() -> HotswapConfigBuilder {
        HotswapConfigBuilder::new()
    }
}
