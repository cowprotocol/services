use {
    super::{native::NativePriceEstimating, QuoteVerificationMode},
    crate::price_estimation::PriceEstimationError,
    anyhow::anyhow,
    futures::{
        future::{BoxFuture, FutureExt},
        stream::{FuturesUnordered, StreamExt},
    },
    gas_estimation::GasPriceEstimating,
    model::order::OrderKind,
    std::{cmp::Ordering, fmt::Debug, num::NonZeroUsize, sync::Arc, time::Instant},
};

mod native;
mod quote;

/// Stage index and index within stage of an estimator stored in the
/// [`CompetitionEstimator`] used as an identifier.
#[derive(Copy, Debug, Clone, Eq, PartialEq)]
struct EstimatorIndex(EstimatorStageIndex, usize);

/// Stage index. It can be primary estimator (no index) or fallback estimator
/// with its index
#[derive(Copy, Debug, Clone, Eq, PartialEq)]
enum EstimatorStageIndex {
    Primary,
    Fallback(usize),
}

type PriceEstimationStage<T> = Vec<(String, T)>;
type ResultWithIndex<O> = (EstimatorIndex, Result<O, PriceEstimationError>);

/// Price estimator that pulls estimates from various sources
/// and competes on the best price. Sources are provided as a list of lists, the
/// outer list representing the sequential stage of the search, and the inner
/// list representing all source that should be queried in parallel in the given
/// stage Returns a price estimation early if there is a configurable number of
/// successful estimates for every query or if all price sources returned an
/// estimate.
pub struct CompetitionEstimator<T> {
    primary_stage: Option<PriceEstimationStage<T>>,
    fallback_stages: Vec<PriceEstimationStage<T>>,
    usable_results_for_early_return: NonZeroUsize,
    ranking: PriceRanking,
    verification_mode: QuoteVerificationMode,
}

