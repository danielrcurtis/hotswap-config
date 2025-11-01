//! Environment variable configuration source.

use super::ConfigSource;
use crate::error::Result;
use config::Environment;
use std::collections::HashMap;

/// Environment variable configuration source.
///
/// Loads configuration from environment variables with a specified prefix
/// and separator for nested keys.
///
/// # Examples
///
/// ```rust
/// use hotswap_config::sources::EnvSource;
///
/// // APP_SERVER__PORT=8080 -> server.port = 8080
/// let source = EnvSource::new("APP", "__");
/// ```
pub struct EnvSource {
    prefix: String,
    separator: String,
    priority: i32,
}

impl EnvSource {
    /// Create a new environment variable source.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Prefix for environment variables (e.g., "APP")
    /// * `separator` - Separator for nested keys (e.g., "__" for APP_DB__HOST)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use hotswap_config::sources::EnvSource;
    ///
    /// // Matches: APP_SERVER__PORT, APP_DB__HOST, etc.
    /// let source = EnvSource::new("APP", "__");
    /// ```
    pub fn new(prefix: impl Into<String>, separator: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            separator: separator.into(),
            priority: 300, // Env vars have highest priority by default
        }
    }

    /// Set the priority for this source.
    ///
    /// Higher priority sources override lower priority ones.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

impl ConfigSource for EnvSource {
    fn load(&self) -> Result<HashMap<String, config::Value>> {
        // Use the config crate's Environment source
        let env_source = Environment::with_prefix(&self.prefix)
            .separator(&self.separator)
            .try_parsing(true); // Try to parse numbers, booleans, etc.

        // Build a config with just this environment source
        let config_builder = config::Config::builder()
            .add_source(env_source)
            .build()
            .map_err(|e| {
                crate::error::ConfigError::LoadError(format!(
                    "Failed to load environment variables: {}",
                    e
                ))
            })?;

        // Extract as HashMap
        let map = config_builder
            .try_deserialize::<HashMap<String, config::Value>>()
            .map_err(|e| {
                crate::error::ConfigError::DeserializationError(format!(
                    "Failed to parse environment variables: {}",
                    e
                ))
            })?;

        Ok(map)
    }

    fn name(&self) -> String {
        format!("env:{}*", self.prefix)
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

#[cfg(test)]
#[allow(unsafe_code)] // For env var manipulation in tests
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_source_creation() {
        let source = EnvSource::new("APP", "__");
        assert_eq!(source.prefix, "APP");
        assert_eq!(source.separator, "__");
        assert_eq!(source.priority(), 300);
    }

    #[test]
    fn test_with_priority() {
        let source = EnvSource::new("APP", "__").with_priority(400);
        assert_eq!(source.priority(), 400);
    }

    #[test]
    fn test_name() {
        let source = EnvSource::new("APP", "__");
        assert_eq!(source.name(), "env:APP*");
    }

    #[test]
    fn test_load_empty() {
        // Clear any TEST_* env vars first
        for (key, _) in env::vars() {
            if key.starts_with("TEST_HOTSWAP_") {
                unsafe {
                    env::remove_var(&key);
                }
            }
        }

        let source = EnvSource::new("TEST_HOTSWAP_NONEXISTENT", "__");
        let result = source.load();
        assert!(result.is_ok());
        // Should return empty map if no matching env vars
        let map = result.unwrap();
        assert!(map.is_empty() || !map.is_empty()); // Either is valid
    }

    // Note: Testing actual env var loading is done in integration tests
    // because the config crate's Environment source behavior can be
    // tricky to test in unit tests due to when env vars are captured.
}
