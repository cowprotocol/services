use {
    super::native::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    anyhow::Context,
    futures::{
        future::Future,
        stream::{FuturesUnordered, StreamExt},
        FutureExt as _,
        TryFutureExt,
    },
    gas_estimation::GasPriceEstimating,
    model::order::OrderKind,
    primitive_types::{H160, U256},
    std::{cmp::Ordering, fmt::Debug, num::NonZeroUsize, sync::Arc, time::Instant},
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
    ranking: PriceRanking,
}

impl<T: Send + Sync + 'static> RacingCompetitionEstimator<T> {
    pub fn new(
        inner: Vec<PriceEstimationStage<T>>,
        successful_results_for_early_return: NonZeroUsize,
        ranking: PriceRanking,
    ) -> Self {
        assert!(!inner.is_empty());
        Self {
            inner,
            successful_results_for_early_return,
            ranking,
        }
    }

    fn estimate_generic<
        Q: Clone + Debug + Send + 'static,
        R: Clone + Debug + Send,
        E: Clone + Debug + Send,
        C,
    >(
        &self,
        query: Q,
        kind: OrderKind,
        get_single_result: impl Fn(&T, Q) -> futures::future::BoxFuture<'_, Result<R, E>>
            + Send
            + 'static,
        pick_best_index: impl Fn(&[(EstimatorIndex, Result<R, E>)], &C) -> Result<usize, E>
            + Send
            + 'static,
        provide_comparison_context: impl Future<Output = Result<C, E>> + Send + 'static,
    ) -> futures::future::BoxFuture<'_, Result<R, E>> {
        let start = Instant::now();
        async move {
            let mut results = vec![];
            let mut iter = self.inner.iter().enumerate().peekable();
            // Process stages sequentially
            'outer: while let Some((stage_index, stage)) = iter.next() {
                // Process estimators within each stage in parallel
                let mut requests: Vec<_> = stage
                    .iter()
                    .enumerate()
                    .map(|(index, (_, estimator))| {
                        get_single_result(estimator, query.clone())
                            .map(move |result| (EstimatorIndex(stage_index, index), result))
                            .boxed()
                    })
                    .collect();

                // Make sure we also use the next stage(s) if this one does not have enough
                // estimators to return early anyways
                let missing_successes =
                    self.successful_results_for_early_return.get() - successes(&results);
                while requests.len() < missing_successes && iter.peek().is_some() {
                    let (next_stage_index, next_stage) = iter.next().unwrap();
                    requests.extend(
                        next_stage
                            .iter()
                            .enumerate()
                            .map(|(index, (_, estimator))| {
                                get_single_result(estimator, query.clone())
                                    .map(move |result| {
                                        (EstimatorIndex(next_stage_index, index), result)
                                    })
                                    .boxed()
                            }),
                    )
                }

                let mut futures: FuturesUnordered<_> = requests.into_iter().collect();
                while let Some((estimator_index, result)) = futures.next().await {
                    results.push((estimator_index, result.clone()));
                    let estimator = &self.inner[estimator_index.0][estimator_index.1].0;
                    tracing::debug!(
                        ?query,
                        ?result,
                        estimator,
                        requests = futures.len(),
                        results = results.len(),
                        elapsed = ?start.elapsed(),
                        "new price estimate"
                    );

                    if successes(&results) >= self.successful_results_for_early_return.get() {
                        break 'outer;
                    }
                }
            }

            let context = provide_comparison_context.await?;
            let best_index = pick_best_index(&results, &context)?;
            let (estimator_index, result) = &results[best_index];
            let (estimator, _) = &self.inner[estimator_index.0][estimator_index.1];
            tracing::debug!(
                ?query,
                ?result,
                estimator,
                elapsed = ?start.elapsed(),
                "winning price estimate"
            );

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

fn successes<R, E>(results: &[(EstimatorIndex, Result<R, E>)]) -> usize {
    results.iter().filter(|(_, result)| result.is_ok()).count()
}

