use super::{Estimate, PriceEstimating, PriceEstimationError, Query};
use anyhow::Result;

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
}

#[async_trait::async_trait]
impl PriceEstimating for PriorityPriceEstimator {
    async fn estimates(&self, queries: &[Query]) -> Vec<Result<Estimate, PriceEstimationError>> {
        let mut results = self.inner[0].estimates(queries).await;
        for inner in &self.inner[1..] {
            let retry_indexes = results
                .iter()
                .enumerate()
                .filter(|(_, result)| matches!(result, Err(PriceEstimationError::Other(_))))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if retry_indexes.is_empty() {
                break;
            }
            for index in &retry_indexes {
                let err = match &results[*index] {
                    Err(err) => err,
                    _ => unreachable!(),
                };
                tracing::warn!(?err, "inner price estimator failed");
            }
            let retry_queries = retry_indexes
                .iter()
                .map(|index| queries[*index])
                .collect::<Vec<_>>();
            let retry_results = inner.estimates(&retry_queries).await;
            for (index, result) in retry_indexes.into_iter().zip(retry_results) {
                results[index] = result;
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::MockPriceEstimating;
    use anyhow::anyhow;
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
            vec![
                Err(PriceEstimationError::Other(anyhow!(""))),
                Err(PriceEstimationError::UnsupportedToken(
                    H160::from_low_u64_le(2),
                )),
                Err(PriceEstimationError::Other(anyhow!(""))),
            ]
        });
        let mut second = MockPriceEstimating::new();
        second.expect_estimates().times(1).returning(|queries| {
            assert_eq!(queries.len(), 2);
            assert_eq!(queries[0].sell_token, H160::from_low_u64_le(0));
            assert_eq!(queries[1].sell_token, H160::from_low_u64_le(3));
            vec![
                Err(PriceEstimationError::NoLiquidity),
                Ok(Estimate::default()),
            ]
        });
        let third = MockPriceEstimating::new();

        let priority =
            PriorityPriceEstimator::new(vec![Box::new(first), Box::new(second), Box::new(third)]);

        let result = priority.estimates(&queries).await;
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], Err(PriceEstimationError::NoLiquidity)));
        assert!(matches!(
            result[1],
            Err(PriceEstimationError::UnsupportedToken(_))
        ));
        assert!(matches!(result[2], Ok(_)));
    }
}
