//! A native price estimator wrapper that automatically switches to a fallback
//! estimator when the primary becomes unavailable.
//!
//! # State Machine
//!
//! The estimator operates as a two-state machine:
//!
//! ```text
//!                  3 consecutive
//!                ProtocolInternal errors
//!   ┌─────────┐ ───────────────────────> ┌──────────┐
//!   │ Primary │                          │ Fallback │
//!   └─────────┘ <─────────────────────── └──────────┘
//!                  probe succeeds
//!               (every PROBE_INTERVAL)
//! ```
//!
//! **Primary state**: All requests go to the primary estimator. A counter
//! tracks consecutive [`PriceEstimationError::ProtocolInternal`] errors. Any
//! success resets the counter. Once the counter reaches
//! [`CONSECUTIVE_ERRORS_THRESHOLD`], the estimator switches to fallback and
//! the current request is retried against the fallback.
//!
//! **Fallback state**: All requests go to the fallback estimator. Every
//! [`PROBE_INTERVAL`], one request probes both the primary and fallback
//! concurrently. If the primary probe succeeds, the estimator switches back to
//! primary; otherwise it stays in fallback and resets the probe timer.
//!
//! Only `ProtocolInternal` errors (e.g. connection refused, timeouts) trigger
//! the switch. Domain errors like `NoLiquidity` do not affect the state.

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

/// How often the estimator probes the primary while in fallback state.
const PROBE_INTERVAL: Duration = Duration::from_secs(60);

/// Number of consecutive `ProtocolInternal` errors from the primary before
/// switching to fallback.
const CONSECUTIVE_ERRORS_THRESHOLD: u32 = 3;

enum State {
    Primary {
        /// Counts consecutive protocol internal errors from the primary
        /// estimator.
        consecutive_errors: u32,
    },
    Fallback {
        /// Tracks when we last tried the primary.
        last_probe: Instant,
    },
}

/// What the estimator should do for the current request based on the current
/// state and probe timing.
enum Action {
    /// Use the primary estimator.
    Primary,
    /// Use the fallback estimator directly (within probe interval).
    Fallback,
    /// Probe both primary and fallback concurrently (probe interval elapsed).
    Probe,
}

/// Wraps a primary and fallback [`NativePriceEstimating`] implementation,
/// automatically switching to the fallback when the primary experiences
/// repeated `ProtocolInternal` failures and periodically probing to recover.
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
            state: Mutex::new(State::Primary {
                consecutive_errors: 0,
            }),
        }
    }
}

impl FallbackNativePriceEstimator {
    /// Returns `true` if the fallback should be used.
    fn should_use_fallback(&self, result: &NativePriceEstimateResult) -> bool {
        let mut state = self.state.lock().unwrap();
        if let Err(PriceEstimationError::ProtocolInternal(err)) = result {
            let State::Primary {
                consecutive_errors, ..
            } = &mut *state
            else {
                return false;
            };
            *consecutive_errors += 1;
            if *consecutive_errors >= CONSECUTIVE_ERRORS_THRESHOLD {
                tracing::info!(
                    ?err,
                    "primary native price estimator down after {} consecutive errors, switching \
                     to fallback",
                    *consecutive_errors
                );
                *state = State::Fallback {
                    last_probe: Instant::now(),
                };
                return true;
            }
            tracing::debug!(
                ?err,
                consecutive_errors = *consecutive_errors,
                "primary native price estimator error, not yet switching to fallback"
            );
            false
        } else {
            if let State::Primary {
                consecutive_errors, ..
            } = &mut *state
            {
                *consecutive_errors = 0;
            }
            false
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
            let action = {
                let state = self.state.lock().unwrap();
                match &*state {
                    State::Primary { .. } => Action::Primary,
                    State::Fallback { last_probe } if last_probe.elapsed() >= PROBE_INTERVAL => {
                        Action::Probe
                    }
                    State::Fallback { .. } => Action::Fallback,
                }
            };

            match action {
                Action::Primary => {
                    let result = self.primary.estimate_native_price(token, timeout).await;
                    if self.should_use_fallback(&result) {
                        self.fallback.estimate_native_price(token, timeout).await
                    } else {
                        result
                    }
                }
                Action::Probe => {
                    let (primary_result, fallback_result) = futures::join!(
                        self.primary.estimate_native_price(token, timeout),
                        self.fallback.estimate_native_price(token, timeout),
                    );

                    if matches!(
                        &primary_result,
                        Err(PriceEstimationError::ProtocolInternal(_))
                    ) {
                        let mut state = self.state.lock().unwrap();
                        *state = State::Fallback {
                            last_probe: Instant::now(),
                        };
                        tracing::debug!("primary still down after probe, continuing with fallback");
                        fallback_result
                    } else {
                        tracing::info!("primary native price estimator recovered");
                        let mut state = self.state.lock().unwrap();
                        *state = State::Primary {
                            consecutive_errors: 0,
                        };
                        primary_result
                    }
                }
                Action::Fallback => self.fallback.estimate_native_price(token, timeout).await,
            }
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
            .times(3)
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

        // First two errors: stay in primary, return the error
        for _ in 0..2 {
            let result = estimator.estimate_native_price(token(), timeout()).await;
            assert!(matches!(
                result,
                Err(PriceEstimationError::ProtocolInternal(_))
            ));
        }

        // Third error: threshold reached, switch to fallback
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 2.0);
    }