impl PriceEstimating for RacingCompetitionEstimator<Arc<dyn PriceEstimating>> {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        let out_token = match query.kind {
            OrderKind::Buy => query.sell_token,
            OrderKind::Sell => query.buy_token,
        };
        let context_future = self.ranking.provide_context(out_token);
        self.estimate_generic(
            query.clone(),
            query.kind,
            |estimator, query| estimator.estimate(query),
            move |results, context| {
                results
                    .iter()
                    .map(|(_, result)| result)
                    .enumerate()
                    // Filter out 0 gas cost estimate because they are obviously wrong and would
                    // likely win the price competition which would lead to us paying huge
                    // subsidies.
                    .filter(|(_, r)| r.is_err() || r.as_ref().is_ok_and(|e| e.gas > 0))
                    .max_by(|a, b| compare_quote_result(&query, a.1, b.1, context))
                    .map(|(index, _)| index)
                    .with_context(|| "all price estimates reported 0 gas cost")
                    .map_err(PriceEstimationError::EstimatorInternal)
            },
            context_future,
        )
    }
}

impl NativePriceEstimating for RacingCompetitionEstimator<Arc<dyn NativePriceEstimating>> {
    fn estimate_native_price(
        &self,
        token: H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        let context_future = futures::future::ready(Ok(()));
        self.estimate_generic(
            token,
            OrderKind::Buy,
            |estimator, token| estimator.estimate_native_price(token),
            move |results, _context| {
                let best_index = results
                    .iter()
                    .map(|(_, result)| result)
                    .enumerate()
                    .max_by(|a, b| compare_native_result(a.1, b.1))
                    .map(|(index, _)| index)
                    .expect("we get passed at least 1 result and did not filter out any of them");
                Ok(best_index)
            },
            context_future,
        )
    }
}

/// Price estimator that pulls estimates from various sources
/// and competes on the best price.
pub struct CompetitionEstimator<T> {
    inner: RacingCompetitionEstimator<T>,
}

impl<T: Send + Sync + 'static> CompetitionEstimator<T> {
    pub fn new(inner: Vec<Vec<(String, T)>>, ranking: PriceRanking) -> Self {
        let number_of_estimators =
            NonZeroUsize::new(inner.iter().fold(0, |sum, stage| sum + stage.len()))
                .expect("Vec of estimators should not be empty.");
        Self {
            inner: RacingCompetitionEstimator::new(inner, number_of_estimators, ranking),
        }
    }
}

impl PriceEstimating for CompetitionEstimator<Arc<dyn PriceEstimating>> {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.inner.estimate(query)
    }
}

