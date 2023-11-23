use {
    anyhow::Result,
    shared::rate_limiter::{
        RateLimiter as SharedRateLimiter,
        RateLimiterError as SharedRateLimiterError,
        RateLimitingStrategy as SharedRateLimitingStrategy,
    },
    std::{future::Future, time::Duration},
    thiserror::Error,
};

pub struct RateLimiter {
    inner: SharedRateLimiter,
}

#[derive(Debug, Clone)]
pub struct RateLimitingStrategy {
    inner: SharedRateLimitingStrategy,
}

impl RateLimitingStrategy {
    pub fn try_new(
        back_off_growth_factor: f64,
        min_back_off: Duration,
        max_back_off: Duration,
    ) -> Result<Self> {
        SharedRateLimitingStrategy::try_new(back_off_growth_factor, min_back_off, max_back_off)
            .map(|shared| Self { inner: shared })
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
        }
    }

    pub async fn execute_with_back_off<T>(
        &self,
        task: impl Future<Output = T>,
        requires_back_off: impl Fn(&T) -> bool,
    ) -> Result<T, RateLimiterError> {
        self.inner
            .execute_with_back_off(task, requires_back_off)
            .await
            .map_err(|err| match err {
                SharedRateLimiterError::RateLimited => RateLimiterError::RateLimited,
            })
    }
}