    #[tokio::test]
    async fn stays_in_fallback_without_probing_before_interval() {
        let mut primary = MockNativePriceEstimating::new();
        // Called 3 times (threshold) for the initial failures, NOT called again before
        // probe interval
        primary
            .expect_estimate_native_price()
            .times(3)
            .returning(|_, _| {
                async {
                    Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                        "connection refused"
                    )))
                }
                .boxed()
            });

        let mut fallback = MockNativePriceEstimating::new();
        // Called once when threshold is reached, once for the subsequent request
        fallback
            .expect_estimate_native_price()
            .times(2)
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // First two calls: primary errors returned (below threshold)
        for _ in 0..2 {
            let _ = estimator.estimate_native_price(token(), timeout()).await;
        }

        // Third call: threshold reached, triggers fallback
        let _ = estimator.estimate_native_price(token(), timeout()).await;

        // Fourth call should use fallback directly (within probe interval)
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 2.0);
    }

    #[tokio::test]
    async fn probes_primary_after_interval_and_recovers() {
        let mut primary = MockNativePriceEstimating::new();
        let mut call_count = 0u32;
        primary
            .expect_estimate_native_price()
            .times(4) // 3 for threshold + 1 probe
            .returning(move |_, _| {
                call_count += 1;
                if call_count <= 3 {
                    // First 3 calls: primary is down (reaching threshold)
                    async {
                        Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                            "connection refused"
                        )))
                    }
                    .boxed()
                } else {
                    // Fourth call (probe): primary recovered
                    async { Ok(1.0) }.boxed()
                }
            });

        let mut fallback = MockNativePriceEstimating::new();
        // Called once when threshold is reached + once during probe (concurrent)
        fallback
            .expect_estimate_native_price()
            .times(2)
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // First two calls: primary errors returned (below threshold)
        for _ in 0..2 {
            let _ = estimator.estimate_native_price(token(), timeout()).await;
        }

        // Third call: threshold reached, triggers fallback
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

        // First two calls: primary errors (below threshold)
        for _ in 0..2 {
            let _ = estimator.estimate_native_price(token(), timeout()).await;
        }

        // Third call: threshold reached, triggers fallback
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
    async fn does_not_switch_on_fewer_than_threshold_errors() {
        let mut primary = MockNativePriceEstimating::new();
        let mut call_count = 0u32;
        primary
            .expect_estimate_native_price()
            .times(3)
            .returning(move |_, _| {
                call_count += 1;
                if call_count <= 2 {
                    async {
                        Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                            "transient error"
                        )))
                    }
                    .boxed()
                } else {
                    // Third call: primary recovers before threshold
                    async { Ok(1.0) }.boxed()
                }
            });

        let mut fallback = MockNativePriceEstimating::new();
        fallback.expect_estimate_native_price().never();

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // Two errors: below threshold, stay in primary
        for _ in 0..2 {
            let result = estimator.estimate_native_price(token(), timeout()).await;
            assert!(matches!(
                result,
                Err(PriceEstimationError::ProtocolInternal(_))
            ));
        }

        // Third call: primary succeeds, fallback never used
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 1.0);
    }

    #[tokio::test]
    async fn resets_counter_on_success() {
        let mut primary = MockNativePriceEstimating::new();
        let mut call_count = 0u32;
        primary
            .expect_estimate_native_price()
            .times(4)
            .returning(move |_, _| {
                call_count += 1;
                match call_count {
                    // error, success, error, error — never reaches 3 consecutive
                    1 | 3 | 4 => async {
                        Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                            "transient error"
                        )))
                    }
                    .boxed(),
                    2 => async { Ok(1.0) }.boxed(),
                    _ => unreachable!(),
                }
            });

        let mut fallback = MockNativePriceEstimating::new();
        fallback.expect_estimate_native_price().never();

        let estimator = FallbackNativePriceEstimator::new(Box::new(primary), Box::new(fallback));

        // Call 1: error (consecutive_errors = 1)
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert!(matches!(
            result,
            Err(PriceEstimationError::ProtocolInternal(_))
        ));

        // Call 2: success (consecutive_errors reset to 0)
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert_eq!(result.unwrap(), 1.0);

        // Call 3: error (consecutive_errors = 1)
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert!(matches!(
            result,
            Err(PriceEstimationError::ProtocolInternal(_))
        ));

        // Call 4: error (consecutive_errors = 2, still below threshold)
        let result = estimator.estimate_native_price(token(), timeout()).await;
        assert!(matches!(
            result,
            Err(PriceEstimationError::ProtocolInternal(_))
        ));
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
