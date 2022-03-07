use crate::price_estimation::{
    old_estimator_to_stream, vec_estimates, PriceEstimateResult, PriceEstimating,
    PriceEstimationError, Query,
};

/// Tries inner price estimators in order for queries failing with PriceEstimationError::Other.
/// Successes, UnsupportedToken or NoLiquidity errors are not retried.
pub struct PriorityPriceEstimator {
    inner: Vec<Box<dyn PriceEstimating>>,
}

impl PriorityPriceEstimator {
    pub fn new(inner: Vec<Box<dyn PriceEstimating>>) -> Self {
        assert!(!inner.is_empty());
        Self { inner }
    }
    async fn estimates_(&self, queries: &[Query]) -> Vec<PriceEstimateResult> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        let mut results = vec_estimates(self.inner[0].as_ref(), queries).await;
        log_errors(&results, 0);
        for (i, inner) in (&self.inner[1..]).iter().enumerate() {
            let retry_indexes = results
                .iter()
                .enumerate()
                .filter(|(_, result)| matches!(result, Err(PriceEstimationError::Other(_))))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if retry_indexes.is_empty() {
                break;
            }
            let retry_queries = retry_indexes
                .iter()
                .map(|index| queries[*index])
                .collect::<Vec<_>>();
            let retry_results = vec_estimates(inner.as_ref(), &retry_queries).await;
            log_errors(&retry_results, i + 1);
            for (index, result) in retry_indexes.into_iter().zip(retry_results) {
                results[index] = result;
            }
        }
        results
    }
}

fn log_errors(results: &[PriceEstimateResult], estimator_index: usize) {
    for result in results {
        if let Err(err) = result {
            tracing::warn!(%estimator_index, ?err, "priority price estimator failed");
        }
    }
}

impl PriceEstimating for PriorityPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        old_estimator_to_stream(self.estimates_(queries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{Estimate, MockPriceEstimating};
    use anyhow::anyhow;
    use futures::StreamExt;
    use model::order::OrderKind;
    use primitive_types::H160;

    #[tokio::test]
    async fn works() {
        let queries = [
            Query {
                sell_token: H160::from_low_u64_le(0),
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            Query {
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            Query {
                sell_token: H160::from_low_u64_le(3),
                buy_token: H160::from_low_u64_le(4),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
        ];

        let mut first = MockPriceEstimating::new();
        first.expect_estimates().times(1).returning(|queries| {
            assert_eq!(queries.len(), 3);
            futures::stream::iter([
                Err(PriceEstimationError::Other(anyhow!(""))),
                Err(PriceEstimationError::UnsupportedToken(
                    H160::from_low_u64_le(2),
                )),
                Err(PriceEstimationError::Other(anyhow!(""))),
            ])
            .enumerate()
            .boxed()
        });
        let mut second = MockPriceEstimating::new();
        second.expect_estimates().times(1).returning(|queries| {
            assert_eq!(queries.len(), 2);
            assert_eq!(queries[0].sell_token, H160::from_low_u64_le(0));
            assert_eq!(queries[1].sell_token, H160::from_low_u64_le(3));
            futures::stream::iter([
                Err(PriceEstimationError::NoLiquidity),
                Ok(Estimate::default()),
            ])
            .enumerate()
            .boxed()
        });
        let third = MockPriceEstimating::new();

        let priority =
            PriorityPriceEstimator::new(vec![Box::new(first), Box::new(second), Box::new(third)]);

        let result = vec_estimates(&priority, &queries).await;
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], Err(PriceEstimationError::NoLiquidity)));
        assert!(matches!(
            result[1],
            Err(PriceEstimationError::UnsupportedToken(_))
        ));
        assert!(matches!(result[2], Ok(_)));
    }
}
