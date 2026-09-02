use {
    super::{CompetitionEstimator, EstimatorIndex, PriceRanking, compare_error},
    crate::{
        CompetitionPriceEstimating,
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
        QuoteVerificationMode,
        RankedEstimates,
        StreamingPriceEstimating,
    },
    alloy::primitives::U256,
    event_bus_dto::{
        price_estimate::{EstimateResult, PriceEstimateEvent},
        query::{OrderKind as DtoOrderKind, QueryFields},
        winning_price_estimate::WinningPriceEstimateEvent,
    },
    futures::{
        future::{BoxFuture, FutureExt, TryFutureExt},
        stream::{BoxStream, FuturesUnordered, StreamExt},
    },
    model::order::OrderKind,
    std::{
        cmp::Ordering,
        sync::Arc,
        time::{Duration, Instant},
    },
    tracing::instrument,
};

impl CompetitionPriceEstimating for CompetitionEstimator<Arc<dyn PriceEstimating>> {
    #[instrument(skip_all)]
    fn estimates(
        &self,
        mut query: Arc<Query>,
    ) -> BoxFuture<'_, Result<RankedEstimates, PriceEstimationError>> {
        Arc::make_mut(&mut query).timeout /= self.stages.len() as u32;

        async move {
            let get_context = self.ranking.provide_context(&query);

            let get_results = self
                .produce_results(query.clone(), is_reasonable, |context| {
                    // Call estimate() eagerly so its side-effects still happen
                    // when an early-return drops the future before it's polled.
                    let start = Instant::now();
                    let estimator_name = context.name;
                    let inner_query = context.query.clone();
                    context
                        .estimator
                        .estimate(context.query.clone())
                        .map(move |res| {
                            if res.is_ok() {
                                emit_quote_event(
                                    estimator_name,
                                    &inner_query,
                                    &res,
                                    start.elapsed(),
                                );
                            }
                            res
                        })
                        .boxed()
                })
                .map(Result::Ok);

            let (context, mut results) = futures::try_join!(get_context, get_results)?;

            // Keep all errors, but drop unreasonable Ok results.
            results.retain(|(_, r)| r.is_err() || is_reasonable(r));

            // Rank all estimates from best to worst so callers can inspect the
            // full ordering (e.g. the reference score of the winner).
            results.sort_by(|(_, a), (_, b)| {
                compare_quote_result(&query, a, b, &context, self.verification_mode).reverse()
            });

            let mut results = results.into_iter();
            let Some(winner) = results.next() else {
                return Err(unreasonable_estimates_error());
            };

            self.report_winner(&query, query.kind, &winner);
            match winner {
                (_, Err(err)) => Err(err),
                (EstimatorIndex(stage_index, estimator_index), Ok(quote)) => {
                    let (name, _) = &self.stages[stage_index][estimator_index];
                    emit_winning_price_estimate_event(name, &query);
                    let rest = results.filter_map(|(_, r)| r.ok());
                    Ok(RankedEstimates::new(quote, rest))
                }
            }
        }
        .boxed()
    }
}

