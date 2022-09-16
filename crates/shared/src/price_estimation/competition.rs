use crate::price_estimation::{
    Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
};
use futures::stream::StreamExt;
use model::order::OrderKind;
use std::{cmp::Ordering, num::NonZeroUsize, sync::Arc};

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
}

impl PriceEstimating for RacingCompetitionPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        // Turn the streams from all inner price estimators into a single stream.
        let combined_stream = futures::stream::select_all(self.inner.iter().enumerate().map(
            |(i, (_, estimator))| estimator.estimates(queries).map(move |result| (i, result)),
        ));
        // Stores the estimates for each query and estimator. When we have collected enough results
        // to produce a result of our own the corresponding element is set to None.
        let mut estimates: Vec<Option<Vec<(usize, PriceEstimateResult)>>> =
            vec![Some(Vec::with_capacity(self.inner.len())); queries.len()];
        // Receives items from the combined stream.
        let mut handle_single_result = move |estimator_index: usize, query_index: usize, result| {
            let query = &queries[query_index];
            let estimator = self.inner[estimator_index].0.as_str();
            tracing::debug!(?query, ?result, estimator, "new price estimate");

            // Store the new result in the vector for this query.
            let results = estimates.get_mut(query_index).unwrap().as_mut()?;
            results.push((estimator_index, result));

            // Check if we have enough results to emit a result of our own.
            let successes = results.iter().filter(|result| result.1.is_ok()).count();
            let remaining = self.inner.len() - results.len();
            if successes < self.successful_results_for_early_return.get() && remaining > 0 {
                return None;
            }
            // We have enough successes or there are no remaining estimators running for this query.

            // Find the best result.
            let results = estimates.get_mut(query_index).unwrap().take().unwrap();
            // Unwrap because there has to be at least one result.
            let best_index = best_result(query, results.iter().map(|(_, result)| result)).unwrap();

            // Log and collect metrics.
            let (estimator_index, result) = results.into_iter().nth(best_index).unwrap();
            let estimator = self.inner[estimator_index].0.as_str();
            tracing::debug!(?query, ?result, estimator, "winning price estimate");
            metrics()
                .queries_won
                .with_label_values(&[estimator, query.kind.label()])
                .inc();

            Some((query_index, result))
        };

        combined_stream
            .filter_map(move |(estimator_index, (query_index, result))| {
                let result = handle_single_result(estimator_index, query_index, result);
                futures::future::ready(result)
            })
            .boxed()
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

fn best_result<'a>(
    query: &Query,
    results: impl Iterator<Item = &'a PriceEstimateResult>,
) -> Option<usize> {
    results
        .enumerate()
        .max_by(|a, b| {
            if is_second_result_preferred(query, a.1, b.1) {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        })
        .map(|(index, _)| index)
}

fn is_second_result_preferred(
    query: &Query,
    a: &PriceEstimateResult,
    b: &PriceEstimateResult,
) -> bool {
    match (a, b) {
        (Ok(a), Ok(b)) => is_second_estimate_preferred(query, a, b),
        (Ok(_), Err(_)) => false,
        (Err(_), Ok(_)) => true,
        (Err(a), Err(b)) => is_second_error_preferred(a, b),
    }
}

fn is_second_estimate_preferred(query: &Query, a: &Estimate, b: &Estimate) -> bool {
    match query.kind {
        OrderKind::Buy => b.out_amount < a.out_amount,
        OrderKind::Sell => a.out_amount < b.out_amount,
    }
}

fn is_second_error_preferred(a: &PriceEstimationError, b: &PriceEstimationError) -> bool {
    // NOTE(nlordell): How errors are joined is kind of arbitrary. I decided to
    // just order them in the following priority.
    fn error_to_integer_priority(err: &PriceEstimationError) -> u8 {
        match err {
            // highest priority
            PriceEstimationError::ZeroAmount => 0,
            PriceEstimationError::UnsupportedToken(_) => 1,
            PriceEstimationError::NoLiquidity => 2,
            PriceEstimationError::Other(_) => 3,
            PriceEstimationError::UnsupportedOrderType => 4,
            PriceEstimationError::RateLimited(_) => 5,
            // lowest priority
        }
    }
    error_to_integer_priority(b) < error_to_integer_priority(a)
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
    queries_won: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(global_metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{old_estimator_to_stream, vec_estimates, MockPriceEstimating};
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
                from: None,
                sell_token: H160::from_low_u64_le(0),
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: 1.into(),
                kind: OrderKind::Sell,
            },
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(3),
                buy_token: H160::from_low_u64_le(4),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            Query {
                from: None,
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
                Err(PriceEstimationError::Other(anyhow!("a"))),
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
                    Err(PriceEstimationError::Other(anyhow!("b"))),
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
        // buy 2 is better than buy 1
        assert_eq!(result[1].as_ref().unwrap(), &estimates[1]);
        // pay 1 is better than pay 2
        assert_eq!(result[2].as_ref().unwrap(), &estimates[0]);
        // arbitrarily returns one of equal priority errors
        assert!(matches!(
            result[3].as_ref().unwrap_err(),
            PriceEstimationError::Other(err)
                if err.to_string() == "a" || err.to_string() == "b",
        ));
        // unsupported token has higher priority than no liquidity
        assert!(matches!(
            result[4].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken(_),
        ));
    }

    #[tokio::test]
    async fn racing_estimator_returns_early() {
        let queries = [
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(0),
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            Query {
                from: None,
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
                    already got enough estimates to return early."
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

        let mut stream = racing.estimates(&queries);

        let (i, result) = stream.next().await.unwrap();
        assert_eq!(i, 0);
        assert_eq!(result.as_ref().unwrap(), &estimate(1));

        let (i, result) = stream.next().await.unwrap();
        assert_eq!(i, 1);
        assert_eq!(result.as_ref().unwrap(), &estimate(2));
    }

    #[tokio::test]
    async fn result_ordering() {
        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                ..Default::default()
            }
        }
        let mut first = MockPriceEstimating::new();
        first.expect_estimates().returning(move |_| {
            futures::stream::iter([(1, Ok(estimate(1))), (0, Ok(estimate(0)))]).boxed()
        });
        let mut second = MockPriceEstimating::new();
        second.expect_estimates().returning(move |_| {
            futures::stream::iter([(1, Ok(estimate(1))), (0, Ok(estimate(0)))]).boxed()
        });
        let estimator = CompetitionPriceEstimator::new(vec![
            ("first".to_owned(), Arc::new(first)),
            ("second".to_owned(), Arc::new(second)),
        ]);
        let queries = &[Query {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            ..Default::default()
        }; 2];
        let mut stream = estimator.estimates(queries);

        let (i, result) = stream.next().await.unwrap();
        assert_eq!(i, 1);
        assert_eq!(result.as_ref().unwrap(), &estimate(1));

        let (i, result) = stream.next().await.unwrap();
        assert_eq!(i, 0);
        assert_eq!(result.as_ref().unwrap(), &estimate(0));
    }
}
