//! The main configuration handle providing lock-free access.

use crate::core::ConfigLoader;
use crate::error::{ConfigError, Result, ValidationError};
use arc_swap::ArcSwap;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Type alias for validator functions.
type Validator<T> = Arc<dyn Fn(&T) -> std::result::Result<(), ValidationError> + Send + Sync>;

/// The main configuration handle providing lock-free reads and atomic updates.
///
/// This is the primary interface for accessing configuration. It uses `arc-swap`
/// internally to provide zero-cost reads while allowing atomic updates.
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
///     .with_file("config.yaml")
///     .build::<AppConfig>()
///     .await?;
///
/// // Zero-cost read
/// let cfg = config.get();
/// println!("Port: {}", cfg.port);
/// # Ok(())
/// # }
/// ```
pub struct HotswapConfig<T> {
    /// The current configuration, wrapped in ArcSwap for atomic updates
    current: Arc<ArcSwap<T>>,
    /// Configuration loader for reloading
    loader: Option<Arc<ConfigLoader>>,
    /// Optional validator function
    validator: Option<Validator<T>>,
}

impl<T> HotswapConfig<T> {
    /// Create a new configuration handle with an initial value.
    #[allow(dead_code)]
    pub(crate) fn new(initial: T) -> Self {
        Self {
            current: Arc::new(ArcSwap::new(Arc::new(initial))),
            loader: None,
            validator: None,
        }
    }

    /// Create a configuration handle with loader and validator support.
    pub(crate) fn with_loader(
        initial: T,
        loader: ConfigLoader,
        validator: Option<Validator<T>>,
    ) -> Self {
        Self {
            current: Arc::new(ArcSwap::new(Arc::new(initial))),
            loader: Some(Arc::new(loader)),
            validator,
        }
    }

    /// Get a reference-counted handle to the current configuration.
    ///
    /// This is a zero-cost operation that returns an `Arc<T>`. Readers never
    /// block writers or other readers.
    ///
    /// # Performance
    ///
    /// This operation is lock-free and typically completes in < 10 nanoseconds.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::prelude::*;
    /// # use serde::Deserialize;
    /// # #[derive(Debug, Deserialize, Clone)]
    /// # struct AppConfig { port: u16 }
    /// # async fn example(config: HotswapConfig<AppConfig>) {
    /// let cfg = config.get();
    /// println!("Port: {}", cfg.port);
    /// # }
    /// ```
    pub fn get(&self) -> Arc<T> {
        self.current.load_full()
    }

    /// Manually reload configuration from all sources.
    ///
    /// This triggers a full reload, respecting the precedence order.
    /// If validation fails, the old configuration is retained.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No loader is available (shouldn't happen with normal usage)
    /// - Configuration sources cannot be read
    /// - Deserialization fails
    /// - Validation fails (if a validator is configured)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::prelude::*;
    /// # use serde::Deserialize;
    /// # #[derive(Debug, Deserialize, Clone)]
    /// # struct AppConfig { port: u16 }
    /// # async fn example(config: HotswapConfig<AppConfig>) -> Result<()> {
    /// // Manually trigger a reload
    /// config.reload().await?;
    ///
    /// let cfg = config.get();
    /// println!("Reloaded config, port: {}", cfg.port);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn reload(&self) -> Result<()>
    where
        T: DeserializeOwned + Clone,
    {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| ConfigError::Other("No loader available for reload".to_string()))?;

        // Load the new configuration
        let new_config: T = loader.load()?;

        // Validate if a validator was provided
        if let Some(validator) = &self.validator {
            validator(&new_config).map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        }

        // Atomically swap to the new configuration
        self.current.store(Arc::new(new_config));

        Ok(())
    }

    /// Update configuration with a new value directly.
    ///
    /// This bypasses the loader and directly updates the configuration.
    /// Useful for programmatic updates or testing.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::prelude::*;
    /// # use serde::Deserialize;
    /// # #[derive(Debug, Deserialize, Clone)]
    /// # struct AppConfig { port: u16 }
    /// # async fn example(config: HotswapConfig<AppConfig>) -> Result<()> {
    /// let new_config = AppConfig { port: 9090 };
    /// config.update(new_config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update(&self, new_config: T) -> Result<()> {
        // Validate if a validator was provided
        if let Some(validator) = &self.validator {
            validator(&new_config).map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        }

        // Atomically swap to the new configuration
        self.current.store(Arc::new(new_config));

        Ok(())
    }
}

impl<T> Clone for HotswapConfig<T> {
    fn clone(&self) -> Self {
        Self {
            current: Arc::clone(&self.current),
            loader: self.loader.clone(),
            validator: self.validator.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestConfig {
        value: i32,
    }

    #[test]
    fn test_create_and_read() {
        let config = HotswapConfig::new(TestConfig { value: 42 });
        let cfg = config.get();
        assert_eq!(cfg.value, 42);
    }

    #[test]
    fn test_clone() {
        let config = HotswapConfig::new(TestConfig { value: 42 });
        let config2 = config.clone();

        let cfg1 = config.get();
        let cfg2 = config2.get();

        assert_eq!(cfg1.value, cfg2.value);
    }
}
