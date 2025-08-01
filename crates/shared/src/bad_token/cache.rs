use {
    super::{BadTokenDetecting, TokenQuality},
    anyhow::Result,
    dashmap::DashMap,
    futures::future::join_all,
    primitive_types::H160,
    std::{
        ops::Div,
        sync::Arc,
        time::{Duration, Instant},
    },
    tracing::instrument,
};

pub struct CachingDetector {
    inner: Box<dyn BadTokenDetecting>,
    cache: DashMap<H160, (Instant, TokenQuality)>,
    cache_expiry: Duration,
    prefetch_time: Duration,
}

#[async_trait::async_trait]
impl BadTokenDetecting for CachingDetector {
    #[instrument(skip_all)]
    async fn detect(&self, token: H160) -> Result<TokenQuality> {
        if let Some(quality) = self.get_from_cache(&token, Instant::now()) {
            return Ok(quality);
        }

        let result = self.inner.detect(token).await?;
        self.cache.insert(token, (Instant::now(), result.clone()));
        Ok(result)
    }
}

impl CachingDetector {
    pub fn new(
        inner: Box<dyn BadTokenDetecting>,
        cache_expiry: Duration,
        prefetch_time: Duration,
    ) -> Arc<Self> {
        assert!(
            cache_expiry > prefetch_time,
            "token quality cache prefetch time needs to be less than token quality cache expiry"
        );
        let detector = Arc::new(Self {
            inner,
            cache: Default::default(),
            cache_expiry,
            prefetch_time,
        });
        detector.clone().spawn_maintenance_task();
        detector
    }

    fn get_from_cache(&self, token: &H160, now: Instant) -> Option<TokenQuality> {
        let (instant, quality) = self.cache.get(token)?.value().clone();
        let still_valid = now.saturating_duration_since(instant) < self.cache_expiry;
        still_valid.then_some(quality)
    }

    fn insert_many_into_cache(&self, tokens: impl Iterator<Item = (H160, TokenQuality)>) {
        let now = Instant::now();
        tokens.into_iter().for_each(|(token, quality)| {
            self.cache.insert(token, (now, quality));
        });
    }

    fn spawn_maintenance_task(self: Arc<Self>) {
        // We need to prefetch the token quality the `prefetch_time` before the cache
        // expires
        let prefetch_time_to_expire = self.cache_expiry - self.prefetch_time;
        // The maintenance frequency has to be at least double of the prefetch time
        // frequency in order to guarantee that the prefetch time is executed
        // before the token quality expires. This is because of the
        // Nyquist–Shannon sampling theorem.
        let maintenance_timeout = self.prefetch_time.div(2);
        let detector = Arc::clone(&self);

        tokio::task::spawn(async move {
            loop {
                let start = Instant::now();

                let futures = detector.cache.iter().filter_map(|entry| {
                    let (token, (instant, _)) = entry.pair();
                    let (token, instant) = (*token, *instant);
                    if start.saturating_duration_since(instant) < prefetch_time_to_expire {
                        return None;
                    }
                    let detector = detector.clone();
                    Some(async move {
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
                    })
                });

                let results = join_all(futures).await;
                detector.insert_many_into_cache(results.into_iter().flatten());

                let remaining_sleep = maintenance_timeout.saturating_sub(start.elapsed());
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
            Duration::from_millis(200),
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
            Duration::from_millis(200),
        );
        let now = Instant::now();
        detector.cache.insert(token, (now, TokenQuality::Good));
        assert!(
            detector
                .get_from_cache(&token, now + Duration::from_secs(1))
                .is_some()
        );
        assert!(
            detector
                .get_from_cache(&token, now + Duration::from_secs(3))
                .is_none()
        );
    }

    #[tokio::test]
    async fn cache_prefetch_works() {
        let mut inner = MockBadTokenDetecting::new();
        // we expect it to be called twice: first time + prefetch time
        let mut seq = mockall::Sequence::new();
        // First call returns Ok(TokenQuality::Good)
        inner
            .expect_detect()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(TokenQuality::Good));
        // Second call returns Ok(TokenQuality::Bad)
        inner
            .expect_detect()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                Ok(TokenQuality::Bad {
                    reason: "bad token".to_string(),
                })
            });

        let detector = CachingDetector::new(
            Box::new(inner),
            Duration::from_millis(200),
            Duration::from_millis(50),
        );

        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(result.unwrap().is_good());
        // Check that the result is the same because we haven't reached the prefetch
        // time yet
        tokio::time::sleep(Duration::from_millis(100)).await;
        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(result.unwrap().is_good());
        // We wait so the prefetch fetches the data
        tokio::time::sleep(Duration::from_millis(70)).await;
        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(!result.unwrap().is_good());
    }
}
