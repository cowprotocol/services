use crate::metrics;
use anyhow::{anyhow, ensure, Result};
use reqwest::Response;
use std::{
    future::Future,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

/// Extracts the bytes of the response up to some size limit.
///
/// Returns an error if the byte limit was exceeded.
pub async fn response_body_with_size_limit(
    response: &mut Response,
    limit: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let slice: &[u8] = &chunk;
        if bytes.len() + slice.len() > limit {
            return Err(anyhow!("size limit exceeded"));
        }
        bytes.extend_from_slice(slice);
    }
    Ok(bytes)
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "rate_limiter")]
struct Metrics {
    /// Number of requests dropped while being rate limited.
    #[metric(labels("endpoint"))]
    requests_dropped: prometheus::CounterVec,
    /// Number of responses indicating a rate limiting error.
    #[metric(labels("endpoint"))]
    rate_limited_requests: prometheus::CounterVec,
    /// Number of successful requests.
    #[metric(labels("endpoint"))]
    successful_requests: prometheus::CounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
}

#[derive(Debug, Clone)]
pub struct RateLimitingStrategy {
    drop_requests_until: Instant,
    /// How many requests got rate limited in a row.
    times_rate_limited: u64,
    back_off_growth_factor: f64,
    min_back_off: Duration,
    max_back_off: Duration,
}

impl RateLimitingStrategy {
    pub fn try_new(
        back_off_growth_factor: f64,
        min_back_off: Duration,
        max_back_off: Duration,
    ) -> Result<Self> {
        ensure!(
            back_off_growth_factor.is_normal(),
            "back_off_growth_factor must be a normal f64"
        );
        ensure!(
            back_off_growth_factor >= 1.0,
            "back_off_growth_factor needs to be at least 1.0"
        );
        ensure!(
            min_back_off <= max_back_off,
            "min_back_off needs to be <= max_back_off"
        );
        Ok(Self {
            drop_requests_until: Instant::now(),
            times_rate_limited: 0,
            back_off_growth_factor,
            min_back_off,
            max_back_off,
        })
    }

    /// Resets back off and stops rate limiting requests.
    pub fn response_ok(&mut self, name: &str) {
        metrics()
            .successful_requests
            .with_label_values(&[name])
            .inc();
        self.times_rate_limited = 0;
        self.drop_requests_until = Instant::now();
    }

    /// Calculates back off based on how often we got rate limited in a row.
    fn get_current_back_off(&self) -> Duration {
        let factor = self
            .back_off_growth_factor
            .powf(self.times_rate_limited as f64);
        let back_off_secs = self.min_back_off.as_secs_f64() * factor;
        if !back_off_secs.is_normal() || back_off_secs < 0. || back_off_secs > u64::MAX as f64 {
            // This would cause a panic in `Duration::from_secs_f64()`
            // TODO refactor this when `Duration::try_from_secs_f64()` gets stabilized:
            // https://doc.rust-lang.org/stable/std/time/struct.Duration.html#method.try_from_secs_f64
            return self.max_back_off;
        }
        let current_back_off = Duration::from_secs_f64(back_off_secs);
        std::cmp::min(self.max_back_off, current_back_off)
    }

    /// Returns updated back off if no other thread increased it in the mean time.
    pub fn response_rate_limited(
        &mut self,
        previous_rate_limits: u64,
        name: &str,
    ) -> Option<Duration> {
        metrics()
            .rate_limited_requests
            .with_label_values(&[name])
            .inc();
        if self.times_rate_limited != previous_rate_limits {
            // Don't increase back off if somebody else already updated it in the meantime.
            return None;
        }

        self.times_rate_limited += 1;
        let new_back_off = self.get_current_back_off();
        self.drop_requests_until = Instant::now() + new_back_off;
        Some(new_back_off)
    }

    /// Returns number of times we got rate limited in a row if we are currently allowing requests.
    pub fn times_rate_limited(&self, now: Instant, name: &str) -> Option<u64> {
        if self.drop_requests_until > now {
            metrics().requests_dropped.with_label_values(&[name]).inc();
            return None;
        }

        Some(self.times_rate_limited)
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    pub strategy: Mutex<RateLimitingStrategy>,
    pub name: String,
}

impl RateLimiter {
    fn strategy(&self) -> MutexGuard<RateLimitingStrategy> {
        self.strategy.lock().unwrap()
    }

