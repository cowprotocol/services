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
    std::{cmp::Ordering, sync::Arc, time::Duration},
    tracing::instrument,
};

impl NativePriceEstimating for CompetitionEstimator<Arc<dyn NativePriceEstimating>> {
    #[instrument(skip_all)]
    fn estimate_native_price(
        &self,
        token: H160,
        timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        let timeout_per_stage = timeout / self.stages.len() as u32;
        async move {
            let results = self
                .produce_results(token, Result::is_ok, move |e, q| {
                    async move {
                        let res = e.estimate_native_price(q, timeout_per_stage).await;
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
    };

    fn native_price(native_price: f64) -> Result<f64, PriceEstimationError> {
        Ok(native_price)
    }

    fn error<T>(err: PriceEstimationError) -> Result<T, PriceEstimationError> {
        Err(err)
    }

    /// Returns the best native estimate with respect to the provided ranking
    /// and order kind.
    async fn best_response(
        ranking: PriceRanking,
        estimates: Vec<Result<f64, PriceEstimationError>>,
    ) -> Result<f64, PriceEstimationError> {
        fn estimator(
            estimate: Result<f64, PriceEstimationError>,
        ) -> Arc<dyn NativePriceEstimating> {
            let mut estimator = MockNativePriceEstimating::new();
            estimator
                .expect_estimate_native_price()
                .times(1)
                .return_once(move |_, _| async move { estimate }.boxed());
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
            vec![native_price(1.), native_price(2.)],
        )
        .await;
        assert_eq!(best, native_price(2.));
    }

    /// If all estimators returned an error we return the one with the highest
    /// priority.
    #[tokio::test]
    async fn returns_highest_priority_error_native() {
        // Returns errors with highest priority.
        let best = best_response(
            PriceRanking::MaxOutAmount,
            vec![
                error(PriceEstimationError::RateLimited),
                error(PriceEstimationError::ProtocolInternal(anyhow::anyhow!("!"))),
            ],
        )
        .await;
        assert_eq!(best, error(PriceEstimationError::RateLimited));
    }

    /// Any native price estimate, no matter how bad, is preferred over an
    /// error.
    #[tokio::test]
    async fn prefer_estimate_over_error_native() {
        let best = best_response(
            PriceRanking::MaxOutAmount,
            vec![native_price(1.), error(PriceEstimationError::RateLimited)],
        )
        .await;
        assert_eq!(best, native_price(1.));
    }

    /// Nonsensical prices like infinities, and non-positive values get ignored.
    #[tokio::test]
    async fn ignore_nonsensical_prices() {
        let subnormal = f64::from_bits(1);
        assert!(!subnormal.is_normal());

        for price in [f64::NEG_INFINITY, -1., 0., f64::INFINITY, subnormal] {
            let best = best_response(PriceRanking::MaxOutAmount, vec![native_price(price)]).await;
            assert!(best.is_err());
        }
    }
}
