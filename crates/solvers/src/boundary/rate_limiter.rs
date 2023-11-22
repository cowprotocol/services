use {
    anyhow::{ensure, Context, Result},
    shared::rate_limiter::{
        RateLimiter as SharedRateLimiter,
        RateLimitingStrategy as SharedRateLimitingStrategy,
    },
    std::{future::Future, str::FromStr, time::Duration},
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

const DEFAULT_MAX_RETIRES: usize = 2;

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

#[derive(Error, Debug, Clone, Default)]
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
            .map_err(|_| RateLimiterError::RateLimited)
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
            match self.execute(task(), requires_back_off.clone()).await {
                Ok(result) => return Ok(result),
                Err(RateLimiterError::RateLimited) => {
                    let back_off_duration = self.get_back_off_duration();
                    tokio::time::sleep(back_off_duration).await;
                    retries += 1;
                }
            }
        }
        Err(RateLimiterError::RateLimited)
    }

    fn get_back_off_duration(&self) -> Duration {
        self.inner.strategy.lock().unwrap().get_current_back_off()
    }
}
