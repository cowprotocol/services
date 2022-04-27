use anyhow::{anyhow, ensure, Result};
use reqwest::{RequestBuilder, Response};
use std::{
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
    /// Resets back off and stops rate limiting requests.
    pub fn response_ok(&mut self) {
        self.next_back_off = self.min_back_off;
        self.drop_requests_until = Instant::now();
    }

    /// Returns updated back off if no other thread increased it in the mean time.
    pub fn response_rate_limited(&mut self, previous_back_off: Duration) -> Option<Duration> {
        if self.next_back_off != previous_back_off {
            // Don't increase back off if somebody else already increased it in the meantime.
            return None;
        }

        self.drop_requests_until = Instant::now() + self.next_back_off;
        let increased_back_off = self.next_back_off.mul_f64(self.back_off_growth_factor);
        self.next_back_off = std::cmp::min(increased_back_off, self.max_back_off);
        Some(self.next_back_off)
    }

    pub fn get_next_back_off_if_not_rate_limited(&self, now: Instant) -> Option<Duration> {
        if self.drop_requests_until > now {
            return None;
        }

        Some(self.next_back_off)
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    pub strategy: Mutex<RateLimitingStrategy>,
}

impl RateLimiter {
    fn strategy(&self) -> MutexGuard<RateLimitingStrategy> {
        self.strategy.lock().unwrap()
    }
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
        let now = Instant::now();
        let next_back_off = match self.strategy().get_next_back_off_if_not_rate_limited(now) {
            None => {
                tracing::warn!("dropping request because API is currently rate limited");
                anyhow::bail!("rate limited");
            }
            Some(next_back_off) => next_back_off,
        };

        let response = request.send().await?;

        if response.status() == 429 {
            if let Some(new_back_off) = self.strategy().response_rate_limited(next_back_off) {
                tracing::warn!("extended rate limiting for {}ms", new_back_off.as_millis());
            }
            anyhow::bail!("rate limited");
        } else {
            self.strategy().response_ok();
            tracing::debug!("reset rate limit");
            Ok(response)
        }
    }
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
        let rate_limiter = RateLimiter::from(strategy);
        // note that 1_000 requests will not always trigger a rate limit
        let mut stream = stream::iter(0..1_000).map(|_| async {}).buffer_unordered(2);
        while stream.next().await.is_some() {
            let request = client.get(url);
            let response = rate_limiter.request(request).await;
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
}
