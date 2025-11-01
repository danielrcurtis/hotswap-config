//! The main configuration handle providing lock-free access.

use crate::core::ConfigLoader;
use crate::error::{ConfigError, Result, ValidationError};
use arc_swap::ArcSwap;
use serde::de::DeserializeOwned;
use std::sync::Arc;

#[cfg(feature = "file-watch")]
use crate::notify::{ConfigWatcher, SubscriberRegistry};

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
    /// Optional file watcher for auto-reload
    #[cfg(feature = "file-watch")]
    watcher: Option<Arc<ConfigWatcher>>,
    /// Subscriber registry for change notifications
    #[cfg(feature = "file-watch")]
    subscribers: Arc<SubscriberRegistry>,
}

impl<T> HotswapConfig<T> {
    /// Create a new configuration handle with an initial value.
    ///
    /// This creates a basic configuration handle without any loading or validation.
    /// For most use cases, prefer using `HotswapConfig::builder()` instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use hotswap_config::prelude::*;
    ///
    /// let config = HotswapConfig::new(42);
    /// assert_eq!(*config.get(), 42);
    /// ```
    pub fn new(initial: T) -> Self {
        Self {
            current: Arc::new(ArcSwap::new(Arc::new(initial))),
            loader: None,
            validator: None,
            #[cfg(feature = "file-watch")]
            watcher: None,
            #[cfg(feature = "file-watch")]
            subscribers: Arc::new(SubscriberRegistry::new()),
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
            #[cfg(feature = "file-watch")]
            watcher: None,
            #[cfg(feature = "file-watch")]
            subscribers: Arc::new(SubscriberRegistry::new()),
        }
    }

    /// Set the file watcher for this configuration.
    #[cfg(feature = "file-watch")]
    pub(crate) fn with_watcher(mut self, watcher: Arc<ConfigWatcher>) -> Self {
        self.watcher = Some(watcher);
        self
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

        // Notify subscribers
        #[cfg(feature = "file-watch")]
        self.subscribers.notify_all().await;

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

        // Notify subscribers
        #[cfg(feature = "file-watch")]
        self.subscribers.notify_all().await;

        Ok(())
    }

    /// Subscribe to configuration changes.
    ///
    /// The provided callback will be invoked whenever the configuration
    /// is reloaded or updated. Returns a handle that can be dropped to unsubscribe.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::prelude::*;
    /// # use serde::Deserialize;
    /// # #[derive(Debug, Deserialize, Clone)]
    /// # struct AppConfig { port: u16 }
    /// # async fn example(config: HotswapConfig<AppConfig>) {
    /// let handle = config.subscribe(|| {
    ///     println!("Configuration changed!");
    /// }).await;
    ///
    /// // Later, unsubscribe
    /// drop(handle);
    /// # }
    /// ```
    #[cfg(feature = "file-watch")]
    pub async fn subscribe<F>(&self, callback: F) -> crate::notify::SubscriptionHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.subscribers.subscribe(callback).await
    }

    /// Start watching configuration files for changes.
    ///
    /// When enabled, the configuration will automatically reload when any
    /// watched file changes. This requires a file watcher to be set up
    /// via the builder.
    ///
    /// # Errors
    ///
    /// Returns an error if no file watcher is configured.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::prelude::*;
    /// # use serde::Deserialize;
    /// # #[derive(Debug, Deserialize, Clone)]
    /// # struct AppConfig { port: u16 }
    /// # async fn example() -> Result<()> {
    /// let config = HotswapConfig::builder()
    ///     .with_file("config.yaml")
    ///     .with_file_watch(true)
    ///     .build::<AppConfig>()
    ///     .await?;
    ///
    /// // File watching is now active
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "file-watch")]
    pub fn is_watching(&self) -> bool {
        self.watcher.is_some()
    }
}

impl<T> Clone for HotswapConfig<T> {
    fn clone(&self) -> Self {
        Self {
            current: Arc::clone(&self.current),
            loader: self.loader.clone(),
            validator: self.validator.clone(),
            #[cfg(feature = "file-watch")]
            watcher: self.watcher.clone(),
            #[cfg(feature = "file-watch")]
            subscribers: Arc::clone(&self.subscribers),
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
