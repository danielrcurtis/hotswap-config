//! Configuration rollback support with version history.
//!
//! Tracks previous configuration versions and allows rolling back to earlier states.

use crate::core::HotswapConfig;
use crate::error::{ConfigError, Result};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A versioned configuration snapshot.
#[derive(Clone)]
pub struct ConfigVersion<T> {
    /// Version number (monotonically increasing)
    pub version: u64,
    /// Timestamp when this version was created
    pub timestamp: DateTime<Utc>,
    /// The configuration data
    pub config: Arc<T>,
    /// Optional description of why this version was created
    pub source: Option<String>,
}

/// Configuration history tracker.
///
/// Maintains a bounded history of configuration versions that can be
/// rolled back to.
pub struct ConfigHistory<T> {
    versions: Arc<RwLock<VecDeque<ConfigVersion<T>>>>,
    max_size: usize,
    next_version: Arc<RwLock<u64>>,
}

impl<T: Clone> ConfigHistory<T> {
    /// Create a new configuration history with a maximum size.
    ///
    /// # Arguments
    ///
    /// * `max_size` - Maximum number of versions to keep (older versions are dropped)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use hotswap_config::features::ConfigHistory;
    ///
    /// let history: ConfigHistory<String> = ConfigHistory::new(10);
    /// ```
    pub fn new(max_size: usize) -> Self {
        Self {
            versions: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
            next_version: Arc::new(RwLock::new(0)),
        }
    }

    /// Record a new configuration version.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to record
    /// * `source` - Optional description of the change source
    pub async fn record(&self, config: Arc<T>, source: Option<String>) {
        let mut versions = self.versions.write().await;
        let mut next_version = self.next_version.write().await;

        let version = ConfigVersion {
            version: *next_version,
            timestamp: Utc::now(),
            config,
            source,
        };

        versions.push_back(version);
        *next_version += 1;

        // Trim to max size
        while versions.len() > self.max_size {
            versions.pop_front();
        }
    }

    /// Get the current version number.
    pub async fn current_version(&self) -> u64 {
        let next_version = self.next_version.read().await;
        next_version.saturating_sub(1)
    }

    /// Get the total number of versions in history.
    pub async fn len(&self) -> usize {
        let versions = self.versions.read().await;
        versions.len()
    }

    /// Check if the history is empty.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Get a specific version by version number.
    pub async fn get_version(&self, version: u64) -> Option<ConfigVersion<T>> {
        let versions = self.versions.read().await;
        versions.iter().find(|v| v.version == version).cloned()
    }

    /// Get the N most recent versions.
    pub async fn get_recent(&self, count: usize) -> Vec<ConfigVersion<T>> {
        let versions = self.versions.read().await;
        versions.iter().rev().take(count).cloned().collect()
    }

    /// Get all versions in chronological order.
    pub async fn get_all(&self) -> Vec<ConfigVersion<T>> {
        let versions = self.versions.read().await;
        versions.iter().cloned().collect()
    }

    /// Rollback to a specific version number.
    ///
    /// Returns the configuration at that version, or None if the version
    /// is not in history.
    pub async fn rollback_to_version(&self, version: u64) -> Option<Arc<T>> {
        self.get_version(version).await.map(|v| v.config)
    }

    /// Rollback N steps from the current version.
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of versions to go back (1 = previous version)
    ///
    /// Returns None if stepping back that far exceeds available history.
    pub async fn rollback_steps(&self, steps: usize) -> Option<Arc<T>> {
        let versions = self.versions.read().await;
        if versions.len() <= steps {
            return None;
        }

        // Get the version that is `steps` back from the end
        let index = versions.len() - steps - 1;
        versions.get(index).map(|v| Arc::clone(&v.config))
    }
}

impl<T: Clone> Clone for ConfigHistory<T> {
    fn clone(&self) -> Self {
        Self {
            versions: Arc::clone(&self.versions),
            max_size: self.max_size,
            next_version: Arc::clone(&self.next_version),
        }
    }
}

/// Extension trait for rollback support on HotswapConfig.
pub trait Rollback<T> {
    /// Enable rollback support with a history size.
    ///
    /// Returns a ConfigHistory instance that tracks configuration changes.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use hotswap_config::prelude::*;
    /// use hotswap_config::features::Rollback;
    /// use serde::Deserialize;
    ///
    /// #[derive(Debug, Deserialize, Clone)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// # async fn example(config: HotswapConfig<AppConfig>) -> Result<()> {
    /// let history = config.enable_history(10);
    ///
    /// // Make changes...
    /// config.reload().await?;
    ///
    /// // Rollback 1 step
    /// config.rollback(&history, 1).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn enable_history(&self, max_size: usize) -> ConfigHistory<T>;

