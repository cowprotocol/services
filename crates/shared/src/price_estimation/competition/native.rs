use {
    super::{CompetitionEstimator, compare_error},
    crate::price_estimation::{
        PriceEstimationError,
        native::{NativePriceEstimateResult, NativePriceEstimating, is_price_malformed},
    },
    alloy::primitives::Address,
    anyhow::Context,
    futures::{FutureExt, future::BoxFuture},
    model::order::OrderKind,
    std::{cmp::Ordering, sync::Arc, time::Duration},
    tracing::instrument,
};

impl NativePriceEstimating for CompetitionEstimator<Arc<dyn NativePriceEstimating>> {
    #[instrument(skip_all)]
    fn estimate_native_price(
        &self,
        token: Address,
        total_timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        let started_at = std::time::Instant::now();

        async move {
            let results = self
                .produce_results(token, Result::is_ok, move |context| {
                    async move {
                        // Computes timeout for current stage dynamically based on the total time
                        // left and the remaining stages. That means if some stage finishes early,
                        // subsequent stages will get a bit more time to always use the entire
                        // total timeout.
                        let stage_timeout = {
                            let time_left = total_timeout.saturating_sub(started_at.elapsed());
                            time_left
                                // +1 as remaining_stages does not include our current stage
                                .checked_div(context.remaining_stages() as u32 + 1)
                                .unwrap_or_default()
                        };

                        let res = context
                            .estimator
                            .estimate_native_price(context.query, stage_timeout)
                            .await;
                        if res.as_ref().is_ok_and(|price| is_price_malformed(*price)) {
                            let err = anyhow::anyhow!("estimator returned malformed price");
                            return Err(PriceEstimationError::EstimatorInternal(err));
                        }
                        res
                    }
                    .boxed()
                })
                .await;
            let winner = results
                .into_iter()
                .max_by(|a, b| compare_native_result(&a.1, &b.1))
                .context("could not get any native price")?;
            self.report_winner(&token, OrderKind::Buy, winner)
        }
        .boxed()
    }
}

