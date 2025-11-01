//! Core configuration management types.

mod builder;
mod config_handle;
mod loader;

#[cfg(feature = "validation")]
mod validation;

pub use builder::HotswapConfigBuilder;
pub use config_handle::HotswapConfig;
pub(crate) use loader::ConfigLoader;

#[cfg(feature = "validation")]
pub use validation::Validate;
