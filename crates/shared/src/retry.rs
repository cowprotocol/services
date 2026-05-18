//! Bounded retry-with-sleep for transient failures.
//!
//! Calls the supplied future-producing closure up to a fixed number of times,
//! sleeping with small jitter between attempts so concurrent retriers don't
//! all wake up at once. Returns `Ok` on the first success, otherwise an
//! `Err(Vec<E>)` containing every observed error in order — useful for
//! diagnosing whether a permanent failure was masked as a flake.
use {
    rand::Rng,
    std::{future::Future, time::Duration},
};

const MAX_RETRIES: usize = 5;

/// Retry on every error.
pub async fn retry_with_sleep<F, T, E>(future: impl Fn() -> F) -> Result<T, Vec<E>>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    retry_with_sleep_if(future, |_| true).await
}

/// Retry only when `should_retry(&err)` returns true. Permanent errors
/// (e.g. contract reverts, bad input) bail out immediately so callers don't
/// waste sleep budget on something that won't get better.
pub async fn retry_with_sleep_if<F, T, E, P>(
    future: impl Fn() -> F,
    should_retry: P,
) -> Result<T, Vec<E>>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
    P: Fn(&E) -> bool,
{
    let mut errors = Vec::new();
    for attempt in 0..MAX_RETRIES {
        match future().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                let retryable = should_retry(&err);
                errors.push(err);
                if !retryable || attempt + 1 == MAX_RETRIES {
                    return Err(errors);
                }
                let timeout_with_jitter = 50u64 + rand::rng().random_range(0..=50);
                tokio::time::sleep(Duration::from_millis(timeout_with_jitter)).await;
            }
        }
    }
    Err(errors)
}
