//! File watching for automatic configuration reloads.

use crate::error::{ConfigError, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Configuration watcher that monitors files for changes.
///
/// Uses the `notify` crate to watch configuration files and trigger reloads
/// when they change. Includes debouncing to avoid rapid reloads.
///
/// # Examples
///
/// ```rust,no_run
/// use hotswap_config::notify::ConfigWatcher;
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let (watcher, mut rx) = ConfigWatcher::new(Duration::from_millis(500))?;
/// watcher.watch("/path/to/config.yaml").await?;
///
/// // Listen for reload signals
/// while let Some(()) = rx.recv().await {
///     println!("Config file changed, reload triggered!");
/// }
/// # Ok(())
/// # }
/// ```
pub struct ConfigWatcher {
    watcher: Arc<tokio::sync::Mutex<RecommendedWatcher>>,
    debounce_duration: Duration,
    watched_paths: Arc<tokio::sync::Mutex<Vec<PathBuf>>>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher.
    ///
    /// # Arguments
    ///
    /// * `debounce_duration` - Minimum time between reload triggers (default: 500ms)
    ///
    /// # Returns
    ///
    /// Returns a tuple of (ConfigWatcher, receiver channel). The receiver will
    /// receive a message whenever a reload should be triggered.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying file watcher cannot be created.
    pub fn new(debounce_duration: Duration) -> Result<(Self, mpsc::Receiver<()>)> {
        let (tx, rx) = mpsc::channel(100);
        let debounce = debounce_duration;

        // Channel for raw events from notify
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

        // Create the notify watcher
        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                // Only care about write/modify events
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                ) {
                    let _ = event_tx.send(event);
                }
            }
        })
        .map_err(|e| ConfigError::Other(format!("Failed to create file watcher: {}", e)))?;

        // Spawn a task to debounce events and trigger reloads
        tokio::spawn(async move {
            let mut last_reload = tokio::time::Instant::now();

            while let Some(_event) = event_rx.recv().await {
                let now = tokio::time::Instant::now();
                let elapsed = now.duration_since(last_reload);

                if elapsed >= debounce {
                    // Trigger reload
                    if tx.send(()).await.is_err() {
                        // Receiver dropped, exit
                        break;
                    }
                    last_reload = now;
                } else {
                    // Schedule a delayed reload
                    let remaining = debounce - elapsed;
                    let tx_clone = tx.clone();
                    tokio::spawn(async move {
                        sleep(remaining).await;
                        let _ = tx_clone.send(()).await;
                    });
                }
            }
        });

        Ok((
            Self {
                watcher: Arc::new(tokio::sync::Mutex::new(watcher)),
                debounce_duration,
                watched_paths: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            },
            rx,
        ))
    }

    /// Add a path to watch for changes.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file or directory to watch
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be watched (e.g., doesn't exist).
    pub async fn watch(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();

        // Canonicalize the path to get the absolute path
        let canonical_path = path
            .canonicalize()
            .map_err(|e| ConfigError::LoadError(format!("Failed to resolve path: {}", e)))?;

        let mut watcher = self.watcher.lock().await;
        watcher
            .watch(&canonical_path, RecursiveMode::NonRecursive)
            .map_err(|e| ConfigError::Other(format!("Failed to watch path: {}", e)))?;

        // Track watched paths
        let mut paths = self.watched_paths.lock().await;
        if !paths.contains(&canonical_path) {
            paths.push(canonical_path);
        }

        Ok(())
    }

    /// Stop watching a specific path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to stop watching
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be unwatched.
    pub async fn unwatch(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let canonical_path = path.canonicalize().map_err(|e| {
            ConfigError::LoadError(format!("Failed to resolve path for unwatching: {}", e))
        })?;

        let mut watcher = self.watcher.lock().await;
        watcher
            .unwatch(&canonical_path)
            .map_err(|e| ConfigError::Other(format!("Failed to unwatch path: {}", e)))?;

        // Remove from tracked paths
        let mut paths = self.watched_paths.lock().await;
        paths.retain(|p| p != &canonical_path);

        Ok(())
    }

    /// Get the debounce duration for this watcher.
    pub fn debounce_duration(&self) -> Duration {
        self.debounce_duration
    }

    /// Get a list of currently watched paths.
    pub async fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_watcher_creation() {
        let result = ConfigWatcher::new(Duration::from_millis(100));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_watch_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "port: 8080").unwrap();

        let (watcher, _rx) = ConfigWatcher::new(Duration::from_millis(100)).unwrap();
        let result = watcher.watch(&config_path).await;
        assert!(result.is_ok());

        let paths = watcher.watched_paths().await;
        assert_eq!(paths.len(), 1);
    }

    #[tokio::test]
    async fn test_watch_nonexistent_file() {
        let (watcher, _rx) = ConfigWatcher::new(Duration::from_millis(100)).unwrap();
        let result = watcher.watch("/nonexistent/config.yaml").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_change_triggers_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "port: 8080").unwrap();

        let (watcher, mut rx) = ConfigWatcher::new(Duration::from_millis(100)).unwrap();
        watcher.watch(&config_path).await.unwrap();

        // Modify the file
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            fs::write(&config_path, "port: 9090").unwrap();
        });

        // Wait for reload signal with timeout
        let result = timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_unwatch() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "port: 8080").unwrap();

        let (watcher, _rx) = ConfigWatcher::new(Duration::from_millis(100)).unwrap();
        watcher.watch(&config_path).await.unwrap();

        let paths = watcher.watched_paths().await;
        assert_eq!(paths.len(), 1);

        watcher.unwatch(&config_path).await.unwrap();

        let paths = watcher.watched_paths().await;
        assert_eq!(paths.len(), 0);
    }

    #[tokio::test]
    async fn test_debounce_duration() {
        let duration = Duration::from_millis(500);
        let (watcher, _rx) = ConfigWatcher::new(duration).unwrap();
        assert_eq!(watcher.debounce_duration(), duration);
    }
}
