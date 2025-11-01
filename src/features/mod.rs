//! Optional advanced features.

#[cfg(feature = "partial-updates")]
pub mod partial;

#[cfg(feature = "rollback")]
pub mod rollback;

#[cfg(feature = "gradual-rollout")]
pub mod gradual;
