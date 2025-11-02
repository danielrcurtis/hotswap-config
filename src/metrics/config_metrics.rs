//! Configuration metrics tracking using OpenTelemetry.

use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter};
use std::sync::Arc;
use std::time::Instant;

/// Metrics collector for configuration operations.
///
/// Tracks reload attempts, success/failure rates, latencies, and subscriber counts
/// using OpenTelemetry metrics.
///
/// # Examples
///
/// ```rust,no_run
/// use hotswap_config::metrics::ConfigMetrics;
/// use opentelemetry::global;
///
/// let meter = global::meter("hotswap-config");
/// let metrics = ConfigMetrics::new(meter);
///
/// // Track a reload operation
/// let timer = metrics.start_reload();
/// // ... perform reload ...
/// metrics.record_reload_success(timer);
/// ```
#[derive(Clone)]
pub struct ConfigMetrics {
    reload_attempts: Counter<u64>,
    reload_success: Counter<u64>,
    reload_failures: Counter<u64>,
    reload_duration: Histogram<f64>,
    config_age_seconds: Gauge<i64>,
    active_subscribers: Gauge<i64>,
    validation_failures: Counter<u64>,
    last_update: Arc<parking_lot::Mutex<Instant>>,
}

impl ConfigMetrics {
    /// Create a new metrics collector with the provided meter.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use hotswap_config::metrics::ConfigMetrics;
    /// use opentelemetry::global;
    ///
    /// let meter = global::meter("hotswap-config");
    /// let metrics = ConfigMetrics::new(meter);
    /// ```
    pub fn new(meter: Meter) -> Self {
        let reload_attempts = meter
            .u64_counter("hotswap_config.reload.attempts")
            .with_description("Total number of reload attempts")
            .build();

        let reload_success = meter
            .u64_counter("hotswap_config.reload.success")
            .with_description("Number of successful reloads")
            .build();

        let reload_failures = meter
            .u64_counter("hotswap_config.reload.failures")
            .with_description("Number of failed reloads")
            .build();

        let reload_duration = meter
            .f64_histogram("hotswap_config.reload.duration")
            .with_description("Duration of reload operations in seconds")
            .with_unit("s")
            .build();

        let config_age_seconds = meter
            .i64_gauge("hotswap_config.age")
            .with_description("Time since last configuration update in seconds")
            .with_unit("s")
            .build();

        let active_subscribers = meter
            .i64_gauge("hotswap_config.subscribers.active")
            .with_description("Number of active subscribers")
            .build();

        let validation_failures = meter
            .u64_counter("hotswap_config.validation.failures")
            .with_description("Number of validation failures")
            .build();

        Self {
            reload_attempts,
            reload_success,
            reload_failures,
            reload_duration,
            config_age_seconds,
            active_subscribers,
            validation_failures,
            last_update: Arc::new(parking_lot::Mutex::new(Instant::now())),
        }
    }

    /// Start a reload operation timer.
    ///
    /// Returns an `Instant` that should be passed to `record_reload_success` or
    /// `record_reload_failure` when the operation completes.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// let timer = metrics.start_reload();
    /// // ... perform reload ...
    /// metrics.record_reload_success(timer);
    /// ```
    pub fn start_reload(&self) -> Instant {
        self.reload_attempts.add(1, &[]);
        Instant::now()
    }

    /// Record a successful reload operation.
    ///
    /// # Arguments
    ///
    /// * `start` - The `Instant` returned from `start_reload()`
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// let timer = metrics.start_reload();
    /// // ... perform reload ...
    /// metrics.record_reload_success(timer);
    /// ```
    pub fn record_reload_success(&self, start: Instant) {
        let duration = start.elapsed().as_secs_f64();
        self.reload_success.add(1, &[]);
        self.reload_duration.record(duration, &[]);

        // Update last update time
        *self.last_update.lock() = Instant::now();
    }

    /// Record a failed reload operation.
    ///
    /// # Arguments
    ///
    /// * `start` - The `Instant` returned from `start_reload()`
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// let timer = metrics.start_reload();
    /// // ... perform reload that fails ...
    /// metrics.record_reload_failure(timer);
    /// ```
    pub fn record_reload_failure(&self, start: Instant) {
        let duration = start.elapsed().as_secs_f64();
        self.reload_failures.add(1, &[]);
        self.reload_duration.record(duration, &[]);
    }

    /// Record a validation failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// metrics.record_validation_failure();
    /// ```
    pub fn record_validation_failure(&self) {
        self.validation_failures.add(1, &[]);
    }

    /// Update the number of active subscribers.
    ///
    /// # Arguments
    ///
    /// * `count` - The current number of active subscribers
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// metrics.update_subscriber_count(5);
    /// ```
    pub fn update_subscriber_count(&self, count: i64) {
        self.active_subscribers.record(count, &[]);
    }

    /// Update the configuration age metric.
    ///
    /// This should be called periodically to track how stale the configuration is.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// metrics.update_config_age();
    /// ```
    pub fn update_config_age(&self) {
        let age_secs = self.last_update.lock().elapsed().as_secs() as i64;
        self.config_age_seconds.record(age_secs, &[]);
    }

    /// Record an update operation (manual update, not reload).
    ///
    /// Updates the last update timestamp used for config age tracking.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use hotswap_config::metrics::ConfigMetrics;
    /// # use opentelemetry::global;
    /// # let metrics = ConfigMetrics::new(global::meter("test"));
    /// metrics.record_update();
    /// ```
    pub fn record_update(&self) {
        *self.last_update.lock() = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::global;

    #[test]
    fn test_metrics_creation() {
        let meter = global::meter("test");
        let metrics = ConfigMetrics::new(meter);

        // Test basic operations don't panic
        let timer = metrics.start_reload();
        metrics.record_reload_success(timer);

        let timer = metrics.start_reload();
        metrics.record_reload_failure(timer);

        metrics.record_validation_failure();
        metrics.update_subscriber_count(5);
        metrics.update_config_age();
        metrics.record_update();
    }

    #[test]
    fn test_metrics_clone() {
        let meter = global::meter("test");
        let metrics = ConfigMetrics::new(meter);
        let metrics2 = metrics.clone();

        // Both should work independently
        let timer1 = metrics.start_reload();
        let timer2 = metrics2.start_reload();

        metrics.record_reload_success(timer1);
        metrics2.record_reload_success(timer2);
    }

    #[test]
    fn test_duration_tracking() {
        let meter = global::meter("test");
        let metrics = ConfigMetrics::new(meter);

        let timer = metrics.start_reload();
        std::thread::sleep(std::time::Duration::from_millis(10));
        metrics.record_reload_success(timer);

        // Verify duration was recorded (should be > 0)
        // Note: We can't easily verify the exact value without accessing internal state
    }
}
