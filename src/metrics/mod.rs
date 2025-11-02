//! Built-in metrics for configuration operations.
//!
//! Provides OpenTelemetry metrics tracking:
//! - Reload attempts/success/failures
//! - Reload duration
//! - Configuration age
//! - Active subscribers
//! - Validation failures
//!
//! # Examples
//!
//! ```rust,no_run
//! use hotswap_config::prelude::*;
//! use opentelemetry::global;
//!
//! # async fn example() -> Result<()> {
//! let meter = global::meter("my-app");
//!
//! let config = HotswapConfig::builder()
//!     .with_file("config.yaml")
//!     .with_metrics(meter)
//!     .build::<AppConfig>()
//!     .await?;
//! # Ok(())
//! # }
//! # #[derive(serde::Deserialize, Clone)] struct AppConfig {}
//! ```

mod config_metrics;

pub use config_metrics::ConfigMetrics;
