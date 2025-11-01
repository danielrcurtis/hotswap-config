//! Core configuration management types.

mod config_handle;
mod builder;

#[cfg(feature = "validation")]
mod validation;

pub use config_handle::HotswapConfig;
pub use builder::HotswapConfigBuilder;

#[cfg(feature = "validation")]
pub use validation::Validate;
