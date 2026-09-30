//! Token-bucket rate limit for `simulateBundle` requests.

use {
    serde::Deserialize,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::Semaphore,
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ThrottlerConfig {
    capacity: usize,
    #[serde(with = "humantime_serde")]
    refill_interval: Duration,
}

/// 10 permits per second: a burst of 10, then one every 100ms.
impl Default for ThrottlerConfig {
    fn default() -> Self {
        Self {
            capacity: 10,
            refill_interval: Duration::from_millis(100),
        }
    }
}

#[derive(Debug)]
pub struct Throttler {
    semaphore: Arc<Semaphore>,
}

impl Throttler {
    /// Spawns a background task that adds 1 permit every `refill_interval`,
    /// up to `capacity`. Runs for the lifetime of the runtime.
    pub fn new(config: ThrottlerConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.capacity));
        tokio::task::spawn({
            let semaphore = semaphore.clone();
            async move {
                loop {
                    tokio::time::sleep(config.refill_interval).await;
                    if semaphore.available_permits() < config.capacity {
                        semaphore.add_permits(1);
                        tracing::trace!(
                            available = semaphore.available_permits(),
                            "refilled a simulateBundle permit"
                        );
                    }
                }
            }
        });
        Self { semaphore }
    }

    /// Resolves once a permit is available. The permit is consumed (leaked)
    /// on drop, so the refill task is the sole source of new permits.
    pub async fn throttle(&self) {
        // Logged before the wait: a caller that times out mid-wait never
        // reaches the line below.
        if self.semaphore.available_permits() == 0 {
            tracing::debug!("waiting for a simulateBundle permit");
        }
        let started = Instant::now();
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore is never closed")
            .forget();
        tracing::debug!(
            waited_ms = started.elapsed().as_millis() as u64,
            remaining = self.semaphore.available_permits(),
            "simulateBundle permit consumed"
        );
    }
}
