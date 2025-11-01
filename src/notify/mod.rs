//! Configuration change notification system.
//!
//! Provides file watching and subscriber-based notifications when configuration is reloaded.

pub mod subscriber;
pub mod watcher;

pub use subscriber::{SubscriberRegistry, SubscriptionHandle};
pub use watcher::ConfigWatcher;
