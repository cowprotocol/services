use {
    anyhow::{ensure, Context, Result},
    std::{
        fmt::{Display, Formatter},
        future::Future,
        num::ParseFloatError,
        str::FromStr,
        sync::{Arc, Mutex, MutexGuard},
        time::{Duration, Instant},
    },
    thiserror::Error,
};

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "rate_limiter")]
struct Metrics {
    /// Number of requests dropped while being rate limited.
    #[metric(labels("endpoint"))]
    requests_dropped: prometheus::IntCounterVec,
    /// Number of responses indicating a rate limiting error.
    #[metric(labels("endpoint"))]
    rate_limited_requests: prometheus::IntCounterVec,
    /// Number of successful requests.
    #[metric(labels("endpoint"))]
    successful_requests: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
}

#[derive(Debug, Clone)]
pub struct Strategy {
    drop_requests_until: Instant,
    /// How many requests got rate limited in a row.
    times_rate_limited: u64,
    back_off_growth_factor: f64,
    min_back_off: Duration,
    max_back_off: Duration,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::try_new(1.0, Duration::default(), Duration::default()).unwrap()
    }
}

impl Display for Strategy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RateLimitingStrategy{{ min_back_off: {:?}, max_back_off: {:?}, growth_factor: {:?} }}",
            self.min_back_off, self.max_back_off, self.back_off_growth_factor
        )
    }
}

impl FromStr for Strategy {
    type Err = anyhow::Error;

    fn from_str(config: &str) -> Result<Self> {
        let mut parts = config.split(',');
        let back_off_growth_factor = parts.next().context("missing back_off_growth_factor")?;
        let min_back_off = parts.next().context("missing min_back_off")?;
        let max_back_off = parts.next().context("missing max_back_off")?;
        ensure!(
            parts.next().is_none(),
            "extraneous rate limiting parameters"
        );
        let back_off_growth_factor: f64 = back_off_growth_factor
            .parse()
            .context("parsing back_off_growth_factor")?;
        let min_back_off = duration_from_seconds(min_back_off).context("parsing min_back_off")?;
        let max_back_off = duration_from_seconds(max_back_off).context("parsing max_back_off")?;
        Self::try_new(back_off_growth_factor, min_back_off, max_back_off)
    }
}

impl Strategy {
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
        std::cmp::min(
            Duration::try_from_secs_f64(back_off_secs).unwrap_or(self.max_back_off),
            self.max_back_off,
        )
    }

    /// Returns updated back off if no other thread increased it in the mean
    /// time.
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

        let new_back_off = self.get_current_back_off();
        self.times_rate_limited += 1;
        self.drop_requests_until = Instant::now() + new_back_off;
        Some(new_back_off)
    }

    /// Returns number of times we got rate limited in a row if we are currently
    /// allowing requests.
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
    pub strategy: Mutex<Strategy>,
    pub name: String,
}

impl RateLimiter {
    fn strategy(&self) -> MutexGuard<Strategy> {
        self.strategy.lock().unwrap()
    }

    pub fn from_strategy(strategy: Strategy, name: String) -> Self {
        let metrics = metrics();
        metrics.requests_dropped.with_label_values(&[&name]).reset();
        metrics
            .rate_limited_requests
            .with_label_values(&[&name])
            .reset();
        metrics
            .successful_requests
            .with_label_values(&[&name])
            .reset();
        Self {
            strategy: Mutex::new(strategy),
            name,
        }
    }

    /// Create a testing rate limiter.
    pub fn test() -> Arc<Self> {
        Arc::new(RateLimiter::from_strategy(
            Default::default(),
            "test".into(),
        ))
    }
}

#[derive(Error, Debug, Clone, Default, PartialEq)]
pub enum Error {
    #[default]
    #[error("rate limited")]
    RateLimited,
}

impl RateLimiter {
    /// If a task produces a result which indicates rate limiting is required
    /// future requests will get dropped for some time. Every successive
    /// response like that increases that time exponentially. When a task
    /// eventually returns a normal result again future tasks will no longer get
    /// dropped until the next rate limiting response occurs.
    pub async fn execute<T>(
        &self,
        task: impl Future<Output = T>,
        requires_back_off: impl Fn(&T) -> bool,
    ) -> Result<T, Error> {
        let times_rate_limited = self
            .strategy()
            .times_rate_limited(Instant::now(), &self.name);
        let times_rate_limited = match times_rate_limited {
            None => {
                tracing::warn!(?self.name, "dropping task because API is currently rate limited");
                return Err(Error::RateLimited);
            }
            Some(times_rate_limited) => times_rate_limited,
        };

        let result = task.await;

        if requires_back_off(&result) {
            let new_back_off = self
                .strategy()
                .response_rate_limited(times_rate_limited, &self.name);
            if let Some(new_back_off) = new_back_off {
                tracing::warn!(?self.name, ?new_back_off, "extended rate limiting");
            }
        } else {
            self.strategy().response_ok(&self.name);
            if times_rate_limited > 0 {
                tracing::debug!(?self.name, "reset rate limit");
            }
        }

        Ok(result)
    }

    pub async fn execute_with_back_off<T>(
        &self,
        task: impl Future<Output = T>,
        requires_back_off: impl Fn(&T) -> bool,
    ) -> Result<T, Error> {
        if let Some(back_off_duration) = self.get_back_off_duration_if_limited() {
            tokio::time::sleep(back_off_duration).await;
        }

        self.execute(task, requires_back_off).await
    }

