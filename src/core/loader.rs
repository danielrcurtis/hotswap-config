//! Configuration loader that merges multiple sources.

use crate::error::{ConfigError, Result};
use crate::sources::ConfigSource;
use serde::de::DeserializeOwned;

/// Loads and merges configuration from multiple sources.
///
/// The loader handles precedence by sorting sources by priority and merging them
/// in order (lower priority first, higher priority sources override).
pub struct ConfigLoader {
    sources: Vec<Box<dyn ConfigSource>>,
}

impl ConfigLoader {
    /// Create a new configuration loader.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Add a configuration source.
    pub fn add_source(&mut self, source: Box<dyn ConfigSource>) {
        self.sources.push(source);
    }

    /// Load and merge configuration from all sources.
    ///
    /// Sources are merged in priority order (lowest to highest), so higher priority
    /// sources override values from lower priority sources.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The target configuration type
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any source fails to load
    /// - Deserialization fails
    pub fn load<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        if self.sources.is_empty() {
            return Err(ConfigError::LoadError(
                "No configuration sources specified".to_string(),
            ));
        }

        // Sort sources by priority (lowest first)
        let mut sorted_sources: Vec<_> = self.sources.iter().collect();
        sorted_sources.sort_by_key(|s| s.priority());

        // Start with an empty config builder
        let mut builder = config::Config::builder();

        // Merge each source in priority order
        for source in sorted_sources {
            let values = source.load().map_err(|e| {
                ConfigError::LoadError(format!("Failed to load source '{}': {}", source.name(), e))
            })?;

            // Convert HashMap<String, config::Value> to config::Config and add as source
            for (key, value) in values {
                builder = builder.set_override(&key, value).map_err(|e| {
                    ConfigError::LoadError(format!(
                        "Failed to merge source '{}': {}",
                        source.name(),
                        e
                    ))
                })?;
            }
        }

        // Build the final config
        let config = builder
            .build()
            .map_err(|e| ConfigError::LoadError(format!("Failed to build configuration: {}", e)))?;

        // Deserialize into target type
        config.try_deserialize::<T>().map_err(|e| {
            ConfigError::DeserializationError(format!("Failed to deserialize configuration: {}", e))
        })
    }

    /// Get the list of source names in priority order.
    #[allow(dead_code)]
    pub fn source_names(&self) -> Vec<String> {
        let mut sorted_sources: Vec<_> = self.sources.iter().collect();
        sorted_sources.sort_by_key(|s| s.priority());
        sorted_sources.iter().map(|s| s.name()).collect()
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::ConfigSource;
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestConfig {
        port: u16,
        host: String,
    }

    struct MockSource {
        name: String,
        priority: i32,
        values: HashMap<String, config::Value>,
    }

    impl MockSource {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                name: name.to_string(),
                priority,
                values: HashMap::new(),
            }
        }

        fn with_value(mut self, key: &str, value: impl Into<config::Value>) -> Self {
            self.values.insert(key.to_string(), value.into());
            self
        }
    }

    impl ConfigSource for MockSource {
        fn load(&self) -> Result<HashMap<String, config::Value>> {
            Ok(self.values.clone())
        }

        fn name(&self) -> String {
            self.name.clone()
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[test]
    fn test_empty_loader() {
        let loader = ConfigLoader::new();
        let result: Result<TestConfig> = loader.load();
        assert!(result.is_err());
    }

    #[test]
    fn test_single_source() {
        let mut loader = ConfigLoader::new();
        let source = MockSource::new("test", 100)
            .with_value("port", 8080i64)
            .with_value("host", "localhost");
        loader.add_source(Box::new(source));

        let config: TestConfig = loader.load().unwrap();
        assert_eq!(config.port, 8080);
        assert_eq!(config.host, "localhost");
    }

    #[test]
    fn test_precedence() {
        let mut loader = ConfigLoader::new();

        // Lower priority source (default values)
        let source1 = MockSource::new("default", 100)
            .with_value("port", 8080i64)
            .with_value("host", "localhost");

        // Higher priority source (overrides)
        let source2 = MockSource::new("override", 200).with_value("port", 9090i64);

        loader.add_source(Box::new(source1));
        loader.add_source(Box::new(source2));

        let config: TestConfig = loader.load().unwrap();
        assert_eq!(config.port, 9090); // Overridden
        assert_eq!(config.host, "localhost"); // From default
    }

    #[test]
    fn test_source_names() {
        let mut loader = ConfigLoader::new();
        loader.add_source(Box::new(MockSource::new("source1", 100)));
        loader.add_source(Box::new(MockSource::new("source2", 200)));
        loader.add_source(Box::new(MockSource::new("source3", 50)));

        let names = loader.source_names();
        // Should be sorted by priority
        assert_eq!(names, vec!["source3", "source1", "source2"]);
    }
}
