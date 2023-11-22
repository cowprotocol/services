use {
    anyhow::{ensure, Context, Result},
    shared::rate_limiter::{
        RateLimiter as SharedRateLimiter,
        RateLimiterError as SharedRateLimiterError,
        RateLimitingStrategy as SharedRateLimitingStrategy,
    },
    std::{future::Future, ops::Add, str::FromStr, time::Duration},
    thiserror::Error,
};

pub struct RateLimiter {
    inner: SharedRateLimiter,
    max_retries: usize,
}

#[derive(Debug, Clone)]
pub struct RateLimitingStrategy {
    inner: SharedRateLimitingStrategy,
    max_retries: usize,
}

const DEFAULT_MAX_RETIRES: usize = 1;

impl Default for RateLimitingStrategy {
    fn default() -> Self {
        Self {
            inner: SharedRateLimitingStrategy::default(),
            max_retries: DEFAULT_MAX_RETIRES,
        }
    }
}

impl FromStr for RateLimitingStrategy {
    type Err = anyhow::Error;

    fn from_str(config: &str) -> Result<Self> {
        let mut parts = config.split(',');

        let shared_config = parts.by_ref().take(3).collect::<Vec<_>>().join(",");
        let inner = SharedRateLimitingStrategy::from_str(&shared_config)?;

        let default_max_retries_str = DEFAULT_MAX_RETIRES.to_string();
        let max_retries = parts.next().unwrap_or(&default_max_retries_str);
        let max_retries = max_retries.parse().context("parsing max_retries")?;

        ensure!(
            parts.next().is_none(),
            "extraneous rate limiting parameters"
        );

        Ok(Self { inner, max_retries })
    }
}

#[derive(Error, Debug, Clone, Default, PartialEq)]
pub enum RateLimiterError {
    #[default]
    #[error("rate limited")]
    RateLimited,
}

impl RateLimiter {
    pub fn new(strategy: RateLimitingStrategy, name: String) -> Self {
        Self {
            inner: SharedRateLimiter::from_strategy(strategy.inner, name),
            max_retries: strategy.max_retries,
        }
    }

    pub async fn execute<T>(
        &self,
        task: impl Future<Output = T>,
        requires_back_off: impl Fn(&T) -> bool,
    ) -> Result<T, RateLimiterError> {
        self.inner
            .execute(task, requires_back_off)
            .await
            .map_err(|err| match err {
                SharedRateLimiterError::RateLimited => RateLimiterError::RateLimited,
            })
    }

    pub async fn execute_with_retries<T, F, Fut>(
        &self,
        task: F,
        requires_back_off: impl Fn(&T) -> bool + Clone,
    ) -> Result<T, RateLimiterError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = T>,
    {
        let mut retries = 0;
        while retries < self.max_retries {
            let result = self.execute(task(), requires_back_off.clone()).await;
            let should_retry = match &result {
                Ok(result) => requires_back_off.clone()(result),
                Err(RateLimiterError::RateLimited) => true,
            };

            if should_retry {
                let back_off_duration = self.get_back_off_duration();
                tokio::time::sleep(back_off_duration).await;
                retries += 1;
            } else {
                return result;
            }
        }
        Err(RateLimiterError::RateLimited)
    }

    fn get_back_off_duration(&self) -> Duration {
        self.inner
            .strategy
            .lock()
            .unwrap()
            .get_current_back_off()
            // add 100 millis to make sure the RateLimiter updated it's counter
            .add(Duration::from_millis(100))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn test_execute_with_retries() {
        let strategy = RateLimitingStrategy {
            inner: SharedRateLimitingStrategy::default(),
            max_retries: 2,
        };
        let rate_limiter = RateLimiter::new(strategy, "test".to_string());
        let call_count = AtomicUsize::new(0);

        let task = || {
            let count = call_count.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 1 {
                    Err(RateLimiterError::RateLimited)
                } else {
                    Ok(42)
                }
            }
        };

        let result = rate_limiter
            .execute_with_retries(task, |res| {
                let back_off_required = matches!(res, Err(RateLimiterError::RateLimited));
                back_off_required
            })
            .await
            .and_then(|result: Result<i32, RateLimiterError>| result);
        assert_eq!(result, Ok(42));
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_execute_with_retries_exceeds() {
        let strategy = RateLimitingStrategy {
            inner: SharedRateLimitingStrategy::default(),
            max_retries: 2,
        };
        let rate_limiter = RateLimiter::new(strategy, "test".to_string());
        let call_count = AtomicUsize::new(0);

        let task = || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async move { Err(RateLimiterError::RateLimited) }
        };

        let result = rate_limiter
            .execute_with_retries(task, |res| {
                let back_off_required = matches!(res, Err(RateLimiterError::RateLimited));
                back_off_required
            })
            .await
            .and_then(|result: Result<i32, RateLimiterError>| result);
        assert_eq!(result, Err(RateLimiterError::RateLimited));
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn parse_rate_limiting_strategy() {
        let config_str = "1.5,10,30,3";
        let strategy: RateLimitingStrategy = config_str.parse().unwrap();
        assert_eq!(strategy.inner.back_off_growth_factor, 1.5);
        assert_eq!(strategy.inner.min_back_off, Duration::from_secs(10));
        assert_eq!(strategy.inner.max_back_off, Duration::from_secs(30));
        assert_eq!(strategy.max_retries, 3);
    }

    #[test]
    fn parse_rate_limiting_strategy_with_default_retries() {
        let config_str = "1.5,10,30";
        let strategy: RateLimitingStrategy = config_str.parse().unwrap();
        assert_eq!(strategy.max_retries, DEFAULT_MAX_RETIRES);
    }

    #[test]
    fn parse_invalid_rate_limiting_strategy() {
        let config_str = "invalid";
        assert!(config_str.parse::<RateLimitingStrategy>().is_err());
    }

    #[test]
    fn parse_too_many_args_rate_limiting_strategy() {
        let config_str = "1.5,10,30,3,10";
        assert!(config_str.parse::<RateLimitingStrategy>().is_err());
    }
}