impl<T: Send + Sync + 'static> CompetitionEstimator<T> {
    pub fn new(
        primary_stage: Option<PriceEstimationStage<T>>,
        fallback_stages: Vec<PriceEstimationStage<T>>,
        ranking: PriceRanking,
    ) -> Self {
        assert!(!fallback_stages.is_empty());
        assert!(primary_stage.iter().all(|stage| !stage.is_empty()));
        assert!(fallback_stages.iter().all(|stage| !stage.is_empty()));
        Self {
            primary_stage,
            fallback_stages,
            usable_results_for_early_return: NonZeroUsize::MAX,
            ranking,
            verification_mode: QuoteVerificationMode::Unverified,
        }
    }

    /// Configures if verified price estimates should be ranked higher than
    /// unverified ones even if the price is worse.
    /// Per default verified quotes do not get preferred.
    pub fn with_verification(self, mode: QuoteVerificationMode) -> Self {
        Self {
            verification_mode: mode,
            ..self
        }
    }

    /// Enables the fallback estimator to return after it got the configured
    /// number of successful results instead of having to wait for all
    /// estimators to return a result.
    pub fn with_early_return(self, usable_results_for_early_return: NonZeroUsize) -> Self {
        Self {
            usable_results_for_early_return,
            ..self
        }
    }

    /// Returns the number of missing results to fulfill
    /// `usable_results_for_early_return`
    fn missing_results<R>(
        result_is_usable: impl Fn(&Result<R, PriceEstimationError>) -> bool,
        usable_results_for_early_return: NonZeroUsize,
        results: &[ResultWithIndex<R>],
    ) -> usize
    where
        R: Clone + Debug + Send,
    {
        let usable = results.iter().filter(|(_, r)| result_is_usable(r)).count();
        usable_results_for_early_return.get().saturating_sub(usable)
    }

    /// Send the requests for the `query` using the price estimator stage
    /// provided
    async fn send_requests<'a, Q, R>(
        query: Q,
        get_single_result: impl Fn(&T, Q) -> BoxFuture<'_, Result<R, PriceEstimationError>>
            + Send
            + 'static,
        stage: &'a PriceEstimationStage<T>,
        stage_index: EstimatorStageIndex,
    ) -> FuturesUnordered<BoxFuture<'a, (EstimatorIndex, Result<R, PriceEstimationError>)>>
    where
        Q: Clone + Debug + Send + 'static,
        R: Clone + Debug + Send + 'a,
    {
        let mut requests = FuturesUnordered::new();
        let futures = stage.iter().enumerate().map(|(index, (_name, estimator))| {
            get_single_result(estimator, query.clone())
                .map(move |result| (EstimatorIndex(stage_index, index), result))
                .boxed()
        });
        requests.extend(futures);
        requests
    }

    /// Process the requests to the price estimators. Returns `true` if it gets
    /// all needed valid results
    async fn process_requests<Q, R>(
        query: Q,
        result_is_usable: impl Fn(&Result<R, PriceEstimationError>) -> bool + Copy,
        mut requests: FuturesUnordered<
            BoxFuture<'_, (EstimatorIndex, Result<R, PriceEstimationError>)>,
        >,
        stages: &[PriceEstimationStage<T>],
        results: &mut Vec<(EstimatorIndex, Result<R, PriceEstimationError>)>,
        usable_results_for_early_return: NonZeroUsize,
        start: &Instant,
    ) -> bool
    where
        Q: Clone + Debug + Send + 'static,
        R: Clone + Debug + Send,
    {
        while let Some((estimator_index, result)) = requests.next().await {
            let estimator_stage_index = match estimator_index.0 {
                EstimatorStageIndex::Primary => 0,
                EstimatorStageIndex::Fallback(index) => index,
            };
            let (name, _estimator) = &stages[estimator_stage_index][estimator_index.1];
            tracing::debug!(
                ?query,
                ?result,
                estimator = name,
                requests = requests.len(),
                results = results.len(),
                elapsed = ?start.elapsed(),
                "new price estimate"
            );
            results.push((estimator_index, result));

            if Self::missing_results(result_is_usable, usable_results_for_early_return, results)
                == 0
            {
                return true;
            }
        }
        false
    }

    /// Produce results for the given `input` until the caller does not expect
    /// any more results or we produced all the results we can.
    async fn produce_results<Q, R>(
        &self,
        query: Q,
        result_is_usable: impl Fn(&Result<R, PriceEstimationError>) -> bool + Copy,
        get_single_result: impl Fn(&T, Q) -> BoxFuture<'_, Result<R, PriceEstimationError>>
            + Send
            + 'static
            + Copy,
    ) -> Vec<ResultWithIndex<R>>
    where
        Q: Clone + Debug + Send + 'static,
        R: Clone + Debug + Send,
    {
        let start = Instant::now();
        let mut results = vec![];
        let mut stage_index = 0;

        if let Some(stage) = &self.primary_stage {
            // From the primary estimator we require one result
            let usable_results_for_early_return = NonZeroUsize::new(1).unwrap();
            let requests = Self::send_requests(
                query.clone(),
                get_single_result,
                stage,
                EstimatorStageIndex::Primary,
            )
            .await;
            if Self::process_requests(
                query.clone(),
                result_is_usable,
                requests,
                self.fallback_stages.as_ref(),
                &mut results,
                usable_results_for_early_return,
                &start,
            )
            .await
            {
                return results;
            }
        }

        // Fallback method
        while stage_index < self.fallback_stages.len() {
            let mut requests = FuturesUnordered::new();

            // Collect requests until it's at least theoretically possible to produce enough
            // results to return early.
            let requests_for_batch = Self::missing_results(
                result_is_usable,
                self.usable_results_for_early_return,
                &results,
            );
            while stage_index < self.fallback_stages.len() && requests.len() < requests_for_batch {
                let stage = &self
                    .fallback_stages
                    .get(stage_index)
                    .expect("index checked by loop");
                let futures = Self::send_requests(
                    query.clone(),
                    get_single_result,
                    stage,
                    EstimatorStageIndex::Fallback(stage_index),
                )
                .await;

                requests.extend(futures);
                stage_index += 1;
            }

            if Self::process_requests(
                query.clone(),
                result_is_usable,
                requests,
                self.fallback_stages.as_ref(),
                &mut results,
                self.usable_results_for_early_return,
                &start,
            )
            .await
            {
                return results;
            }
        }
        results
    }

    fn report_winner<Q: Debug, R: Debug>(
        &self,
        query: &Q,
        kind: OrderKind,
        (index, result): ResultWithIndex<R>,
    ) -> Result<R, PriceEstimationError> {
        let EstimatorIndex(stage_index, estimator_index) = index;
        let name = match stage_index {
            EstimatorStageIndex::Primary => {
                let primary_stage =
                    self.primary_stage
                        .as_ref()
                        .ok_or(PriceEstimationError::EstimatorInternal(anyhow!(
                            "primary stage not configured"
                        )))?;
                primary_stage[estimator_index].0.as_ref()
            }
            EstimatorStageIndex::Fallback(stage_index) => self.fallback_stages[stage_index]
                [estimator_index]
                .0
                .as_ref(),
        };
        tracing::debug!(?query, ?result, estimator = name, "winning price estimate");
        if result.is_ok() {
            metrics()
                .queries_won
                .with_label_values(&[name, kind.label()])
                .inc();
        }
        result
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{Estimate, MockPriceEstimating, PriceEstimating, Query},
        anyhow::anyhow,
        futures::channel::oneshot::channel,
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
                verification: Default::default(),
                sell_token: H160::from_low_u64_le(0),
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: Default::default(),
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: Default::default(),
                sell_token: H160::from_low_u64_le(2),
                buy_token: H160::from_low_u64_le(3),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: Default::default(),
                sell_token: H160::from_low_u64_le(3),
                buy_token: H160::from_low_u64_le(4),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: Default::default(),
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

        let setup_estimator = |responses: Vec<Result<Estimate, PriceEstimationError>>| {
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
            None,
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
            verification: Default::default(),
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
        let racing: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator::new(
            None,
            vec![vec![
                ("first".to_owned(), Arc::new(first)),
                ("second".to_owned(), Arc::new(second)),
                ("third".to_owned(), Arc::new(third)),
            ]],
            PriceRanking::MaxOutAmount,
        );
        let racing = racing.with_early_return(1.try_into().unwrap());

        let result = racing.estimate(query).await;
        assert_eq!(result.as_ref().unwrap(), &estimate(1));
    }

    #[tokio::test]
    async fn queries_stages_sequentially() {
        let query = Arc::new(Query {
            verification: Default::default(),
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

        let mut primary = MockPriceEstimating::new();
        primary.expect_estimate().times(1).returning(move |_| {
            async {
                sleep(Duration::from_millis(20)).await;
                Err(PriceEstimationError::NoLiquidity)
            }
            .boxed()
        });

        let racing: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator::new(
            Some(vec![("primary".to_owned(), Arc::new(primary))]),
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
            PriceRanking::MaxOutAmount,
        );
        let racing = racing.with_early_return(2.try_into().unwrap());

        let result = racing.estimate(query).await;
        assert_eq!(result.as_ref().unwrap(), &estimate(3));
    }

    #[tokio::test]
    async fn primary_estimator_query() {
        let query = Arc::new(Query {
            verification: Default::default(),
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
        first.expect_estimate().never();

        let mut second = MockPriceEstimating::new();
        second.expect_estimate().never();

        let mut primary = MockPriceEstimating::new();
        primary.expect_estimate().times(1).returning(move |_| {
            async {
                sleep(Duration::from_millis(20)).await;
                Ok(estimate(1))
            }
            .boxed()
        });

        let racing: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator::new(
            Some(vec![("primary".to_owned(), Arc::new(primary))]),
            vec![vec![
                ("first".to_owned(), Arc::new(first)),
                ("second".to_owned(), Arc::new(second)),
            ]],
            PriceRanking::MaxOutAmount,
        );
        let racing = racing.with_early_return(2.try_into().unwrap());

        let result = racing.estimate(query).await;
        assert_eq!(result.as_ref().unwrap(), &estimate(1));
    }

    #[tokio::test]
    async fn combines_stages_if_threshold_bigger_than_next_stage_length() {
        let query = Arc::new(Query {
            verification: Default::default(),
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

        let racing: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator {
            primary_stage: None,
            fallback_stages: vec![
                vec![("first".to_owned(), Arc::new(first))],
                vec![("second".to_owned(), Arc::new(second))],
                vec![("third".to_owned(), Arc::new(third))],
                vec![("fourth".to_owned(), Arc::new(fourth))],
            ],
            usable_results_for_early_return: NonZeroUsize::new(2).unwrap(),
            ranking: PriceRanking::MaxOutAmount,
            verification_mode: QuoteVerificationMode::Unverified,
        };

        racing.estimate(query).await.unwrap();
    }
}
