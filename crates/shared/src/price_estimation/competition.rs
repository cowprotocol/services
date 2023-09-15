use {
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
    std::{
        cmp::Ordering,
        collections::HashMap,
        num::NonZeroUsize,
        sync::{Arc, RwLock},
    },
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

/// Index of an estimator stored in the [`CompetitionPriceEstimator`] used as an
/// identifier.
#[derive(Copy, Debug, Clone, Default, Eq, PartialEq)]
struct EstimatorIndex(usize);

#[derive(Copy, Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd)]
struct Wins(u64);

#[derive(Debug, Clone, Default)]
struct Competition {
    /// How many quotes were requested for this trade.
    total_quotes: u64,
    /// How often each price estimator managed to offer the best quote.
    /// The list is always sorted based on the number of wins in descending
    /// order.
    winners: Vec<(EstimatorIndex, Wins)>,
}

struct Prediction {
    /// Which price estimator will probably provide the best quote.
    winner: EstimatorIndex,
    /// How confident we are in the pick.
    _confidence: f64,
}

/// Collects historic data on which price estimator tends to give the best quote
/// for what kind of trade.
#[derive(Debug, Default)]
struct HistoricalWinners(RwLock<HashMap<Trade, Competition>>);

impl HistoricalWinners {
    /// Updates the metrics for the given trade.
    pub fn record_winner(&self, trade: Trade, winner: EstimatorIndex) {
        let mut lock = self.0.write().unwrap();
        let competition = lock.entry(trade).or_default();
        competition.total_quotes += 1;
        let winner_index = competition
            .winners
            .iter()
            .enumerate()
            .find_map(|(index, (estimator, _))| (*estimator == winner).then_some(index));
        match winner_index {
            Some(winner_index) => {
                let (_, mut wins) = competition.winners[winner_index];
                wins.0 += 1;
                if winner_index != 0 {
                    competition
                        .winners
                        .sort_by_key(|entry| std::cmp::Reverse(entry.1));
                }
            }
            None => {
                competition.winners.push((winner, Wins(1)));
            }
        }
    }

    /// Predicts based on historic data which price estimators should get asked
    /// to return a quote for the given trade in order to be at least
    /// `required_confidence` confident that we'll get the best possible
    /// price.
    pub fn predict_best_candidates(
        &self,
        quote: &Trade,
        required_confidence: f64,
    ) -> Vec<Prediction> {
        let lock = self.0.read().unwrap();
        let Some(competition) = lock.get(quote) else {
            return vec![];
        };
        if competition.total_quotes < 100 {
            // Not enough data to generate a meaningful prediction.
            return vec![];
        }
        let mut total_confidence = 0.;
        let mut predictions = vec![];
        for (estimator, wins) in &competition.winners {
            let confidence = wins.0 as f64 / competition.total_quotes as f64;
            predictions.push(Prediction {
                winner: *estimator,
                _confidence: confidence,
            });
            total_confidence += confidence;
            if total_confidence >= required_confidence {
                break;
            }
        }
        predictions
    }
}

/// Price estimator that pulls estimates from various sources
/// and competes on the best price. Returns a price estimation
/// early if there is a configurable number of successful estimates
/// for every query or if all price sources returned an estimate.
pub struct RacingCompetitionPriceEstimator {
    inner: Vec<(String, Arc<dyn PriceEstimating>)>,
    successful_results_for_early_return: NonZeroUsize,
    competition: Option<HistoricalWinners>,
    /// The likelyhood of us including the winning price estimator based on
    /// historic data.
    required_confidence: f64,
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
            competition: None,
            required_confidence: 1.,
        }
    }
}

impl PriceEstimating for RacingCompetitionPriceEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        async move {
            let predictions = match &self.competition {
                Some(competition) => competition
                    .predict_best_candidates(&Trade::from(&*query), self.required_confidence),
                None => vec![],
            };

            let mut results = vec![];
            let mut futures: Vec<_> = self
                .inner
                .iter()
                .map(|(_, estimator)| estimator.estimate(query.clone()))
                .collect();
            loop {
                let (result, index, rest) = futures::future::select_all(futures).await;
                futures = rest;
                results.push((index, result.clone()));
                let estimator = &self.inner[index].0;
                tracing::debug!(?query, ?result, estimator, "new price estimate");

                let successes = results.iter().filter(|(_, result)| result.is_ok()).count();
                if successes >= self.successful_results_for_early_return.get()
                    || results.len() >= self.inner.len()
                {
                    break;
                }
            }

            let best_index = best_result(&query, results.iter().map(|(_, result)| result)).unwrap();
            let (estimator_index, result) = &results[best_index];
            let (estimator, _) = &self.inner[*estimator_index];
            tracing::debug!(?query, ?result, estimator, "winning price estimate");

            let requests = self.inner.len() as u64;
            metrics()
                .requests
                .with_label_values(&["executed"])
                .inc_by(requests);

            if result.is_ok() {
                // Collect stats for winner predictions.
                metrics()
                    .queries_won
                    .with_label_values(&[estimator, query.kind.label()])
                    .inc();

                if let Some(competition) = &self.competition {
                    let trade = Trade::from(&*query);
                    let estimator = EstimatorIndex(*estimator_index);
                    let was_correct = predictions.iter().any(|p| p.winner == estimator);
                    metrics().record_prediction(&trade, was_correct);
                    competition.record_winner(trade, estimator);
                    metrics()
                        .requests
                        .with_label_values(&["saved"])
                        .inc_by(requests - predictions.len() as u64);
                }
            }

            result.clone()
        }
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

    /// Enables predicting the winning price estimator and gathering of related
    /// metrics.
    pub fn with_predictions(mut self, confidence: f64) -> Self {
        self.inner.competition = Some(Default::default());
        self.inner.required_confidence = confidence;
        self
    }
}

impl PriceEstimating for CompetitionPriceEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.inner.estimate(query)
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

    /// Number of quotes we (un)successfully predicted the winning price
    /// estimator for.
    #[metric(labels("result", "sell_token", "buy_token", "kind"))]
    quote_predictions: prometheus::IntCounterVec,

    /// Number of requests we saved due to greedy selection based on historic
    /// data.
    #[metric(labels("status"))]
    requests: prometheus::IntCounterVec,
}

impl Metrics {
    fn record_prediction(&self, trade: &Trade, correct: bool) {
        let result = match correct {
            true => "correct",
            false => "incorrect",
        };
        let kind = match trade.kind {
            OrderKind::Buy => "buy",
            OrderKind::Sell => "sell",
        };
        self.quote_predictions
            .with_label_values(&[
                result,
                hex::encode(trade.sell_token).as_str(),
                hex::encode(trade.buy_token).as_str(),
                kind,
            ])
            .inc();
    }
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

        let priority = CompetitionPriceEstimator::new(vec![
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

        let racing = RacingCompetitionPriceEstimator::new(
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
}
