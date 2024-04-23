use {
    super::{compare_error, CompetitionEstimator, PriceRanking},
    crate::price_estimation::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
        QuoteVerificationMode,
    },
    anyhow::Context,
    futures::future::{BoxFuture, FutureExt, TryFutureExt},
    model::order::OrderKind,
    primitive_types::{H160, U256},
    std::{cmp::Ordering, sync::Arc},
};

impl PriceEstimating for CompetitionEstimator<Arc<dyn PriceEstimating>> {
    fn estimate(&self, query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult> {
        async move {
            let out_token = match query.kind {
                OrderKind::Buy => query.sell_token,
                OrderKind::Sell => query.buy_token,
            };
            let get_context = self.ranking.provide_context(out_token);

            // Filter out 0 gas cost estimate because they are obviously wrong and would
            // likely win the price competition which would lead to us paying huge
            // subsidies.
            let gas_is_reasonable = |r: &PriceEstimateResult| r.as_ref().is_ok_and(|r| r.gas > 0);
            let get_results = self
                .produce_results(query.clone(), gas_is_reasonable, |e, q| e.estimate(q))
                .map(Result::Ok);

            let (context, results) = futures::try_join!(get_context, get_results)?;

            let winner = results
                .into_iter()
                .filter(|(_index, r)| r.is_err() || gas_is_reasonable(r))
                .max_by(|a, b| {
                    compare_quote_result(
                        &query,
                        &a.1,
                        &b.1,
                        &context,
                        !matches!(self.verification_mode, QuoteVerificationMode::Unverified),
                    )
                })
                .with_context(|| "all price estimates reported 0 gas cost")
                .map_err(PriceEstimationError::EstimatorInternal)?;
            self.report_winner(&query, query.kind, winner)
        }
        .boxed()
    }
}

fn compare_quote_result(
    query: &Query,
    a: &PriceEstimateResult,
    b: &PriceEstimateResult,
    context: &RankingContext,
    prefer_verified_estimates: bool,
) -> Ordering {
    match (a, b) {
        (Ok(a), Ok(b)) => {
            match (prefer_verified_estimates, a.verified, b.verified) {
                // prefer verified over unverified quotes
                (true, true, false) => Ordering::Greater,
                (true, false, true) => Ordering::Less,
                _ => compare_quote(query, a, b, context),
            }
        }
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

impl PriceRanking {
    async fn provide_context(&self, token: H160) -> Result<RankingContext, PriceEstimationError> {
        match self {
            PriceRanking::MaxOutAmount => Ok(RankingContext {
                native_price: 1.0,
                gas_price: 0.,
            }),
            PriceRanking::BestBangForBuck { native, gas } => {
                let gas = gas.clone();
                let native = native.clone();
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
        }
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
            price_estimation::{
                native::MockNativePriceEstimating,
                MockPriceEstimating,
                QuoteVerificationMode,
            },
        },
        gas_estimation::GasPrice1559,
        model::order::OrderKind,
    };

    fn price(out_amount: u128, gas: u64) -> PriceEstimateResult {
        Ok(Estimate {
            out_amount: out_amount.into(),
            gas,
            ..Default::default()
        })
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
        verification: QuoteVerificationMode,
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
        )
        .with_verification(verification);

        priority
            .estimate(Arc::new(Query {
                kind,
                ..Default::default()
            }))
            .await
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
            QuoteVerificationMode::Unverified,
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
            QuoteVerificationMode::Unverified,
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
            QuoteVerificationMode::Unverified,
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
            QuoteVerificationMode::Unverified,
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
            QuoteVerificationMode::Unverified,
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
            QuoteVerificationMode::Unverified,
        )
        .await;
        assert_eq!(best, price(1, 1_000_000));
    }

    #[tokio::test]
    async fn prefer_verified_over_unverified() {
        let worse_verified_quote = Ok(Estimate {
            out_amount: 900_000.into(),
            gas: 2_000,
            verified: true,
            ..Default::default()
        });
        let better_unverified_quote = Ok(Estimate {
            out_amount: 1_000_000.into(),
            gas: 1_000,
            verified: false,
            ..Default::default()
        });

        let best = best_response(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            vec![
                better_unverified_quote.clone(),
                worse_verified_quote.clone(),
            ],
            QuoteVerificationMode::Prefer,
        )
        .await;
        assert_eq!(best, worse_verified_quote.clone());

        let best = best_response(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            vec![
                better_unverified_quote.clone(),
                worse_verified_quote.clone(),
            ],
            QuoteVerificationMode::RequireWhenPossible,
        )
        .await;
        assert_eq!(best, worse_verified_quote.clone());

        let best = best_response(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            vec![
                better_unverified_quote.clone(),
                worse_verified_quote.clone(),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await;
        assert_eq!(best, better_unverified_quote);
    }
}