    fn get_back_off_duration_if_limited(&self) -> Option<Duration> {
        let strategy = self.strategy.lock().unwrap();
        let now = Instant::now();

        if strategy.drop_requests_until > now {
            let back_off_duration = strategy.drop_requests_until - now;
            Some(back_off_duration)
        } else {
            None
        }
    }
}

/// Shared module with common back-off checks.
pub mod back_off {
    use reqwest::Response;

    /// Determines if the HTTP response indicates that the API should back off
    /// for a while.
    pub fn on_http_429(response: &Result<Response, reqwest::Error>) -> bool {
        matches!(response, Ok(response) if response.status() == 429)
    }
}

fn duration_from_seconds(s: &str) -> Result<Duration, ParseFloatError> {
    Ok(Duration::from_secs_f64(s.parse()?))
}

#[cfg(test)]
mod tests {
    use {super::*, futures::FutureExt, std::ops::Add, tokio::time::sleep};

    #[test]
    fn current_back_off_does_not_panic() {
        let max = Duration::from_secs(60);
        let back_off = Strategy {
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
        let back_off = Strategy {
            drop_requests_until: Instant::now(),
            times_rate_limited: 3,
            back_off_growth_factor: 2.,
            min_back_off: Duration::from_millis(16),
            max_back_off: max,
        }
        .get_current_back_off();
        assert_eq!(Duration::from_millis(16 * 8), back_off);
    }

    #[tokio::test]
    async fn drops_requests_correctly() {
        let strategy =
            Strategy::try_new(2.0, Duration::from_millis(20), Duration::from_millis(50)).unwrap();
        let rate_limiter = RateLimiter::from_strategy(strategy, "test".into());

        let result = rate_limiter.execute(async { 1 }, |_| false).await;
        assert!(matches!(result, Ok(1)));
        assert_eq!(
            // get_current_back_off returns how much the back off should be extended if we
            // were to encounter an error now, therefore we start with 20
            Duration::from_millis(20),
            rate_limiter.strategy().get_current_back_off()
        );

        // generate first response requiring a rate limit
        let result = rate_limiter.execute(async { 2 }, |_| true).await;
        // return actual result even if response suggest a rate limit
        assert!(matches!(result, Ok(2)));
        assert_eq!(
            Duration::from_millis(40),
            rate_limiter.strategy().get_current_back_off()
        );

        let result = rate_limiter
            .execute(
                async {
                    unreachable!("don't evaluate closure when rate limited");
                    #[allow(unreachable_code)] // to help the type checker
                    3
                },
                |_| unreachable!("don't evaluate closure when rate limited"),
            )
            .now_or_never()
            .expect("tasks return immediately during back off period");
        assert!(matches!(result, Err(Error::RateLimited)));

        // sleep until new requests are allowed
        sleep(Duration::from_millis(20)).await;

        // generate another response requiring a rate limit
        let result = rate_limiter.execute(async { 4 }, |_| true).await;
        assert!(matches!(result, Ok(4)));
        assert_eq!(
            // back off got increased but doesn't exceed max_back_off
            Duration::from_millis(50),
            rate_limiter.strategy().get_current_back_off()
        );
    }

    #[tokio::test]
    async fn test_execute_with_no_back_off() {
        let timeout = Duration::from_secs(30);
        let strategy = Strategy::try_new(1.0, timeout, timeout).unwrap();
        let original_drop_until = strategy.drop_requests_until;
        let rate_limiter = RateLimiter::from_strategy(strategy, "test_no_back_off".to_string());

        let result = rate_limiter
            .execute_with_back_off(async { 1 }, |_| false)
            .now_or_never()
            .unwrap()
            .unwrap();

        assert_eq!(result, 1);
        {
            let current_strategy = rate_limiter.strategy.lock().unwrap();
            assert!(current_strategy.drop_requests_until < original_drop_until.add(timeout));
        }

        let result = rate_limiter.execute(async { 1 }, |_| false).await.unwrap();
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_execute_with_back_off() {
        let timeout = Duration::from_millis(50);
        let strategy = Strategy::try_new(1.0, timeout, timeout).unwrap();
        let original_drop_until = strategy.drop_requests_until;
        let rate_limiter = RateLimiter::from_strategy(strategy, "test_back_off".to_string());

        // start the back off
        let result = rate_limiter
            .execute_with_back_off(async { 1 }, |_| true)
            .await
            .unwrap();

        assert_eq!(result, 1);
        let drop_until = {
            let current_strategy = rate_limiter.strategy.lock().unwrap();
            let drop_until = current_strategy.drop_requests_until;
            assert!(drop_until >= original_drop_until.add(timeout));
            drop_until
        };

        // back off is not over, expecting a RateLimiterError
        let result = rate_limiter.execute(async { 1 }, |_| false).await;
        assert_eq!(result, Err(Error::RateLimited));
        {
            let current_strategy = rate_limiter.strategy.lock().unwrap();
            assert_eq!(current_strategy.drop_requests_until, drop_until);
        }

        // back off is over
        let result = rate_limiter
            .execute_with_back_off(async { 1 }, |_| false)
            .await
            .unwrap();
        assert_eq!(result, 1);
    }
}
