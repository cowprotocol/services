use {
    super::{BadTokenDetecting, TokenQuality},
    anyhow::Result,
    futures::future::join_all,
    primitive_types::H160,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
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
    pub fn new(
        inner: Box<dyn BadTokenDetecting>,
        cache_expiry: Duration,
        maintenance_task_timeout: Duration,
    ) -> Arc<Self> {
        let detector = Arc::new(Self {
            inner,
            cache: Default::default(),
            cache_expiry,
        });
        detector
            .clone()
            .spawn_maintenance_task(maintenance_task_timeout);
        detector
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

    fn insert_many_into_cache(&self, tokens: &[(H160, TokenQuality)]) {
        let mut lock = self.cache.lock().unwrap();
        for (token, quality) in tokens {
            lock.insert(*token, (Instant::now(), quality.clone()));
        }
    }

    fn spawn_maintenance_task(self: Arc<Self>, maintenance_timeout: Duration) {
        let cache_expiry = self.cache_expiry;
        let detector = Arc::clone(&self);

        tokio::task::spawn(async move {
            loop {
                let start = Instant::now();

                let expired_tokens: Vec<H160> = {
                    let cache = detector.cache.lock().unwrap();
                    let now = Instant::now();
                    cache
                        .iter()
                        .filter(|(_, (instant, _))| {
                            now.checked_duration_since(*instant).unwrap_or_default() >= cache_expiry
                        })
                        .map(|(token, _)| *token)
                        .collect()
                };

                let results = join_all(expired_tokens.into_iter().map(|token| {
                    let detector = detector.clone();
                    async move {
                        match detector.inner.detect(token).await {
                            Ok(result) => Some((token, result)),
                            Err(err) => {
                                tracing::warn!(
                                    ?token,
                                    ?err,
                                    "unable to determine token quality in the background task"
                                );
                                None
                            }
                        }
                    }
                }))
                .await
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

                detector.insert_many_into_cache(&results);

                let remaining_sleep = maintenance_timeout
                    .checked_sub(start.elapsed())
                    .unwrap_or_default();
                tokio::time::sleep(remaining_sleep).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::bad_token::MockBadTokenDetecting, futures::FutureExt};

    #[tokio::test]
    async fn goes_to_cache() {
        // Would panic if called twice.
        let mut inner = MockBadTokenDetecting::new();
        inner
            .expect_detect()
            .times(1)
            .returning(|_| Ok(TokenQuality::Good));

        let detector = CachingDetector::new(
            Box::new(inner),
            Duration::from_secs(1),
            Duration::from_secs(10),
        );

        for _ in 0..2 {
            let result = detector
                .detect(H160::from_low_u64_le(0))
                .now_or_never()
                .unwrap();
            assert!(result.unwrap().is_good());
        }
    }

    #[tokio::test]
    async fn cache_expires() {
        let inner = MockBadTokenDetecting::new();
        let token = H160::from_low_u64_le(0);
        let detector = CachingDetector::new(
            Box::new(inner),
            Duration::from_secs(2),
            Duration::from_secs(10),
        );
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