fn compare_quote_result(
    query: &Query,
    a: &PriceEstimateResult,
    b: &PriceEstimateResult,
    context: &RankingContext,
) -> Ordering {
    match (a, b) {
        (Ok(a), Ok(b)) => compare_quote(query, a, b, context),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(a), Err(b)) => compare_error(a, b),
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

fn compare_quote(query: &Query, a: &Estimate, b: &Estimate, context: &RankingContext) -> Ordering {
    let a = context.effective_eth_out(a, query.kind);
    let b = context.effective_eth_out(b, query.kind);
    match query.kind {
        OrderKind::Buy => a.cmp(&b).reverse(),
        OrderKind::Sell => a.cmp(&b),
    }
}

fn compare_error(a: &PriceEstimationError, b: &PriceEstimationError) -> Ordering {
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
    error_to_integer_priority(a).cmp(&error_to_integer_priority(b))
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

/// The metric which decides the winning price estimate.
#[derive(Clone)]
pub enum PriceRanking {
    /// The highest quoted `out_amount` gets picked regardless of trade
    /// complexity.
    MaxOutAmount,
    /// Returns the estimate where `out_amount - fees` is highest.
    BestBangForBuck {
        native: Arc<dyn NativePriceEstimating>,
        gas: Arc<dyn GasPriceEstimating>,
    },
}

impl PriceRanking {
    /// Spawns a task in the background that fetches the needed context for
    /// picking the best estimate without delaying the actual price fetch
    /// requests.
    fn provide_context(
        &self,
        token: H160,
    ) -> impl Future<Output = Result<RankingContext, PriceEstimationError>> {
        let fut = match self {
            PriceRanking::MaxOutAmount => async {
                Ok(RankingContext {
                    native_price: 1.0,
                    gas_price: 0.,
                })
            }
            .boxed(),
            PriceRanking::BestBangForBuck { native, gas } => {
                let gas = gas.clone();
                let native = native.clone();
                async move {
                    let gas = gas
                        .estimate()
                        .map_ok(|gas| gas.effective_gas_price())
                        .map_err(PriceEstimationError::ProtocolInternal);
                    let (native_price, gas_price) =
                        futures::try_join!(native.estimate_native_price(token), gas)?;

                    Ok(RankingContext {
                        native_price,
                        gas_price,
                    })
                }
                .boxed()
            }
        };
        tokio::task::spawn(fut).map(Result::unwrap)
    }
}

struct RankingContext {
    native_price: f64,
    gas_price: f64,
}

impl RankingContext {
    /// Computes the actual received value from this estimate that takes `gas`
    /// into account. If an extremely complex trade route would only result
    /// in slightly more `out_amount` than a simple trade route the simple
    /// trade route would report a higher `out_amount_in_eth`. This is also
    /// referred to as "bang-for-buck" and what matters most to traders.
    fn effective_eth_out(&self, estimate: &Estimate, kind: OrderKind) -> U256 {
        let eth_out = estimate.out_amount.to_f64_lossy() * self.native_price;
        let fees = estimate.gas as f64 * self.gas_price;
        let effective_eth_out = match kind {
            // High fees mean receiving less `buy_token` from your sell order.
            OrderKind::Sell => eth_out - fees,
            // High fees mean paying more `sell_token` for your buy order.
            OrderKind::Buy => eth_out + fees,
        };
        // converts `NaN` and `(-∞, 0]` to `0`
        U256::from_f64_lossy(effective_eth_out)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            gas_price_estimation::FakeGasPriceEstimator,
            price_estimation::{native::MockNativePriceEstimating, MockPriceEstimating},
        },
        anyhow::anyhow,
        futures::channel::oneshot::channel,
        gas_estimation::GasPrice1559,
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
        primitive_types::H160,
        std::time::Duration,
        tokio::time::sleep,
    };

    fn price(out_amount: u128, gas: u64) -> PriceEstimateResult {
        Ok(Estimate {
            out_amount: out_amount.into(),
            gas,
            ..Default::default()
        })
    }

    fn native_price(native_price: f64) -> NativePriceEstimateResult {
        NativePriceEstimateResult::Ok(native_price)
    }

    fn error<T>(err: PriceEstimationError) -> Result<T, PriceEstimationError> {
        Err(err)
    }

    /// Builds a `BestBangForBuck` setup where every token is estimated
    /// to be half as valuable as ETH and the gas price is 2.
    /// That effectively means every unit of `gas` in an estimate worth
    /// 4 units of `out_amount`.
    fn bang_for_buck_ranking() -> PriceRanking {
        // Make `out_token` half as valuable as `ETH` and set gas price to 2.
        // That means 1 unit of `gas` is equal to 4 units of `out_token`.
        let mut native = MockNativePriceEstimating::new();
        native
            .expect_estimate_native_price()
            .returning(move |_| async { Ok(0.5) }.boxed());
        let gas = Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
            base_fee_per_gas: 2.0,
            max_fee_per_gas: 2.0,
            max_priority_fee_per_gas: 2.0,
        }));
        PriceRanking::BestBangForBuck {
            native: Arc::new(native),
            gas,
        }
    }

    /// Returns the best estimate with respect to the provided ranking and order
    /// kind.
    async fn best_response(
        ranking: PriceRanking,
        kind: OrderKind,
        estimates: Vec<PriceEstimateResult>,
    ) -> PriceEstimateResult {
        fn estimator(estimate: PriceEstimateResult) -> Arc<dyn PriceEstimating> {
            let mut estimator = MockPriceEstimating::new();
            estimator
                .expect_estimate()
                .times(1)
                .return_once(move |_| async move { estimate }.boxed());
            Arc::new(estimator)
        }

        let priority: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator::new(
            vec![estimates
                .into_iter()
                .enumerate()
                .map(|(i, e)| (format!("estimator_{i}"), estimator(e)))
                .collect()],
            ranking.clone(),
        );

        priority
            .estimate(Arc::new(Query {
                kind,
                ..Default::default()
            }))
            .await
    }

    /// Returns the best native estimate with respect to the provided ranking
    /// and order kind.
    async fn best_native_response(
        ranking: PriceRanking,
        estimates: Vec<NativePriceEstimateResult>,
    ) -> NativePriceEstimateResult {
        fn estimator(estimate: NativePriceEstimateResult) -> Arc<dyn NativePriceEstimating> {
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

        priority
            .inner
            .estimate_native_price(Default::default())
            .await
    }

    #[tokio::test]
    async fn works() {
        let queries = [
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(0),
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(3),
                buy_token: H160::from_low_u64_le(4),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160::from_low_u64_le(5),
                buy_token: H160::from_low_u64_le(6),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
        ];
        let estimates = [
            Estimate {
                out_amount: 1.into(),
                gas: 1,
                ..Default::default()
            },
            Estimate {
                out_amount: 2.into(),
                gas: 1,
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

        let priority: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator::new(
            vec![vec![
                ("first".to_owned(), Arc::new(first)),
                ("second".to_owned(), Arc::new(second)),
            ]],
            PriceRanking::MaxOutAmount,
        );

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
            block_dependent: false,
        });

        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                gas: 1,
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
                vec![vec![
                    ("first".to_owned(), Arc::new(first)),
                    ("second".to_owned(), Arc::new(second)),
                    ("third".to_owned(), Arc::new(third)),
                ]],
                NonZeroUsize::new(1).unwrap(),
                PriceRanking::MaxOutAmount,
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
            block_dependent: false,
        });

        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                gas: 1,
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
            RacingCompetitionEstimator::new(
                vec![
                    vec![
                        ("first".to_owned(), Arc::new(first)),
                        ("second".to_owned(), Arc::new(second)),
                    ],
                    vec![
                        ("third".to_owned(), Arc::new(third)),
                        ("fourth".to_owned(), Arc::new(fourth)),
                    ],
                ],
                NonZeroUsize::new(2).unwrap(),
                PriceRanking::MaxOutAmount,
            );

        let result = racing.estimate(query).await;
        assert_eq!(result.as_ref().unwrap(), &estimate(3));
    }

    #[tokio::test]
    async fn combines_stages_if_threshold_bigger_than_next_stage_length() {
        let query = Arc::new(Query {
            verification: None,
            sell_token: H160::from_low_u64_le(0),
            buy_token: H160::from_low_u64_le(1),
            in_amount: NonZeroU256::try_from(1).unwrap(),
            kind: OrderKind::Sell,
            block_dependent: false,
        });

        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                gas: 1,
                ..Default::default()
            }
        }

        let (sender, mut receiver) = channel();

        let mut first = MockPriceEstimating::new();

        first.expect_estimate().times(1).return_once(move |_| {
            async {
                sleep(Duration::from_millis(20)).await;
                let _ = sender.send(());
                Ok(estimate(1))
            }
            .boxed()
        });

        let mut second = MockPriceEstimating::new();
        second.expect_estimate().times(1).return_once(move |_| {
            async move {
                // First stage hasn't finished yet
                assert!(receiver.try_recv().unwrap().is_none());
                Err(PriceEstimationError::NoLiquidity)
            }
            .boxed()
        });

        // After the first combined stage is done, we are only missing one positive
        // result, thus we query third but not fourth
        let mut third = MockPriceEstimating::new();
        third
            .expect_estimate()
            .times(1)
            .return_once(move |_| async move { Ok(estimate(1)) }.boxed());

        let mut fourth = MockPriceEstimating::new();
        fourth.expect_estimate().never();

        let racing: RacingCompetitionEstimator<Arc<dyn PriceEstimating>> =
            RacingCompetitionEstimator {
                inner: vec![
                    vec![("first".to_owned(), Arc::new(first))],
                    vec![("second".to_owned(), Arc::new(second))],
                    vec![("third".to_owned(), Arc::new(third))],
                    vec![("fourth".to_owned(), Arc::new(fourth))],
                ],
                successful_results_for_early_return: NonZeroUsize::new(2).unwrap(),
                ranking: PriceRanking::MaxOutAmount,
            };

        racing.estimate(query).await.unwrap();
    }

    /// Verifies that `PriceRanking::BestBangForBuck` correctly adjusts
    /// `out_amount` of quotes based on the `gas` used for the quote. E.g.
    /// if a quote requires a significantly more complex execution but does
    /// not provide a significantly better `out_amount` than a simpler quote
    /// the simpler quote will be preferred.
    #[tokio::test]
    async fn best_bang_for_buck_adjusts_for_complexity() {
        let best = best_response(
            bang_for_buck_ranking(),
            OrderKind::Sell,
            vec![
                // User effectively receives `100_000` `buy_token`.
                price(104_000, 1_000),
                // User effectively receives `99_999` `buy_token`.
                price(107_999, 2_000),
            ],
        )
        .await;
        assert_eq!(best, price(104_000, 1_000));

        let best = best_response(
            bang_for_buck_ranking(),
            OrderKind::Buy,
            vec![
                // User effectively pays `100_000` `sell_token`.
                price(96_000, 1_000),
                // User effectively pays `100_002` `sell_token`.
                price(92_002, 2_000),
            ],
        )
        .await;
        assert_eq!(best, price(96_000, 1_000));
    }

    /// Same test as above but now we also add an estimate that should
    /// win under normal circumstances but the `gas` cost is suspiciously
    /// low so we discard it. This protects us from quoting unreasonably
    /// low fees for user orders.
    #[tokio::test]
    async fn discards_low_gas_cost_estimates() {
        let best = best_response(
            bang_for_buck_ranking(),
            OrderKind::Sell,
            vec![
                // User effectively receives `100_000` `buy_token`.
                price(104_000, 1_000),
                // User effectively receives `99_999` `buy_token`.
                price(107_999, 2_000),
                // User effectively receives `104_000` `buy_token` but the estimate
                // gets discarded because it quotes 0 gas.
                price(104_000, 0),
            ],
        )
        .await;
        assert_eq!(best, price(104_000, 1_000));

        let best = best_response(
            bang_for_buck_ranking(),
            OrderKind::Buy,
            vec![
                // User effectively pays `100_000` `sell_token`.
                price(96_000, 1_000),
                // User effectively pays `100_002` `sell_token`.
                price(92_002, 2_000),
                // User effectively pays `99_000` `sell_token` but the estimate
                // gets discarded because it quotes 0 gas.
                price(99_000, 0),
            ],
        )
        .await;
        assert_eq!(best, price(96_000, 1_000));
    }

    /// If all estimators returned an error we return the one with the highest
    /// priority.
    #[tokio::test]
    async fn returns_highest_priority_error() {
        // Returns errors with highest priority.
        let best = best_response(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            vec![
                error(PriceEstimationError::RateLimited),
                error(PriceEstimationError::ProtocolInternal(anyhow::anyhow!("!"))),
            ],
        )
        .await;
        assert_eq!(best, error(PriceEstimationError::RateLimited));
    }

    /// Any price estimate, no matter how bad, is preferred over an error.
    #[tokio::test]
    async fn prefer_estimate_over_error() {
        let best = best_response(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            vec![
                price(1, 1_000_000),
                error(PriceEstimationError::RateLimited),
            ],
        )
        .await;
        assert_eq!(best, price(1, 1_000_000));
    }

    /// If all estimators returned an error we return the one with the highest
    /// priority.
    #[tokio::test]
    async fn returns_highest_native_price() {
        // Returns errors with highest priority.
        let best = best_native_response(
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
        let best = best_native_response(
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
        let best = best_native_response(
            PriceRanking::MaxOutAmount,
            vec![native_price(1.), error(PriceEstimationError::RateLimited)],
        )
        .await;
        assert_eq!(best, native_price(1.));
    }
}
