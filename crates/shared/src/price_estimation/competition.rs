use super::{Estimate, PriceEstimating, PriceEstimationError, Query};
use anyhow::{anyhow, Result};
use futures::future;
use num::BigRational;
use std::cmp;

/// Price estimator that pulls estimates from various sources
/// and competes on the best price.
pub struct CompetitionPriceEstimator {
    inner: Vec<(String, Box<dyn PriceEstimating>)>,
}

impl CompetitionPriceEstimator {
    pub fn new(inner: Vec<(String, Box<dyn PriceEstimating>)>) -> Self {
        assert!(!inner.is_empty());
        Self { inner }
    }
}

#[async_trait::async_trait]
impl PriceEstimating for CompetitionPriceEstimator {
    async fn estimates(&self, queries: &[Query]) -> Vec<Result<Estimate, PriceEstimationError>> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        let all_estimates =
            future::join_all(self.inner.iter().map(|(name, estimator)| async move {
                (name, estimator.estimates(queries).await)
            }))
            .await;

        queries
            .iter()
            .enumerate()
            .map(|(i, query)| {
                all_estimates
                    .iter()
                    .fold(
                        Err(PriceEstimationError::Other(anyhow!(
                            "no successful price estimates"
                        ))),
                        |previous_result, (name, estimates)| {
                            fold_price_estimation_result(
                                query,
                                name,
                                previous_result,
                                estimates[i].clone(),
                            )
                        },
                    )
                    .map(|winning_estimate| {
                        // The `EstimateData::estimator_name` field is getting
                        // flagged as dead code despited being used in the log
                        // below. Just convince the linter that we use it.
                        let _ = winning_estimate.estimator_name;
                        tracing::debug!(?query, ?winning_estimate, "winning price estimate");
                        winning_estimate.estimate
                    })
            })
            .collect()
    }
}

#[derive(Debug)]
struct EstimateData<'a> {
    estimator_name: &'a str,
    estimate: Estimate,
    sell_over_buy: BigRational,
}

fn fold_price_estimation_result<'a>(
    query: &'a Query,
    estimator_name: &'a str,
    previous_result: Result<EstimateData<'a>, PriceEstimationError>,
    estimate: Result<Estimate, PriceEstimationError>,
) -> Result<EstimateData<'a>, PriceEstimationError> {
    match &estimate {
        Ok(estimate) => tracing::debug!(
            %estimator_name, ?query, ?estimate,
            "received price estimate",
        ),
        Err(err) => tracing::warn!(
            %estimator_name, ?query, ?err,
            "price estimation error",
        ),
    }

    let estimate_with_price = estimate.and_then(|estimate| {
        let sell_over_buy = estimate
            .price_in_sell_token_rational(query)
            .ok_or(PriceEstimationError::ZeroAmount)?;
        Ok(EstimateData {
            estimator_name,
            estimate,
            sell_over_buy,
        })
    });

    match (previous_result, estimate_with_price) {
        // We want to MINIMIZE the `price_in_sell_token_rational` which is
        // computed as `sell_amount / buy_amount`. Minimizing this means
        // increasing the `buy_amount` (i.e. user gets more) or decreasing the
        // `sell_amount` (i.e. user pays less).
        (Ok(previous), Ok(estimate)) => Ok(cmp::min_by_key(previous, estimate, |data| {
            data.sell_over_buy.clone()
        })),
        (Ok(estimate), Err(_)) | (Err(_), Ok(estimate)) => Ok(estimate),
        (Err(previous_err), Err(err)) => Err(join_error(previous_err, err)),
    }
}

fn join_error(a: PriceEstimationError, b: PriceEstimationError) -> PriceEstimationError {
    // NOTE(nlordell): How errors are joined is kind of arbitrary. I decided to
    // just order them in the following priority:
    // - ZeroAmount
    // - UnsupportedToken
    // - NoLiquidity
    // - Other
    // - UnsupportedOrderType
    match (a, b) {
        (err @ PriceEstimationError::ZeroAmount, _)
        | (_, err @ PriceEstimationError::ZeroAmount) => err,
        (err @ PriceEstimationError::UnsupportedToken(_), _)
        | (_, err @ PriceEstimationError::UnsupportedToken(_)) => err,
        (err @ PriceEstimationError::NoLiquidity, _)
        | (_, err @ PriceEstimationError::NoLiquidity) => err,
        (err @ PriceEstimationError::Other(_), _) | (_, err @ PriceEstimationError::Other(_)) => {
            err
        }
        (err @ PriceEstimationError::UnsupportedOrderType, _) => err,
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
                kind: OrderKind::Sell,
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
            Query {
                sell_token: H160::from_low_u64_le(5),
                buy_token: H160::from_low_u64_le(6),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
        ];
        let estimates = [
            Estimate {
                out_amount: 1.into(),
                ..Default::default()
            },
            Estimate {
                out_amount: 2.into(),
                ..Default::default()
            },
        ];

        let mut first = MockPriceEstimating::new();
        first.expect_estimates().times(1).returning(move |queries| {
            assert_eq!(queries.len(), 5);
            vec![
                Ok(estimates[0]),
                Ok(estimates[0]),
                Ok(estimates[0]),
                Err(PriceEstimationError::Other(anyhow!(""))),
                Err(PriceEstimationError::NoLiquidity),
            ]
        });
        let mut second = MockPriceEstimating::new();
        second
            .expect_estimates()
            .times(1)
            .returning(move |queries| {
                assert_eq!(queries.len(), 5);
                vec![
                    Err(PriceEstimationError::Other(anyhow!(""))),
                    Ok(estimates[1]),
                    Ok(estimates[1]),
                    Err(PriceEstimationError::Other(anyhow!(""))),
                    Err(PriceEstimationError::UnsupportedToken(H160([0; 20]))),
                ]
            });

        let priority = CompetitionPriceEstimator::new(vec![
            ("first".to_owned(), Box::new(first)),
            ("second".to_owned(), Box::new(second)),
        ]);

        let result = priority.estimates(&queries).await;
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].as_ref().unwrap(), &estimates[0]);
        assert_eq!(result[1].as_ref().unwrap(), &estimates[1]); // buy 2 is better than buy 1
        assert_eq!(result[2].as_ref().unwrap(), &estimates[0]); // pay 1 is better than pay 2
        assert!(matches!(
            result[3].as_ref().unwrap_err(),
            PriceEstimationError::Other(err)
                if err.to_string() == "no successful price estimates",
        ));
        assert!(matches!(
            result[4].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken(_),
        ));
    }
}
