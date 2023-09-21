use {
    super::native::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    futures::FutureExt as _,
    model::order::OrderKind,
    primitive_types::H160,
    std::{cmp::Ordering, fmt::Debug, num::NonZeroUsize, sync::Arc},
};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct Trade {
    sell_token: H160,
    buy_token: H160,
    kind: OrderKind,
}

impl From<&Query> for Trade {
    fn from(query: &Query) -> Self {
        Self {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            kind: query.kind,
        }
    }
}

/// Stage index and index within stage of an estimator stored in the
/// [`CompetitionEstimator`] used as an identifier.
#[derive(Copy, Debug, Clone, Default, Eq, PartialEq)]
struct EstimatorIndex(usize, usize);

#[derive(Copy, Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd)]
struct Wins(u64);

type PriceEstimationStage<T> = Vec<(String, T)>;

/// Price estimator that pulls estimates from various sources
/// and competes on the best price. Sources are provided as a list of lists, the
/// outer list representing the sequential stage of the search, and the inner
/// list representing all source that should be queried in parallel in the given
/// stage Returns a price estimation early if there is a configurable number of
/// successful estimates for every query or if all price sources returned an
/// estimate.
pub struct RacingCompetitionEstimator<T> {
    inner: Vec<PriceEstimationStage<T>>,
    successful_results_for_early_return: NonZeroUsize,
}

impl<T: Send + Sync + 'static> RacingCompetitionEstimator<T> {
    pub fn new(
        inner: PriceEstimationStage<T>,
        successful_results_for_early_return: NonZeroUsize,
    ) -> Self {
        assert!(!inner.is_empty());
        Self {
            inner: vec![inner],
            successful_results_for_early_return,
        }
    }

    fn estimate_generic<
        Q: Clone + Debug + Send + 'static,
        R: Clone + Debug + Send,
        E: Clone + Debug + Send,
    >(
        &self,
        query: Q,
        kind: OrderKind,
        get_single_result: impl Fn(&T, Q) -> futures::future::BoxFuture<'_, Result<R, E>>
            + Send
            + 'static,
        compare_results: impl Fn(&Result<R, E>, &Result<R, E>) -> Ordering + Send + 'static,
    ) -> futures::future::BoxFuture<'_, Result<R, E>> {
        async move {
            let mut results = vec![];
            // Process stages sequentially
            'outer: for (stage_index, stage) in self.inner.iter().enumerate() {
                // Process estimators within each stage in parallel
                let mut futures: Vec<_> = stage
                    .iter()
                    .enumerate()
                    .map(|(index, (_, estimator))| {
                        // Return estimator `index` together with the result because `select_all()`
                        // is allowed to shuffle around futures which makes the index return by
                        // `select_all()` meaningless for our purposes.
                        get_single_result(estimator, query.clone())
                            .map(move |result| (index, result))
                            .boxed()
                    })
                    .collect();
                while !futures.is_empty() {
                    let ((estimator_index, result), _, rest) =
                        futures::future::select_all(futures).await;
                    futures = rest;
                    results.push((stage_index, estimator_index, result.clone()));
                    let estimator = &self.inner[stage_index][estimator_index].0;
                    tracing::debug!(?query, ?result, estimator, "new price estimate");

                    let successes = results
                        .iter()
                        .filter(|(_, _, result)| result.is_ok())
                        .count();
                    if successes >= self.successful_results_for_early_return.get() {
                        break 'outer;
                    }
                }
            }

            let best_index = results
                .iter()
                .map(|(_, _, result)| result)
                .enumerate()
                .max_by(|a, b| compare_results(a.1, b.1))
                .map(|(index, _)| index)
                .unwrap();
            let (stage_index, estimator_index, result) = &results[best_index];
            let (estimator, _) = &self.inner[*stage_index][*estimator_index];
            tracing::debug!(?query, ?result, estimator, "winning price estimate");

            let total_estimators = self.inner.iter().fold(0, |sum, inner| sum + inner.len()) as u64;
            let queried_estimators = results.len() as u64;
            metrics()
                .requests
                .with_label_values(&["executed"])
                .inc_by(queried_estimators);
            metrics()
                .requests
                .with_label_values(&["saved"])
                .inc_by(total_estimators - queried_estimators);

            if result.is_ok() {
                // Collect stats for winner predictions.
                metrics()
                    .queries_won
                    .with_label_values(&[estimator, kind.label()])
                    .inc();
            }
            result.clone()
        }
        .boxed()
    }
}

