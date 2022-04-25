use anyhow::{anyhow, ensure, Result};
use reqwest::{RequestBuilder, Response};
use std::{
    sync::Mutex,
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

#[derive(Debug, Clone)]
pub struct RateLimitingStrategy {
    drop_requests_until: Instant,
    next_back_off: Duration,
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
            back_off_growth_factor > 1.0,
            "back_off_growth_factor needs to be greater than 1.0"
        );
        ensure!(
            min_back_off <= max_back_off,
            "min_back_off needs to be <= max_back_off"
        );
        Ok(Self {
            drop_requests_until: Instant::now(),
            next_back_off: min_back_off,
            back_off_growth_factor,
            min_back_off,
            max_back_off,
        })
    }
}

impl RateLimitingStrategy {
    pub fn increase_back_off_with_cap(&mut self) {
        self.drop_requests_until = Instant::now() + self.next_back_off;
        tracing::warn!(
            "extended rate limiting for {}ms",
            self.next_back_off.as_millis()
        );
        let increased_back_off = self.next_back_off.mul_f64(self.back_off_growth_factor);
        self.next_back_off = std::cmp::min(increased_back_off, self.max_back_off);
    }

    pub fn reset_back_off(&mut self) {
        tracing::debug!("reset rate limit");
        self.next_back_off = self.min_back_off;
        self.drop_requests_until = Instant::now();
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    pub strategy: Mutex<RateLimitingStrategy>,
}

impl From<RateLimitingStrategy> for RateLimiter {
    fn from(strategy: RateLimitingStrategy) -> Self {
        Self {
            strategy: Mutex::new(strategy),
        }
    }
}

impl RateLimiter {
    /// If a request receives the response "Too many requests" (status code 429) future requests
    /// will get dropped for some time. Every successive 429 response increases that time exponentially.
    /// When a request eventually returns a normal result again future requests will no longer get
    /// dropped until the next 429 response occurs.
    pub async fn request(&self, request: RequestBuilder) -> Result<Response> {
        let next_back_off = {
            tracing::warn!("dropping request because API is currently rate limited");
            let strategy = self.strategy.lock().unwrap();
            if strategy.drop_requests_until > Instant::now() {
                return Err(anyhow::anyhow!("rate limited".to_string()));
            }
            strategy.next_back_off
        };

        let response = request.send().await?;

        let mut strategy = self.strategy.lock().unwrap();
        if response.status() == 429 {
            if strategy.next_back_off == next_back_off {
                // only increase back off if no other request increased it in the mean time
                strategy.increase_back_off_with_cap();
            }
            return Err(anyhow::anyhow!("rate limited".to_string()));
        } else {
            strategy.reset_back_off();
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Client;

    use super::*;

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
}