impl StreamingPriceEstimating for CompetitionEstimator<Arc<dyn PriceEstimating>> {
    /// Runs every estimator concurrently across all stages and forwards a quote
    /// only when it ranks strictly better than the best one already sent, so
    /// the client sees a series that improves in *ranking order* and never
    /// regresses. The first successful quote goes out as soon as the
    /// fastest solver answers. The caller stops by dropping the stream.
    ///
    /// "Better" is the same ranking the one-shot [`Self::estimates`] uses.
    /// A verified quote outranks an unverified one regardless of `out_amount`,
    /// so a later verified quote can supersede an earlier unverified one that
    /// had a higher nominal amount.
    ///
    /// Errors are not forwarded as they arrive. If no quote is ever produced,
    /// the stream ends with the single error the one-shot [`Self::estimates`]
    /// would return for the same query: the highest-priority estimator error,
    /// or the "unreasonable estimates" error when every quote had 0 gas or 0
    /// out_amount.
    fn estimate_stream(&self, query: Arc<Query>) -> BoxStream<'_, PriceEstimateResult> {
        async_stream::stream! {
            let mut estimates = self
                .stages
                .iter()
                .flatten()
                .map(|(_name, estimator)| estimator.estimate(query.clone()))
                .collect::<FuturesUnordered<_>>()
                // Only errors and reasonable estimates can be ranked
                .filter(|r| std::future::ready(r.is_err() || is_reasonable(r)));

            let context_fut = self.ranking.provide_context(&query).shared();

            // Collect estimates concurrently while fetching the ranking context;
            // they can't be ranked before it resolves.
            let mut results: Vec<_> = (&mut estimates)
                .take_until(context_fut.clone())
                .collect()
                .await;

            let context = match context_fut.await {
                Ok(context) => context,
                // Without a ranking context we cannot rank anything, so fail the
                // whole stream like the one-shot path does on a context error.
                Err(err) => {
                    yield Err(err);
                    return;
                }
            };

            // Replay the buffered results (arrival order), then continue draining
            // the live stream. Every result is kept so that, if no quote is ever
            // forwarded, the terminal error can be picked as `estimate` does.
            let mut best: Option<Estimate> = None;
            let mut stream = futures::stream::iter(std::mem::take(&mut results)).chain(estimates);
            while let Some(result) = stream.next().await {
                if let Ok(estimate) = &result {
                    let beats_best = best.as_ref().is_none_or(|best| {
                        compare_quote_result(
                            &query,
                            &result,
                            &Ok(best.clone()),
                            &context,
                            self.verification_mode,
                        )
                        .is_gt()
                    });
                    if beats_best {
                        best = Some(estimate.clone());
                        yield Ok(estimate.clone());
                    }
                }
                results.push(result);
            }

            if best.is_none() {
                yield results
                    .into_iter()
                    .max_by(|a, b| compare_quote_result(&query, a, b, &context, self.verification_mode))
                    .unwrap_or_else(|| Err(unreasonable_estimates_error()));
            }
        }
        .boxed()
    }
}

fn is_reasonable(result: &PriceEstimateResult) -> bool {
    result
        .as_ref()
        .is_ok_and(|estimate| estimate.gas > 0 && !estimate.out_amount.is_zero())
}

fn unreasonable_estimates_error() -> PriceEstimationError {
    PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
        "all price estimates were unreasonable (0 gas or 0 out_amount)"
    ))
}