impl PriceEstimating for RacingCompetitionEstimator<Arc<dyn PriceEstimating>> {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.estimate_generic(
            query.clone(),
            query.kind,
            |estimator, query| estimator.estimate(query),
            move |a, b| {
                if is_second_quote_result_preferred(query.as_ref(), a, b) {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            },
        )
    }
}

impl NativePriceEstimating for RacingCompetitionEstimator<Arc<dyn NativePriceEstimating>> {
    fn estimate_native_price(
        &self,
        token: H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        self.estimate_generic(
            token,
            OrderKind::Buy,
            |estimator, token| estimator.estimate_native_price(token),
            move |a, b| {
                if is_second_native_result_preferred(a, b) {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            },
        )
    }
}

/// Price estimator that pulls estimates from various sources
/// and competes on the best price.
pub struct CompetitionEstimator<T> {
    inner: RacingCompetitionEstimator<T>,
}

impl<T: Send + Sync + 'static> CompetitionEstimator<T> {
    pub fn new(inner: Vec<(String, T)>) -> Self {
        let number_of_estimators =
            NonZeroUsize::new(inner.len()).expect("Vec of estimators should not be empty.");
        Self {
            inner: RacingCompetitionEstimator::new(inner, number_of_estimators),
        }
    }
}

impl PriceEstimating for CompetitionEstimator<Arc<dyn PriceEstimating>> {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.inner.estimate(query)
    }
}

