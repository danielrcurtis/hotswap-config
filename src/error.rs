//! Error types for hotswap-config.

use std::fmt;

/// Result type alias for hotswap-config operations.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Errors that can occur when working with configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Failed to load configuration from a source.
    #[error("Failed to load configuration: {0}")]
    LoadError(String),

    /// Failed to deserialize configuration.
    #[error("Failed to deserialize configuration: {0}")]
    DeserializationError(String),

    /// Configuration validation failed.
    #[error("Configuration validation failed: {0}")]
    ValidationError(String),

    /// File watching is not supported or failed to initialize.
    #[error("File watching error: {0}")]
    WatchError(String),

    /// Attempted to use a feature that is not enabled.
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(&'static str),

    /// Configuration source does not support watching.
    #[error("Configuration source does not support watching")]
    WatchNotSupported,

    /// IO error occurred.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to parse configuration file.
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[cfg(feature = "rollback")]
    /// Not enough history to rollback the requested number of steps.
    #[error("Insufficient history: cannot rollback {requested} steps (only {available} available)")]
    InsufficientHistory {
        /// Number of steps requested to roll back
        requested: usize,
        /// Number of historical versions available
        available: usize,
    },

    #[cfg(feature = "partial-updates")]
    /// JSON patch operation failed.
    #[error("Patch operation failed: {0}")]
    PatchError(String),

    /// Generic error for other cases.
    #[error("Configuration error: {0}")]
    Other(String),
}

/// Validation error for configuration validation.
#[derive(Debug)]
pub enum ValidationError {
    /// Custom validation error with a message.
    Custom(String),

    /// A specific field has an invalid value.
    InvalidField {
        /// The field name/path
        field: String,
        /// The reason why it's invalid
        reason: String,
    },

    /// Multiple validation errors occurred.
    Multiple(Vec<ValidationError>),
}

impl ValidationError {
    /// Create a custom validation error.
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    /// Create an invalid field error.
    pub fn invalid_field(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidField {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(msg) => write!(f, "{}", msg),
            Self::InvalidField { field, reason } => {
                write!(f, "Field '{}' is invalid: {}", field, reason)
            }
            Self::Multiple(errors) => {
                writeln!(f, "Multiple validation errors:")?;
                for (i, err) in errors.iter().enumerate() {
                    writeln!(f, "  {}. {}", i + 1, err)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ValidationError {}

impl From<ValidationError> for ConfigError {
    fn from(err: ValidationError) -> Self {
        ConfigError::ValidationError(err.to_string())
    }
}