fn compare_native_result(
    a: &Result<f64, PriceEstimationError>,
    b: &Result<f64, PriceEstimationError>,
) -> Ordering {
    match (a, b) {
        (Ok(a), Ok(b)) => a.total_cmp(b),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(a), Err(b)) => compare_error(a, b),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{
            HEALTHY_PRICE_ESTIMATION_TIME,
            competition::PriceRanking,
            native::MockNativePriceEstimating,
        },
        std::pin::Pin,
    };

    type NativePriceFuture =
        Pin<Box<dyn Future<Output = Result<f64, PriceEstimationError>> + Send>>;

    /// Returns the best native estimate with respect to the provided ranking
    /// and order kind.
    async fn best_response(
        ranking: PriceRanking,
        estimates: Vec<NativePriceFuture>,
    ) -> Result<f64, PriceEstimationError> {
        fn estimator(estimate: NativePriceFuture) -> Arc<dyn NativePriceEstimating> {
            let mut estimator = MockNativePriceEstimating::new();
            estimator
                .expect_estimate_native_price()
                .times(1)
                .return_once(move |_, _| estimate);
            Arc::new(estimator)
        }

        let priority: CompetitionEstimator<Arc<dyn NativePriceEstimating>> =
            CompetitionEstimator::new(
                vec![
                    estimates
                        .into_iter()
                        .enumerate()
                        .map(|(i, e)| (format!("estimator_{i}"), estimator(e)))
                        .collect(),
                ],
                ranking.clone(),
            );

        priority
            .estimate_native_price(Default::default(), HEALTHY_PRICE_ESTIMATION_TIME)
            .await
    }

    /// If all estimators returned an error we return the one with the highest
    /// priority.
    #[tokio::test]
    async fn returns_highest_native_price() {
        // Returns errors with highest priority.
        let best = best_response(
            PriceRanking::MaxOutAmount,
            vec![async { Ok(1.) }.boxed(), async { Ok(2.) }.boxed()],
        )
        .await;
        assert_eq!(best, Ok(2.));
    }

    /// If all estimators returned an error we return the one with the highest
    /// priority.
    #[tokio::test]
    async fn returns_highest_priority_error_native() {
        // Returns errors with highest priority.
        let best = best_response(
            PriceRanking::MaxOutAmount,
            vec![
                async { Err(PriceEstimationError::RateLimited) }.boxed(),
                async { Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!("!"))) }.boxed(),
            ],
        )
        .await;
        assert_eq!(best, Err(PriceEstimationError::RateLimited));
    }

    /// Any native price estimate, no matter how bad, is preferred over an
    /// error.
    #[tokio::test]
    async fn prefer_estimate_over_error_native() {
        let best = best_response(
            PriceRanking::MaxOutAmount,
            vec![
                async { Ok(1.) }.boxed(),
                async { Err(PriceEstimationError::RateLimited) }.boxed(),
            ],
        )
        .await;
        assert_eq!(best, Ok(1.));
    }

    /// Nonsensical prices like infinities, and non-positive values get ignored.
    #[tokio::test]
    async fn ignore_nonsensical_prices() {
        let subnormal = f64::from_bits(1);
        assert!(!subnormal.is_normal());

        for price in [f64::NEG_INFINITY, -1., 0., f64::INFINITY, subnormal] {
            let best = best_response(
                PriceRanking::MaxOutAmount,
                vec![async move { Ok(price) }.boxed()],
            )
            .await;
            assert!(best.is_err());
        }
    }

    /// If early stages don't use some of their allocated time later stages
    /// can use it instead.
    #[tokio::test]
    async fn later_stages_may_use_excess_time() {
        fn estimator(
            estimate: NativePriceFuture,
            max_timeout: Duration,
        ) -> Arc<dyn NativePriceEstimating> {
            let mut estimator = MockNativePriceEstimating::new();
            estimator
                .expect_estimate_native_price()
                .times(1)
                .withf(move |_query, actual_timeout| {
                    // allow for small difference due to tokio scheduling
                    const BUFFER: Duration = Duration::from_millis(10);
                    ((max_timeout - BUFFER)..=max_timeout).contains(actual_timeout)
                })
                .return_once(move |_, _| estimate);
            Arc::new(estimator)
        }

        const TOTAL_TIMEOUT: Duration = Duration::from_millis(500);
        const FIRST_STAGE: Duration = Duration::from_millis(100);

        let priority: CompetitionEstimator<Arc<dyn NativePriceEstimating>> =
            CompetitionEstimator::new(
                vec![
                    vec![
                        (
                            "1.1".into(),
                            estimator(
                                async {
                                    // The first stage takes a bit of time but is still
                                    // way faster than it needs to be.
                                    tokio::time::sleep(FIRST_STAGE).await;
                                    // return an error to require the second estimator to run
                                    Err(PriceEstimationError::NoLiquidity)
                                }
                                .boxed(),
                                // first stage gets half the total time
                                TOTAL_TIMEOUT / 2,
                            )
                        ),
                        // We add a second estimator in the first stage to catch
                        // the error when you compute the remaining time based on the
                        // number of estimator invocations instead of stage invocations.
                        (
                            "1.2".into(),
                            estimator(
                                // this task may return immediately because we need to process
                                // the whole stage before we start the next one
                                async { Err(PriceEstimationError::NoLiquidity) }.boxed(),
                                TOTAL_TIMEOUT / 2
                            ),
                        ),
                    ],
                    vec![(
                        "2.1".into(),
                        estimator(
                            async { Ok(1.) }.boxed(),
                            // Because the first stage was faster than the allocated
                            // time, the second stage now gets a bit more time.
                            TOTAL_TIMEOUT - FIRST_STAGE,
                        ),
                    )],
                ],
                PriceRanking::MaxOutAmount,
            )
            // allow bailing out after 1 successful result to prevent both
            // stages to be kicked-off at the same time (if we have too few stages
            // configured the CompetitionEstimator will trigger them all concurrently)
            .with_early_return(1.try_into().unwrap());

        let res = priority
            .estimate_native_price(Default::default(), TOTAL_TIMEOUT)
            .await;
        assert_eq!(res, Ok(1.));
    }
}
