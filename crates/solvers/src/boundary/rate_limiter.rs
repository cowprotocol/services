use {
    anyhow::Result,
    shared::rate_limiter::{
        RateLimiter as SharedRateLimiter,
        RateLimitingStrategy as SharedRateLimitingStrategy,
    },
    std::{future::Future, str::FromStr},
    thiserror::Error,
};

pub struct RateLimiter {
    inner: SharedRateLimiter,
}

#[derive(Debug, Clone)]
pub struct RateLimitingStrategy {
    inner: SharedRateLimitingStrategy,
}

impl Default for RateLimitingStrategy {
    fn default() -> Self {
        Self {
            inner: SharedRateLimitingStrategy::default(),
        }
    }
}

impl FromStr for RateLimitingStrategy {
    type Err = anyhow::Error;

    fn from_str(config: &str) -> Result<Self> {
        SharedRateLimitingStrategy::from_str(config).map(|strategy| Self { inner: strategy })
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
}
