//! Configuration validation support.

use crate::error::ValidationError;

/// Trait for configuration validation.
///
/// Implement this trait on your configuration types to enable automatic validation
/// before updates are applied.
///
/// # Examples
///
/// ```rust
/// use hotswap_config::core::Validate;
/// use hotswap_config::error::ValidationError;
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, Clone)]
/// struct ServerConfig {
///     port: u16,
///     max_connections: usize,
/// }
///
/// impl Validate for ServerConfig {
///     fn validate(&self) -> Result<(), ValidationError> {
///         if self.port < 1024 {
///             return Err(ValidationError::invalid_field(
///                 "port",
///                 "must be >= 1024 (privileged ports require root)"
///             ));
///         }
///
///         if self.max_connections == 0 {
///             return Err(ValidationError::invalid_field(
///                 "max_connections",
///                 "must be greater than 0"
///             ));
///         }
///
///         Ok(())
///     }
/// }
/// ```
pub trait Validate {
    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Should return a `ValidationError` describing what validation failed.
    fn validate(&self) -> Result<(), ValidationError>;
}
