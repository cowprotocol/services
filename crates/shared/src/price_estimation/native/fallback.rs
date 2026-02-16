use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    alloy::primitives::Address,
    futures::{FutureExt, future::BoxFuture},
    std::{
        sync::Mutex,
        time::{Duration, Instant},
    },
};

const PROBE_INTERVAL: Duration = Duration::from_secs(60);

enum State {
    Primary,
    /// Using fallback; `last_probe` tracks when we last tried the primary.
    Fallback {
        last_probe: Instant,
    },
}

pub struct FallbackNativePriceEstimator {
    primary: Box<dyn NativePriceEstimating>,
    fallback: Box<dyn NativePriceEstimating>,
    state: Mutex<State>,
}

impl FallbackNativePriceEstimator {
    pub fn new(
        primary: Box<dyn NativePriceEstimating>,
        fallback: Box<dyn NativePriceEstimating>,
    ) -> Self {
        Self {
            primary,
            fallback,
            state: Mutex::new(State::Primary),
        }
    }
}

impl NativePriceEstimating for FallbackNativePriceEstimator {
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let (in_fallback, should_probe) = {
                let state = self.state.lock().unwrap();
                match &*state {
                    State::Primary => (false, false),
                    State::Fallback { last_probe } => {
                        (true, last_probe.elapsed() >= PROBE_INTERVAL)
                    }
                }
            };

            if in_fallback && should_probe {
                // Probe primary alongside fallback
                let (primary_result, fallback_result) = futures::join!(
                    self.primary.estimate_native_price(token, timeout),
                    self.fallback.estimate_native_price(token, timeout),
                );

                if matches!(
                    &primary_result,
                    Err(PriceEstimationError::ProtocolInternal(_))
                ) {
                    // Primary still down, update probe timestamp
                    let mut state = self.state.lock().unwrap();
                    *state = State::Fallback {
                        last_probe: Instant::now(),
                    };
                    tracing::debug!("primary still down after probe, continuing with fallback");
                    return fallback_result;
                }

                // Primary recovered
                tracing::info!("primary native price estimator recovered");
                let mut state = self.state.lock().unwrap();
                *state = State::Primary;
                return primary_result;
            }

            if in_fallback {
                return self.fallback.estimate_native_price(token, timeout).await;
            }

            // Primary mode
            let result = self.primary.estimate_native_price(token, timeout).await;
            if let Err(PriceEstimationError::ProtocolInternal(ref err)) = result {
                tracing::warn!(
                    ?err,
                    "primary native price estimator down, switching to fallback"
                );
                {
                    let mut state = self.state.lock().unwrap();
                    *state = State::Fallback {
                        last_probe: Instant::now(),
                    };
                }
                return self.fallback.estimate_native_price(token, timeout).await;
            }

            result
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::native::MockNativePriceEstimating,
        futures::FutureExt,
    };

    fn token() -> Address {
        Address::with_last_byte(1)
    }

    fn timeout() -> Duration {
        Duration::from_secs(5)
    }

    #[tokio::test]
    async fn uses_primary_when_healthy() {
        let mut primary = MockNativePriceEstimating::new();
        primary
            .expect_estimate_native_price()
            .returning(|_, _| async { Ok(1.0) }.boxed());

        let mut fallback = MockNativePriceEstimating::new();
        fallback.expect_estimate_native_price().never();

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 1.0);
    }

    #[tokio::test]
    async fn switches_to_fallback_on_protocol_internal() {
        let mut primary = MockNativePriceEstimating::new();
        primary
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| {
                async {
                    Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                        "connection refused"
                    )))
                }
                .boxed()
            });

        let mut fallback = MockNativePriceEstimating::new();
        fallback
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 2.0);
    }

    #[tokio::test]
    async fn stays_in_fallback_without_probing_before_interval() {
        let mut primary = MockNativePriceEstimating::new();
        // Called once for the initial failure, NOT called again before probe interval
        primary
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| {
                async {
                    Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                        "connection refused"
                    )))
                }
                .boxed()
            });

        let mut fallback = MockNativePriceEstimating::new();
        // Called once for the initial fallback, once for the subsequent request
        fallback
            .expect_estimate_native_price()
            .times(2)
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // First call triggers fallback
        let _ = estimator.estimate_native_price(token(), timeout()).await;

        // Second call should use fallback directly (within probe interval)
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 2.0);
    }

    #[tokio::test]
    async fn probes_primary_after_interval_and_recovers() {
        let mut primary = MockNativePriceEstimating::new();
        let mut call_count = 0u32;
        primary
            .expect_estimate_native_price()
            .times(2)
            .returning(move |_, _| {
                call_count += 1;
                if call_count == 1 {
                    // First call: primary is down
                    async {
                        Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                            "connection refused"
                        )))
                    }
                    .boxed()
                } else {
                    // Second call (probe): primary recovered
                    async { Ok(1.0) }.boxed()
                }
            });

        let mut fallback = MockNativePriceEstimating::new();
        // Called for initial fallback + during probe (concurrent with primary)
        fallback
            .expect_estimate_native_price()
            .times(2)
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // First call triggers fallback
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 2.0);

        // Force probe interval to expire
        {
            let mut state = estimator.state.lock().unwrap();
            *state = State::Fallback {
                last_probe: Instant::now() - PROBE_INTERVAL - Duration::from_secs(1),
            };
        }

        // This call should probe primary (which recovers) and return primary result
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 1.0);
    }

    #[tokio::test]
    async fn probes_primary_after_interval_stays_in_fallback_if_still_down() {
        let mut primary = MockNativePriceEstimating::new();
        primary.expect_estimate_native_price().returning(|_, _| {
            async {
                Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                    "connection refused"
                )))
            }
            .boxed()
        });

        let mut fallback = MockNativePriceEstimating::new();
        fallback
            .expect_estimate_native_price()
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // First call triggers fallback
        let _ = estimator.estimate_native_price(token(), timeout()).await;

        // Force probe interval to expire
        {
            let mut state = estimator.state.lock().unwrap();
            *state = State::Fallback {
                last_probe: Instant::now() - PROBE_INTERVAL - Duration::from_secs(1),
            };
        }

        // Probe fires, primary still down → use fallback result
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 2.0);
    }

    #[tokio::test]
    async fn does_not_switch_on_non_protocol_errors() {
        let mut primary = MockNativePriceEstimating::new();
        primary
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Err(PriceEstimationError::NoLiquidity) }.boxed());

        let mut fallback = MockNativePriceEstimating::new();
        fallback.expect_estimate_native_price().never();

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert!(matches!(result, Err(PriceEstimationError::NoLiquidity)));
    }
}
