use {
    super::{compare_error, CompetitionEstimator},
    crate::price_estimation::{
        native::{NativePriceEstimateResult, NativePriceEstimating},
        PriceEstimationError,
    },
    anyhow::Context,
    futures::{future::BoxFuture, FutureExt},
    model::order::OrderKind,
    primitive_types::H160,
    std::{cmp::Ordering, sync::Arc},
};

impl NativePriceEstimating for CompetitionEstimator<Arc<dyn NativePriceEstimating>> {
    fn estimate_native_price(&self, token: H160) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let results = self
                .produce_results(token, is_result_usable, |e, q| e.estimate_native_price(q))
                .await;
            let winner = results
                .into_iter()
                .filter(|(_index, result)| is_result_usable(result))
                .max_by(|a, b| compare_native_result(&a.1, &b.1))
                .context("could not get any native price")?;
            self.report_winner(&token, OrderKind::Buy, winner)
        }
        .boxed()
    }
}

fn is_result_usable(result: &NativePriceEstimateResult) -> bool {
    result
        .as_ref()
        .is_ok_and(|price| price.is_normal() && *price > 0.)
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
        crate::price_estimation::{competition::PriceRanking, native::MockNativePriceEstimating},
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
                .return_once(move |_| async move { estimate }.boxed());
            Arc::new(estimator)
        }

        let priority: CompetitionEstimator<Arc<dyn NativePriceEstimating>> =
            CompetitionEstimator::new(
                vec![estimates
                    .into_iter()
                    .enumerate()
                    .map(|(i, e)| (format!("estimator_{i}"), estimator(e)))
                    .collect()],
                ranking.clone(),
            );

        priority.estimate_native_price(Default::default()).await
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