    /// Rollback N steps in the history.
    ///
    /// # Arguments
    ///
    /// * `history` - The ConfigHistory instance
    /// * `steps` - Number of versions to go back
    ///
    /// # Errors
    ///
    /// Returns an error if the requested step count exceeds available history.
    fn rollback(
        &self,
        history: &ConfigHistory<T>,
        steps: usize,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Rollback to a specific version number.
    fn rollback_to_version(
        &self,
        history: &ConfigHistory<T>,
        version: u64,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

impl<T> Rollback<T> for HotswapConfig<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn enable_history(&self, max_size: usize) -> ConfigHistory<T> {
        let history = ConfigHistory::new(max_size);

        // Record the current configuration as version 0
        let current = self.get();
        let history_clone = history.clone();
        tokio::spawn(async move {
            history_clone
                .record(current, Some("Initial version".to_string()))
                .await;
        });

        history
    }

    async fn rollback(&self, history: &ConfigHistory<T>, steps: usize) -> Result<()> {
        let config = history.rollback_steps(steps).await.ok_or_else(|| {
            ConfigError::Other(format!(
                "Cannot rollback {} steps: insufficient history",
                steps
            ))
        })?;

        self.update((*config).clone()).await?;

        // Record this rollback in history
        history
            .record(config, Some(format!("Rollback {} steps", steps)))
            .await;

        Ok(())
    }

    async fn rollback_to_version(&self, history: &ConfigHistory<T>, version: u64) -> Result<()> {
        let config = history.rollback_to_version(version).await.ok_or_else(|| {
            ConfigError::Other(format!("Version {} not found in history", version))
        })?;

        self.update((*config).clone()).await?;

        // Record this rollback in history
        history
            .record(config, Some(format!("Rollback to version {}", version)))
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_history_creation() {
        let history: ConfigHistory<i32> = ConfigHistory::new(5);
        assert_eq!(history.len().await, 0);
        assert!(history.is_empty().await);
    }

    #[tokio::test]
    async fn test_record_and_retrieve() {
        let history = ConfigHistory::new(5);

        history.record(Arc::new(1), Some("First".to_string())).await;
        history
            .record(Arc::new(2), Some("Second".to_string()))
            .await;

        assert_eq!(history.len().await, 2);

        let version = history.get_version(0).await.unwrap();
        assert_eq!(*version.config, 1);

        let version = history.get_version(1).await.unwrap();
        assert_eq!(*version.config, 2);
    }

    #[tokio::test]
    async fn test_max_size_limit() {
        let history = ConfigHistory::new(3);

        for i in 0..5 {
            history.record(Arc::new(i), None).await;
        }

        assert_eq!(history.len().await, 3);

        // Should have versions 2, 3, 4 (oldest dropped)
        assert!(history.get_version(0).await.is_none());
        assert!(history.get_version(1).await.is_none());
        assert!(history.get_version(2).await.is_some());
    }

    #[tokio::test]
    async fn test_rollback_steps() {
        let history = ConfigHistory::new(5);

        history.record(Arc::new(10), None).await;
        history.record(Arc::new(20), None).await;
        history.record(Arc::new(30), None).await;

        let config = history.rollback_steps(1).await.unwrap();
        assert_eq!(*config, 20);

        let config = history.rollback_steps(2).await.unwrap();
        assert_eq!(*config, 10);
    }

    #[tokio::test]
    async fn test_rollback_steps_exceeds_history() {
        let history = ConfigHistory::new(5);

        history.record(Arc::new(10), None).await;
        history.record(Arc::new(20), None).await;

        let config = history.rollback_steps(5).await;
        assert!(config.is_none());
    }

    #[tokio::test]
    async fn test_get_recent() {
        let history = ConfigHistory::new(10);

        for i in 0..5 {
            history.record(Arc::new(i), None).await;
        }

        let recent = history.get_recent(3).await;
        assert_eq!(recent.len(), 3);
        assert_eq!(*recent[0].config, 4);
        assert_eq!(*recent[1].config, 3);
        assert_eq!(*recent[2].config, 2);
    }

    #[tokio::test]
    async fn test_current_version() {
        let history = ConfigHistory::new(5);

        history.record(Arc::new(1), None).await;
        assert_eq!(history.current_version().await, 0);

        history.record(Arc::new(2), None).await;
        assert_eq!(history.current_version().await, 1);
    }

    #[tokio::test]
    async fn test_hotswap_config_rollback() {
        let config = HotswapConfig::new(10);
        let history = config.enable_history(5);

        // Wait for initial record
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Make some updates
        config.update(20).await.unwrap();
        history
            .record(config.get(), Some("Update to 20".to_string()))
            .await;

        config.update(30).await.unwrap();
        history
            .record(config.get(), Some("Update to 30".to_string()))
            .await;

        // Current should be 30
        assert_eq!(*config.get(), 30);

        // Rollback 1 step (to 20)
        config.rollback(&history, 1).await.unwrap();
        assert_eq!(*config.get(), 20);
    }
}
