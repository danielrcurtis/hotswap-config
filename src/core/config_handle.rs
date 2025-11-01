//! The main configuration handle providing lock-free access.

use arc_swap::ArcSwap;
use std::sync::Arc;

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
}

impl<T> HotswapConfig<T> {
    /// Create a new configuration handle with an initial value.
    pub(crate) fn new(initial: T) -> Self {
        Self {
            current: Arc::new(ArcSwap::new(Arc::new(initial))),
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
}

impl<T> Clone for HotswapConfig<T> {
    fn clone(&self) -> Self {
        Self {
            current: Arc::clone(&self.current),
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
