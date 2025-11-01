//! Subscriber-based notifications for configuration changes.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Handle for a subscription that can be dropped to unsubscribe.
///
/// When the handle is dropped, the subscription is automatically removed.
pub struct SubscriptionHandle {
    id: usize,
    registry: Arc<RwLock<SubscriberRegistryInner>>,
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        let id = self.id;
        let registry = Arc::clone(&self.registry);
        tokio::spawn(async move {
            let mut inner = registry.write().await;
            inner.subscribers.retain(|(sub_id, _)| *sub_id != id);
        });
    }
}

/// Internal subscriber registry state.
struct SubscriberRegistryInner {
    subscribers: Vec<(usize, Box<dyn Fn() + Send + Sync>)>,
    next_id: usize,
}

/// Registry for managing configuration change subscribers.
///
/// Allows code to register callbacks that are invoked whenever the
/// configuration is updated.
///
/// # Examples
///
/// ```rust,no_run
/// use hotswap_config::notify::SubscriberRegistry;
/// use std::sync::Arc;
///
/// # async fn example() {
/// let registry = SubscriberRegistry::new();
///
/// let handle = registry.subscribe(|| {
///     println!("Config changed!");
/// }).await;
///
/// // Notify all subscribers
/// registry.notify_all().await;
///
/// // Unsubscribe by dropping the handle
/// drop(handle);
/// # }
/// ```
pub struct SubscriberRegistry {
    inner: Arc<RwLock<SubscriberRegistryInner>>,
}

impl SubscriberRegistry {
    /// Create a new subscriber registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(SubscriberRegistryInner {
                subscribers: Vec::new(),
                next_id: 0,
            })),
        }
    }

    /// Subscribe to configuration changes.
    ///
    /// The provided callback will be invoked whenever the configuration
    /// is updated. Returns a handle that can be dropped to unsubscribe.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call when config changes
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::notify::SubscriberRegistry;
    /// # async fn example() {
    /// let registry = SubscriberRegistry::new();
    ///
    /// let handle = registry.subscribe(|| {
    ///     println!("Configuration updated!");
    /// }).await;
    ///
    /// // Later, unsubscribe
    /// drop(handle);
    /// # }
    /// ```
    pub async fn subscribe<F>(&self, callback: F) -> SubscriptionHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        let mut inner = self.inner.write().await;
        let id = inner.next_id;
        inner.next_id += 1;
        inner.subscribers.push((id, Box::new(callback)));

        SubscriptionHandle {
            id,
            registry: Arc::clone(&self.inner),
        }
    }

    /// Notify all subscribers of a configuration change.
    ///
    /// This calls all registered callbacks in the order they were subscribed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::notify::SubscriberRegistry;
    /// # async fn example() {
    /// let registry = SubscriberRegistry::new();
    ///
    /// registry.subscribe(|| println!("Subscriber 1")).await;
    /// registry.subscribe(|| println!("Subscriber 2")).await;
    ///
    /// // Notify all subscribers
    /// registry.notify_all().await;
    /// # }
    /// ```
    pub async fn notify_all(&self) {
        let inner = self.inner.read().await;
        for (_id, callback) in &inner.subscribers {
            callback();
        }
    }

    /// Get the number of active subscribers.
    pub async fn subscriber_count(&self) -> usize {
        let inner = self.inner.read().await;
        inner.subscribers.len()
    }
}

impl Default for SubscriberRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SubscriberRegistry {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_subscribe_and_notify() {
        let registry = SubscriberRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = Arc::clone(&counter);
        let _handle = registry
            .subscribe(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        registry.notify_all().await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        registry.notify_all().await;
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let registry = SubscriberRegistry::new();
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));

        let counter1_clone = Arc::clone(&counter1);
        let _handle1 = registry
            .subscribe(move || {
                counter1_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let counter2_clone = Arc::clone(&counter2);
        let _handle2 = registry
            .subscribe(move || {
                counter2_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        registry.notify_all().await;
        assert_eq!(counter1.load(Ordering::SeqCst), 1);
        assert_eq!(counter2.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        let registry = SubscriberRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = Arc::clone(&counter);
        let handle = registry
            .subscribe(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        registry.notify_all().await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Unsubscribe by dropping handle
        drop(handle);

        // Give the drop task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        registry.notify_all().await;
        // Counter should still be 1 (not incremented)
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let registry = SubscriberRegistry::new();
        assert_eq!(registry.subscriber_count().await, 0);

        let _handle1 = registry.subscribe(|| {}).await;
        assert_eq!(registry.subscriber_count().await, 1);

        let _handle2 = registry.subscribe(|| {}).await;
        assert_eq!(registry.subscriber_count().await, 2);

        drop(_handle1);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert_eq!(registry.subscriber_count().await, 1);
    }

    #[tokio::test]
    async fn test_clone_registry() {
        let registry = SubscriberRegistry::new();
        let registry2 = registry.clone();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let _handle = registry
            .subscribe(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // Notify via clone
        registry2.notify_all().await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
