//! Optional advanced features.

#[cfg(feature = "partial-updates")]
pub mod partial;

#[cfg(feature = "partial-updates")]
pub use partial::PartialUpdate;

#[cfg(feature = "rollback")]
pub mod rollback;

#[cfg(feature = "rollback")]
pub use rollback::{ConfigHistory, ConfigVersion, Rollback};

#[cfg(feature = "gradual-rollout")]
pub mod gradual;

#[cfg(feature = "gradual-rollout")]
pub use gradual::{GradualRollout, GradualRolloutExt};
