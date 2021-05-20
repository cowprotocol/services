use super::{BadTokenDetecting, TokenQuality};
use anyhow::Result;
use primitive_types::H160;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct CachingDetector {
    inner: Box<dyn BadTokenDetecting>,
    // std mutex is fine because we don't hold lock across await.
    cache: Mutex<HashMap<H160, (Instant, TokenQuality)>>,
    cache_expiry: Duration,
}

#[async_trait::async_trait]
impl BadTokenDetecting for CachingDetector {
    async fn detect(&self, token: H160) -> Result<TokenQuality> {
        if let Some(quality) = self.get_from_cache(&token, Instant::now()) {
            return Ok(quality);
        }

        let result = self.inner.detect(token).await?;
        self.insert_into_cache(token, result.clone());
        Ok(result)
    }
}

impl CachingDetector {
    pub fn new(inner: Box<dyn BadTokenDetecting>, cache_expiry: Duration) -> Self {
        Self {
            inner,
            cache: Default::default(),
            cache_expiry,
        }
    }

    fn get_from_cache(&self, token: &H160, now: Instant) -> Option<TokenQuality> {
        match self.cache.lock().unwrap().get(token) {
            Some((instant, quality))
                if now.checked_duration_since(*instant).unwrap_or_default() < self.cache_expiry =>
            {
                Some(quality.clone())
            }
            _ => None,
        }
    }

    fn insert_into_cache(&self, token: H160, quality: TokenQuality) {
        self.cache
            .lock()
            .unwrap()
            .insert(token, (Instant::now(), quality));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::MockBadTokenDetecting;
    use futures::FutureExt;

    #[test]
    fn goes_to_cache() {
        // Would panic if called twice.
        let mut inner = MockBadTokenDetecting::new();
        inner
            .expect_detect()
            .times(1)
            .returning(|_| Ok(TokenQuality::Good));

        let detector = CachingDetector::new(Box::new(inner), Duration::from_secs(1));

        for _ in 0..2 {
            let result = detector
                .detect(H160::from_low_u64_le(0))
                .now_or_never()
                .unwrap();
            assert!(result.unwrap().is_good());
        }
    }

    #[test]
    fn cache_expires() {
        let inner = MockBadTokenDetecting::new();
        let token = H160::from_low_u64_le(0);
        let detector = CachingDetector::new(Box::new(inner), Duration::from_secs(2));
        let now = Instant::now();
        detector
            .cache
            .lock()
            .unwrap()
            .insert(token, (now, TokenQuality::Good));
        assert!(detector
            .get_from_cache(&token, now + Duration::from_secs(1))
            .is_some());
        assert!(detector
            .get_from_cache(&token, now + Duration::from_secs(3))
            .is_none());
    }
}