fn compare_quote_result(
    query: &Query,
    a: &PriceEstimateResult,
    b: &PriceEstimateResult,
    context: &RankingContext,
    verification_mode: QuoteVerificationMode,
) -> Ordering {
    let prefer_verified = !matches!(verification_mode, QuoteVerificationMode::Unverified);
    match (a, b) {
        (Ok(a), Ok(b)) => {
            match (prefer_verified, a.verified, b.verified) {
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
    let a = context.effective_out_amount(a, query);
    let b = context.effective_out_amount(b, query);
    match query.kind {
        OrderKind::Buy => a.cmp(&b).reverse(),
        OrderKind::Sell => a.cmp(&b),
    }
}

impl PriceRanking {
    async fn provide_context(&self, query: &Query) -> Result<RankingContext, PriceEstimationError> {
        match self {
            PriceRanking::MaxOutAmount => Ok(RankingContext {
                sell_token_native_price: 1.0,
                gas_price: 0.,
            }),
            PriceRanking::BestBangForBuck { native, gas } => {
                let gas = gas.clone();
                let native = native.clone();
                let gas = gas
                    .effective_gas_price()
                    .map_err(PriceEstimationError::ProtocolInternal);
                let (sell_token_native_price, gas_price) = futures::try_join!(
                    native.estimate_native_price(query.sell_token, query.timeout),
                    gas
                )?;

                Ok(RankingContext {
                    sell_token_native_price,
                    gas_price: gas_price as f64,
                })
            }
        }
    }
}

#[derive(Clone)]
struct RankingContext {
    /// Native price of the sell token (ETH per unit of sell_token).
    sell_token_native_price: f64,
    gas_price: f64,
}

impl RankingContext {
    /// Uses the native sell token price to compute a qoute's effective out
    /// amount that takes the quote's gas cost into account.
    /// sell orders: buy token they receive after fees
    /// buy orders: sell tokens they have to pay including fees
    ///
    /// Fees ultimately get reported to the user in the sell token so it's
    /// important that the quote ranking logic is aligned with that and
    /// ranks quotes always using the sell token native price and never
    /// the buy token native price.
    fn effective_out_amount(&self, estimate: &Estimate, query: &Query) -> U256 {
        let gas_cost_in_eth = estimate.gas as f64 * self.gas_price;
        let gas_cost_in_sell = gas_cost_in_eth / self.sell_token_native_price;
        let (sell_amount, buy_amount) = estimate.amounts(query);
        let effective_out_amount = match query.kind {
            // Convert the sell-token gas fee to buy-token units via the
            // quote's exchange rate, then subtract from what the user gets.
            OrderKind::Sell => {
                let buy_amount = f64::from(buy_amount);
                let sell_amount = f64::from(sell_amount);
                let gas_cost_in_buy = gas_cost_in_sell * (buy_amount / sell_amount);
                buy_amount - gas_cost_in_buy
            }
            // The user pays sell_amount plus the gas fee, both in sell_token.
            OrderKind::Buy => f64::from(sell_amount) + gas_cost_in_sell,
        };
        match effective_out_amount {
            // converts `NaN` and `(-∞, 0]` to `0`
            v if v.is_sign_negative() || v.is_nan() => U256::ZERO,
            // Previous case already covered negative infinity
            v if v.is_infinite() => U256::MAX,
            // Note on truncation: previously we used primitive_types::U256::from_f64_lossy which
            // truncated the floating point, while alloy is slightly more faithful to the original
            // value and rounds to closest integer: [0, 0.5) => 0, [0.5, 1] => 1
            // Source: https://github.com/paritytech/parity-common/blob/2b887751f2bd3aafe7d6b33197f5a4a35ae61d34/primitive-types/src/fp_conversion.rs#L4-L13
            v => U256::saturating_from(v.trunc()),
        }
    }
}

fn query_fields(query: &Query) -> QueryFields {
    QueryFields {
        sell_token: query.sell_token.to_string(),
        buy_token: query.buy_token.to_string(),
        in_amount: query.in_amount.to_string(),
        kind: match query.kind {
            OrderKind::Sell => DtoOrderKind::Sell,
            OrderKind::Buy => DtoOrderKind::Buy,
        },
    }
}

fn emit_winning_price_estimate_event(estimator_name: &str, query: &Query) {
    observe::event_bus::publish_event(WinningPriceEstimateEvent {
        query: query_fields(query),
        estimator: estimator_name.to_owned(),
    });
}

fn emit_quote_event(
    estimator_name: &str,
    query: &Query,
    result: &PriceEstimateResult,
    elapsed: Duration,
) {
    let event = PriceEstimateEvent {
        query: query_fields(query),
        from: query.verification.from.to_string(),
        // even though as_millis returns u128 timeout and elapsed are not expected to even surpass
        // JSON's 53bit limit as u53::MAX would roughly be half a milion years, furthermore,
        // the cast truncates values to u64
        timeout: query.timeout.as_millis() as u64,
        elapsed: elapsed.as_millis() as u64,
        estimator: estimator_name.to_owned(),
        result: match result {
            Ok(estimate) => EstimateResult::Ok {
                out_amount: estimate.out_amount.to_string(),
                gas: estimate.gas.to_string(),
                verified: estimate.verified,
            },
            Err(err) => EstimateResult::Err {
                error: err.to_string(),
            },
        },
    };
    observe::event_bus::publish_event(event);
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            CompetitionPriceEstimating,
            Estimate,
            MockPriceEstimating,
            QuoteVerificationMode,
            native::MockNativePriceEstimating,
        },
        alloy::{eips::eip1559::Eip1559Estimation, primitives::U256},
        gas_price_estimation::FakeGasPriceEstimator,
        model::order::OrderKind,
        number::nonzero::NonZeroU256,
    };

    fn price(out_amount: u128, gas: u64) -> PriceEstimateResult {
        Ok(Estimate {
            out_amount: U256::from(out_amount),
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
            .returning(move |_, _| async { Ok(0.5) }.boxed());
        let gas = Arc::new(FakeGasPriceEstimator::new(Eip1559Estimation {
            max_fee_per_gas: 2,
            max_priority_fee_per_gas: 2,
        }));
        PriceRanking::BestBangForBuck {
            native: Arc::new(native),
            gas,
        }
    }

    /// Runs all provided estimators and returns all ranked quotes best-first,
    /// or the highest-priority error if every estimator failed.
    ///
    /// `in_amount` is the query's sell_amount for sell orders and buy_amount
    /// for buy orders. It's part of the sell-order ranking formula, so tests
    /// that depend on `BestBangForBuck` sell rankings must pass a realistic
    /// value.
    async fn competition_results(
        ranking: PriceRanking,
        kind: OrderKind,
        in_amount: u128,
        estimates: Vec<PriceEstimateResult>,
        verification: QuoteVerificationMode,
    ) -> Result<Vec<Estimate>, PriceEstimationError> {
        fn estimator(estimate: PriceEstimateResult) -> Arc<dyn PriceEstimating> {
            let mut estimator = MockPriceEstimating::new();
            estimator
                .expect_estimate()
                .times(1)
                .return_once(move |_| async move { estimate }.boxed());
            Arc::new(estimator)
        }

        let priority: CompetitionEstimator<Arc<dyn PriceEstimating>> = CompetitionEstimator::new(
            vec![
                estimates
                    .into_iter()
                    .enumerate()
                    .map(|(i, e)| (format!("estimator_{i}"), estimator(e)))
                    .collect(),
            ],
            ranking.clone(),
        )
        .with_verification(verification);

        priority
            .estimates(Arc::new(Query {
                kind,
                in_amount: NonZeroU256::try_from(in_amount).unwrap(),
                ..Default::default()
            }))
            .await
            .map(|r| r.into_vec())
    }

    /// Verifies that `PriceRanking::BestBangForBuck` correctly adjusts
    /// `out_amount` of quotes based on the `gas` used for the quote. E.g.
    /// if a quote requires a significantly more complex execution but does
    /// not provide a significantly better `out_amount` than a simpler quote
    /// the simpler quote will be preferred, and both quotes appear in the
    /// ranked output in that order.
    #[tokio::test]
    async fn best_bang_for_buck_adjusts_for_complexity() {
        // sell_amount = 100_000, sell_token native price = 0.5, gas_price = 2
        // => 1 unit of gas costs 4 units of sell_token.
        let quotes = competition_results(
            bang_for_buck_ranking(),
            OrderKind::Sell,
            100_000,
            vec![
                // Gas costs 4_000 sell_token; expressed in buy_token via the
                // 1.04 quote rate that's 4_160 => effective receive 99_840.
                price(104_000, 1_000),
                // Gas costs 8_000 sell_token; at rate 1.07999 that's 8_639.92
                // buy_token => effective receive 99_359.
                price(107_999, 2_000),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap();
        assert_eq!(
            quotes,
            vec![
                price(104_000, 1_000).unwrap(),
                price(107_999, 2_000).unwrap(),
            ]
        );

        let quotes = competition_results(
            bang_for_buck_ranking(),
            OrderKind::Buy,
            100_000,
            vec![
                // User effectively pays `100_000` `sell_token`.
                price(96_000, 1_000),
                // User effectively pays `100_002` `sell_token`.
                price(92_002, 2_000),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap();
        assert_eq!(
            quotes,
            vec![price(96_000, 1_000).unwrap(), price(92_002, 2_000).unwrap(),]
        );
    }

    /// Same test as above but now we also add an estimate that should
    /// win under normal circumstances but the `gas` cost is suspiciously
    /// low so we discard it. This protects us from quoting unreasonably
    /// low fees for user orders.
    #[tokio::test]
    async fn discards_low_gas_cost_estimates() {
        let quotes = competition_results(
            bang_for_buck_ranking(),
            OrderKind::Sell,
            100_000,
            vec![
                // Effective receive 99_840 buy_token (see test above).
                price(104_000, 1_000),
                // Effective receive 99_359 buy_token (see test above).
                price(107_999, 2_000),
                // Would win on raw out_amount, but discarded because gas=0.
                price(104_000, 0),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap();
        assert_eq!(
            quotes,
            vec![
                price(104_000, 1_000).unwrap(),
                price(107_999, 2_000).unwrap(),
            ]
        );

        let quotes = competition_results(
            bang_for_buck_ranking(),
            OrderKind::Buy,
            100_000,
            vec![
                // User effectively pays `100_000` `sell_token`.
                price(96_000, 1_000),
                // User effectively pays `100_002` `sell_token`.
                price(92_002, 2_000),
                // Would win on raw out_amount, but discarded because gas=0.
                price(99_000, 0),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap();
        assert_eq!(
            quotes,
            vec![price(96_000, 1_000).unwrap(), price(92_002, 2_000).unwrap(),]
        );
    }

    /// If all estimators returned an error we return the one with the highest
    /// priority.
    #[tokio::test]
    async fn returns_highest_priority_error() {
        let err = competition_results(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            1,
            vec![
                error(PriceEstimationError::RateLimited),
                error(PriceEstimationError::ProtocolInternal(anyhow::anyhow!("!"))),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap_err();
        assert_eq!(err, PriceEstimationError::RateLimited);
    }

    /// Any price estimate, no matter how bad, is preferred over an error.
    /// The error is not included in the ranked output.
    #[tokio::test]
    async fn prefer_estimate_over_error() {
        let quotes = competition_results(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            1,
            vec![
                price(1, 1_000_000),
                error(PriceEstimationError::RateLimited),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap();
        assert_eq!(quotes, vec![price(1, 1_000_000).unwrap()]);
    }

    #[tokio::test]
    async fn prefer_verified_over_unverified() {
        let worse_verified_quote = Ok(Estimate {
            out_amount: U256::from(900_000),
            gas: 2_000,
            verified: true,
            ..Default::default()
        });
        let better_unverified_quote = Ok(Estimate {
            out_amount: U256::from(1_000_000),
            gas: 1_000,
            verified: false,
            ..Default::default()
        });

        // With Prefer: verified quote leads even though price is worse.
        let quotes = competition_results(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            1,
            vec![
                better_unverified_quote.clone(),
                worse_verified_quote.clone(),
            ],
            QuoteVerificationMode::Prefer,
        )
        .await
        .unwrap();
        assert_eq!(
            quotes,
            vec![
                worse_verified_quote.clone().unwrap(),
                better_unverified_quote.clone().unwrap(),
            ]
        );

        // Without verification preference: better price leads regardless of
        // verification status.
        let quotes = competition_results(
            PriceRanking::MaxOutAmount,
            OrderKind::Sell,
            1,
            vec![
                better_unverified_quote.clone(),
                worse_verified_quote.clone(),
            ],
            QuoteVerificationMode::Unverified,
        )
        .await
        .unwrap();
        assert_eq!(
            quotes,
            vec![
                better_unverified_quote.clone().unwrap(),
                worse_verified_quote.clone().unwrap(),
            ]
        );
    }
}
