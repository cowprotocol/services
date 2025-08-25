use {
    super::{CompetitionEstimator, compare_error},
    crate::price_estimation::{
        PriceEstimationError,
        native::{NativePriceEstimateResult, NativePriceEstimating, is_price_malformed},
    },
    anyhow::Context,
    futures::{FutureExt, future::BoxFuture},
    model::order::OrderKind,
    primitive_types::H160,
    std::{
        cmp::Ordering,
        sync::{
            Arc,
            atomic::{self, AtomicU32},
        },
        time::Duration,
    },
    tracing::instrument,
};

impl NativePriceEstimating for CompetitionEstimator<Arc<dyn NativePriceEstimating>> {
    #[instrument(skip_all)]
    fn estimate_native_price(
        &self,
        token: H160,
        total_timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        let started_at = std::time::Instant::now();

        async move {
            let remaining_stages = Arc::new(AtomicU32::new(self.stages.len() as u32));
            let results = self
                .produce_results(token, Result::is_ok, move |estimator, query| {
                    let remaining_stages = Arc::clone(&remaining_stages);
                    async move {
                        // Computes timeout for current stage dynamically based on the total time
                        // left and the remaining stages. That means if some stage finishes early,
                        // subsequent stages will get a bit more time to always use the entire
                        // total timeout.
                        let stage_timeout = {
                            let time_left = total_timeout.saturating_sub(started_at.elapsed());
                            let remaining_stages =
                                remaining_stages.fetch_sub(1, atomic::Ordering::Relaxed);
                            time_left.checked_div(remaining_stages).unwrap_or_default()
                        };

                        let res = estimator.estimate_native_price(query, stage_timeout).await;
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
}
