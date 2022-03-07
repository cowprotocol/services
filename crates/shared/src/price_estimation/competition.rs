use crate::{
    metrics,
    price_estimation::{
        old_estimator_to_stream, vec_estimates, Estimate, PriceEstimateResult, PriceEstimating,
        PriceEstimationError, Query,
    },
};
use anyhow::{anyhow, Result};
use futures::future;
use futures::FutureExt;
use model::order::OrderKind;
use num::BigRational;
use std::cmp;
use std::num::NonZeroUsize;
use std::sync::Arc;

/// Price estimator that pulls estimates from various sources
/// and competes on the best price. Returns a price estimation
/// early if there is a configurable number of successful estimates
/// for every query or if all price sources returned an estimate.
pub struct RacingCompetitionPriceEstimator {
    inner: Vec<(String, Arc<dyn PriceEstimating>)>,
    successful_results_for_early_return: NonZeroUsize,
}

impl RacingCompetitionPriceEstimator {
    pub fn new(
        inner: Vec<(String, Arc<dyn PriceEstimating>)>,
        successful_results_for_early_return: NonZeroUsize,
    ) -> Self {
        assert!(!inner.is_empty());
        Self {
            inner,
            successful_results_for_early_return,
        }
    }

    fn enough_successful_estimates_for_each_query(
        &self,
        queries: &[Query],
        results: &[(&String, Vec<PriceEstimateResult>)],
    ) -> bool {
        for i in 0..queries.len() {
            if results.iter().filter(|result| result.1[i].is_ok()).count()
                < self.successful_results_for_early_return.into()
            {
                return false;
            }
        }

        true
    }

    async fn estimates_<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> impl Iterator<Item = PriceEstimateResult> + 'a {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        let mut remaining_estimates: Vec<_> = self
            .inner
            .iter()
            .map(|(name, estimator)| {
                async move { (name, vec_estimates(estimator.as_ref(), queries).await) }.boxed()
            })
            .collect();

        let mut computed_estimates = Vec::with_capacity(queries.len());

        while !remaining_estimates.is_empty() {
            let (estimate, _, rest) = future::select_all(remaining_estimates).await;
            computed_estimates.push(estimate);
            if self.enough_successful_estimates_for_each_query(queries, &computed_estimates) {
                break;
            }
            remaining_estimates = rest;
        }

        merge_estimates_from_multiple_estimators(queries, computed_estimates)
    }
}

impl PriceEstimating for RacingCompetitionPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        old_estimator_to_stream(self.estimates_(queries))
    }
}

/// Price estimator that pulls estimates from various sources
/// and competes on the best price.
pub struct CompetitionPriceEstimator {
    inner: RacingCompetitionPriceEstimator,
}

impl CompetitionPriceEstimator {
    pub fn new(inner: Vec<(String, Arc<dyn PriceEstimating>)>) -> Self {
        let number_of_estimators =
            NonZeroUsize::new(inner.len()).expect("Vec of estimators should not be empty.");
        Self {
            inner: RacingCompetitionPriceEstimator::new(inner, number_of_estimators),
        }
    }
}

impl PriceEstimating for CompetitionPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        self.inner.estimates(queries)
    }
}

fn merge_estimates_from_multiple_estimators<'a>(
    queries: &'a [Query],
    all_estimates: Vec<(&'a String, Vec<PriceEstimateResult>)>,
) -> impl Iterator<Item = PriceEstimateResult> + 'a {
    queries.iter().enumerate().map(move |(i, query)| {
        all_estimates
            .iter()
            .fold(
                Err(PriceEstimationError::Other(anyhow!(
                    "no successful price estimates"
                ))),
                |previous_result, (name, estimates)| {
                    fold_price_estimation_result(query, name, previous_result, estimates[i].clone())
                },
            )
            .map(|winning_estimate| {
                tracing::debug!(?query, ?winning_estimate, "winning price estimate");
                metrics()
                    .queries_won
                    .with_label_values(&[
                        winning_estimate.estimator_name,
                        winning_estimate.kind.label(),
                    ])
                    .inc();
                winning_estimate.estimate
            })
    })
}

#[derive(Debug)]
struct EstimateData<'a> {
    kind: OrderKind,
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
            kind: query.kind,
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

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "competition_price_estimator")]
struct Metrics {
    /// Number of wins for a particular price estimator and order kind.
    ///
    /// Note that the order kind is included in the metric. This is because some
    /// estimators only support sell orders (e.g. 1Inch) which would skew the
    /// total metrics. Additionally, this allows us to see how different
    /// estimators behave for buy vs sell orders.
    #[metric(labels("estimator_type", "order_kind"))]
    queries_won: prometheus::CounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::MockPriceEstimating;
    use anyhow::anyhow;
    use futures::StreamExt;
    use model::order::OrderKind;
    use primitive_types::H160;
    use std::time::Duration;
    use tokio::time::sleep;

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
            futures::stream::iter([
                Ok(estimates[0]),
                Ok(estimates[0]),
                Ok(estimates[0]),
                Err(PriceEstimationError::Other(anyhow!(""))),
                Err(PriceEstimationError::NoLiquidity),
            ])
            .enumerate()
            .boxed()
        });
        let mut second = MockPriceEstimating::new();
        second
            .expect_estimates()
            .times(1)
            .returning(move |queries| {
                assert_eq!(queries.len(), 5);
                futures::stream::iter([
                    Err(PriceEstimationError::Other(anyhow!(""))),
                    Ok(estimates[1]),
                    Ok(estimates[1]),
                    Err(PriceEstimationError::Other(anyhow!(""))),
                    Err(PriceEstimationError::UnsupportedToken(H160([0; 20]))),
                ])
                .enumerate()
                .boxed()
            });

        let priority = CompetitionPriceEstimator::new(vec![
            ("first".to_owned(), Arc::new(first)),
            ("second".to_owned(), Arc::new(second)),
        ]);

        let result = vec_estimates(&priority, &queries).await;
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

    #[tokio::test]
    async fn racing_estimator_returns_early() {
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
        ];
        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                ..Default::default()
            }
        }

        let mut first = MockPriceEstimating::new();
        first.expect_estimates().times(1).returning(move |queries| {
            assert_eq!(queries.len(), 2);
            futures::stream::iter([Ok(estimate(1)), Err(PriceEstimationError::NoLiquidity)])
                .enumerate()
                .boxed()
        });

        let mut second = MockPriceEstimating::new();
        second
            .expect_estimates()
            .times(1)
            .returning(move |queries| {
                assert_eq!(queries.len(), 2);
                old_estimator_to_stream(async {
                    sleep(Duration::from_millis(10)).await;
                    [Err(PriceEstimationError::NoLiquidity), Ok(estimate(2))]
                })
            });

        let mut third = MockPriceEstimating::new();
        third.expect_estimates().times(1).returning(move |queries| {
            assert_eq!(queries.len(), 2);
            futures::stream::once(async {
                sleep(Duration::from_millis(20)).await;
                unreachable!(
                    "This estimation gets canceled because the racing estimator\
                    already go enough estimates to return early."
                )
            })
            .boxed()
        });

        let racing = RacingCompetitionPriceEstimator::new(
            vec![
                ("first".to_owned(), Arc::new(first)),
                ("second".to_owned(), Arc::new(second)),
                ("third".to_owned(), Arc::new(third)),
            ],
            NonZeroUsize::new(1).unwrap(),
        );

        let result = vec_estimates(&racing, &queries).await;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].as_ref().unwrap(), &estimate(1));
        assert_eq!(result[1].as_ref().unwrap(), &estimate(2)); // buy 2 is better than buy 1
    }
}