    pub fn from_strategy(strategy: RateLimitingStrategy, name: String) -> Self {
        Self {
            strategy: Mutex::new(strategy),
            name,
        }
    }
}

impl RateLimiter {
    /// If a task produces a result which indicates rate limiting is required future requests
    /// will get dropped for some time. Every successive response like that increases that time exponentially.
    /// When a task eventually returns a normal result again future tasks will no longer get
    /// dropped until the next rate limiting response occurs.
    pub async fn execute<T, E>(
        &self,
        task: impl Future<Output = Result<T, E>>,
        requires_back_off: impl Fn(&Result<T, E>) -> bool,
    ) -> Result<T>
    where
        anyhow::Error: From<E>,
    {
        let times_rate_limited = match self
            .strategy()
            .times_rate_limited(Instant::now(), &self.name)
        {
            None => {
                tracing::warn!(?self.name, "dropping task because API is currently rate limited");
                anyhow::bail!("backing off rate limit");
            }
            Some(times_rate_limited) => times_rate_limited,
        };

        let result = task.await;

        if requires_back_off(&result) {
            if let Some(new_back_off) = self
                .strategy()
                .response_rate_limited(times_rate_limited, &self.name)
            {
                tracing::warn!(?self.name, ?new_back_off, "extended rate limiting");
            }
        } else {
            self.strategy().response_ok(&self.name);
            tracing::debug!(?self.name, "reset rate limit");
        }

        result.map_err(anyhow::Error::from)
    }
}

pub fn requires_back_off(response: &Result<Response, reqwest::Error>) -> bool {
    matches!(response, Ok(response) if response.status() == 429)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::{self, StreamExt};
    use reqwest::Client;
    use tokio::time::{sleep_until, Instant as TokioInstant};

    #[tokio::test]
    #[ignore]
    async fn real() {
        let client = Client::default();

        let mut response = client.get("https://cow.fi").send().await.unwrap();
        let bytes = response_body_with_size_limit(&mut response, 10).await;
        dbg!(&bytes);
        assert!(bytes.is_err());

        let mut response = client.get("https://cow.fi").send().await.unwrap();
        let bytes = response_body_with_size_limit(&mut response, 1_000_000)
            .await
            .unwrap();
        dbg!(bytes.len());
        let text = std::str::from_utf8(&bytes).unwrap();
        dbg!(text);
    }

    #[tokio::test]
    #[ignore]
    async fn rate_limited_requests() {
        let client = Client::default();

        let url = "https://apiv5.paraswap.io/prices?srcToken=0x99d8a9c45b2eca8864373a26d1459e3dff1e17f3&destToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&srcDecimals=18&destDecimals=6&amount=100000000&side=BUY&network=1&excludeDEXS=ParaSwapPool4";
        let strategy = RateLimitingStrategy::try_new(
            2.0,
            Duration::from_millis(16),
            Duration::from_millis(20_000),
        )
        .unwrap();
        let rate_limiter = RateLimiter::from_strategy(strategy, "test".into());
        // note that 1_000 requests will not always trigger a rate limit
        let mut stream = stream::iter(0..1_000).map(|_| async {}).buffer_unordered(2);
        while stream.next().await.is_some() {
            let request = client.get(url).send();
            let response = rate_limiter
                .execute(request, super::requires_back_off)
                .await;
            match &response {
                Ok(response) => println!("{}", response.status()),
                Err(e) => {
                    println!("error: {}", e);
                    let instant = rate_limiter.strategy.lock().unwrap().drop_requests_until;
                    println!(
                        "sleeping for {} milliseconds",
                        instant.duration_since(Instant::now()).as_millis()
                    );
                    sleep_until(TokioInstant::from_std(instant)).await;
                }
            }
        }
    }

    #[test]
    fn current_back_off_does_not_panic() {
        let max = Duration::from_secs(60);
        let back_off = RateLimitingStrategy {
            drop_requests_until: Instant::now(),
            times_rate_limited: 1,
            // internal calculations don't overflow `Duration`
            back_off_growth_factor: f64::MAX,
            min_back_off: Duration::from_millis(16),
            max_back_off: max,
        }
        .get_current_back_off();
        assert_eq!(max, back_off);

        let max = Duration::from_secs(60);
        let back_off = RateLimitingStrategy {
            drop_requests_until: Instant::now(),
            times_rate_limited: 3,
            back_off_growth_factor: 2.,
            min_back_off: Duration::from_millis(16),
            max_back_off: max,
        }
        .get_current_back_off();
        assert_eq!(Duration::from_millis(16 * 8), back_off);
    }
}