fn is_second_quote_result_preferred(
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

fn is_second_native_result_preferred(
    a: &Result<f64, PriceEstimationError>,
    b: &Result<f64, PriceEstimationError>,
) -> bool {
    match (a, b) {
        (Ok(a), Ok(b)) => b >= a,
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
    // Errors are sorted by recoverability. E.g. a rate-limited estimation may
    // succeed if tried again, whereas unsupported order types can never recover
    // unless code changes. This can be used to decide which errors we want to
    // cache
    fn error_to_integer_priority(err: &PriceEstimationError) -> u8 {
        match err {
            // highest priority (prefer)
            PriceEstimationError::RateLimited => 5,
            PriceEstimationError::ProtocolInternal(_) => 4,
            PriceEstimationError::EstimatorInternal(_) => 3,
            PriceEstimationError::UnsupportedToken { .. } => 2,
            PriceEstimationError::NoLiquidity => 1,
            PriceEstimationError::UnsupportedOrderType(_) => 0,
            // lowest priority
        }
    }
    error_to_integer_priority(b) > error_to_integer_priority(a)
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

    /// Number of requests we saved due to greedy selection based on historic
    /// data.
    #[metric(labels("status"))]
    requests: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::MockPriceEstimating,
        anyhow::anyhow,
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
        primitive_types::H160,
        std::time::Duration,
        tokio::time::sleep,
    };

    #[tokio::test]
    async fn works() {
        let queries = [
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(0),
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(3),
                buy_token: H160::from_low_u64_le(4),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(5),
                buy_token: H160::from_low_u64_le(6),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            }),
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

        let setup_estimator = |responses: Vec<PriceEstimateResult>| {
            let mut estimator = MockPriceEstimating::new();
            for response in responses {
                estimator.expect_estimate().times(1).returning(move |_| {
                    let response = response.clone();
                    {
                        async move { response }.boxed()
                    }
                });
            }
            estimator
        };

        let first = setup_estimator(vec![
            Ok(estimates[0]),
            Ok(estimates[0]),
            Ok(estimates[0]),
            Err(PriceEstimationError::ProtocolInternal(anyhow!("a"))),
            Err(PriceEstimationError::NoLiquidity),
        ]);

        let second = setup_estimator(vec![
            Err(PriceEstimationError::ProtocolInternal(anyhow!(""))),
            Ok(estimates[1]),
            Ok(estimates[1]),
            Err(PriceEstimationError::ProtocolInternal(anyhow!("b"))),
            Err(PriceEstimationError::UnsupportedToken {
                token: H160([0; 20]),
                reason: "".to_string(),
            }),
        ]);

        let priority: CompetitionEstimator<Arc<dyn PriceEstimating>> =
            CompetitionEstimator::new(vec![
                ("first".to_owned(), Arc::new(first)),
                ("second".to_owned(), Arc::new(second)),
            ]);

        let result = priority.estimate(queries[0].clone()).await;
        assert_eq!(result.as_ref().unwrap(), &estimates[0]);

        let result = priority.estimate(queries[1].clone()).await;
        // buy 2 is better than buy 1
        assert_eq!(result.as_ref().unwrap(), &estimates[1]);

        let result = priority.estimate(queries[2].clone()).await;
        // pay 1 is better than pay 2
        assert_eq!(result.as_ref().unwrap(), &estimates[0]);

        let result = priority.estimate(queries[3].clone()).await;
        // arbitrarily returns one of equal priority errors
        assert!(matches!(
            result.as_ref().unwrap_err(),
            PriceEstimationError::ProtocolInternal(err)
                if err.to_string() == "a" || err.to_string() == "b",
        ));

        let result = priority.estimate(queries[4].clone()).await;
        // unsupported token has higher priority than no liquidity
        assert!(matches!(
            result.as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken { .. }
        ));
    }

    #[tokio::test]
    async fn racing_estimator_returns_early() {
        let query = Arc::new(Query {
            verification: None,
            sell_token: H160::from_low_u64_le(0),
            buy_token: H160::from_low_u64_le(1),
            in_amount: NonZeroU256::try_from(1).unwrap(),
            kind: OrderKind::Buy,
        });

        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                ..Default::default()
            }
        }

        let mut first = MockPriceEstimating::new();
        first.expect_estimate().times(1).returning(move |_| {
            // immediately return an error (not enough to terminate price competition early)
            async { Err(PriceEstimationError::NoLiquidity) }.boxed()
        });

        let mut second = MockPriceEstimating::new();
        second.expect_estimate().times(1).returning(|_| {
            async {
                sleep(Duration::from_millis(10)).await;
                // return good result after some time; now we can terminate early
                Ok(estimate(1))
            }
            .boxed()
        });

        let mut third = MockPriceEstimating::new();
        third.expect_estimate().times(1).returning(move |_| {
            async {
                sleep(Duration::from_millis(20)).await;
                unreachable!(
                    "This estimation gets canceled because the racing estimator already got \
                     enough estimates to return early."
                )
            }
            .boxed()
        });

        let racing: RacingCompetitionEstimator<Arc<dyn PriceEstimating>> =
            RacingCompetitionEstimator::new(
                vec![
                    ("first".to_owned(), Arc::new(first)),
                    ("second".to_owned(), Arc::new(second)),
                    ("third".to_owned(), Arc::new(third)),
                ],
                NonZeroUsize::new(1).unwrap(),
            );

        let result = racing.estimate(query).await;
        assert_eq!(result.as_ref().unwrap(), &estimate(1));
    }

    #[tokio::test]
    async fn queries_stages_sequentially() {
        let query = Arc::new(Query {
            verification: None,
            sell_token: H160::from_low_u64_le(0),
            buy_token: H160::from_low_u64_le(1),
            in_amount: NonZeroU256::try_from(1).unwrap(),
            kind: OrderKind::Sell,
        });

        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                ..Default::default()
            }
        }

        let mut first = MockPriceEstimating::new();
        first.expect_estimate().times(1).returning(move |_| {
            async {
                // First stage takes longer than second to test they are not executed in
                // parallel
                sleep(Duration::from_millis(20)).await;
                Ok(estimate(1))
            }
            .boxed()
        });

        let mut second = MockPriceEstimating::new();
        second.expect_estimate().times(1).returning(move |_| {
            async {
                sleep(Duration::from_millis(20)).await;
                Err(PriceEstimationError::NoLiquidity)
            }
            .boxed()
        });

        let mut third = MockPriceEstimating::new();
        third
            .expect_estimate()
            .times(1)
            .returning(move |_| async { Ok(estimate(3)) }.boxed());

        let mut fourth = MockPriceEstimating::new();
        fourth.expect_estimate().times(1).returning(move |_| {
            async {
                sleep(Duration::from_millis(10)).await;
                unreachable!(
                    "This estimation gets canceled because the racing estimator already got \
                     enough estimates to return early."
                )
            }
            .boxed()
        });

        let racing: RacingCompetitionEstimator<Arc<dyn PriceEstimating>> =
            RacingCompetitionEstimator {
                inner: vec![
                    vec![
                        ("first".to_owned(), Arc::new(first)),
                        ("second".to_owned(), Arc::new(second)),
                    ],
                    vec![
                        ("third".to_owned(), Arc::new(third)),
                        ("fourth".to_owned(), Arc::new(fourth)),
                    ],
                ],
                successful_results_for_early_return: NonZeroUsize::new(2).unwrap(),
            };

        let result = racing.estimate(query).await;
        assert_eq!(result.as_ref().unwrap(), &estimate(3));
    }
}
