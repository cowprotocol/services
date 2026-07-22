use {
    crate::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
        gas::{GAS_PER_WETH_UNWRAP, GAS_PER_WETH_WRAP, SETTLEMENT_OVERHEAD},
    },
    alloy::primitives::Address,
    anyhow::anyhow,
    bad_tokens::list_based::DenyListedTokens,
    futures::FutureExt,
    model::order::BUY_ETH_ADDRESS,
    std::sync::Arc,
    tracing::instrument,
};

/// Adjustment applied to an estimate returned by the inner estimator after a
/// trivial token substitution (e.g. ETH -> WETH) was performed on the query.
enum Modification {
    AddGas(u64),
    NoOp,
}

/// Verifies that buy and sell tokens are supported and handles
/// ETH as buy token appropriately.
pub struct SanitizedPriceEstimator {
    inner: Arc<dyn PriceEstimating>,
    deny_listed_tokens: DenyListedTokens,
    native_token: Address,
    /// Enables the short-circuiting logic in case the sell and buy tokens are
    /// the same
    is_estimating_native_price: bool,
}

impl SanitizedPriceEstimator {
    pub fn new(
        inner: Arc<dyn PriceEstimating>,
        native_token: Address,
        deny_listed_tokens: DenyListedTokens,
        is_estimating_native_price: bool,
    ) -> Self {
        Self {
            inner,
            native_token,
            deny_listed_tokens,
            is_estimating_native_price,
        }
    }

    /// Checks if the traded tokens are supported by the protocol.
    fn handle_deny_listed_tokens(&self, query: &Query) -> Result<(), PriceEstimationError> {
        for token in [query.sell_token, query.buy_token] {
            if self.deny_listed_tokens.contains(&token) {
                return Err(PriceEstimationError::UnsupportedToken {
                    token,
                    reason: "token is deny listed".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Builds the query forwarded to the inner estimator, substituting the
    /// native token for an ETH side and reporting the gas adjustment that has
    /// to be applied to the resulting estimate.
    fn adjust_query(&self, query: &Query) -> (Query, Modification) {
        let mut adjusted_query = Query::clone(query);
        let modification =
            if query.sell_token != self.native_token && query.buy_token == BUY_ETH_ADDRESS {
                tracing::debug!(?query, "estimate price for buying native asset");
                adjusted_query.buy_token = self.native_token;
                Modification::AddGas(GAS_PER_WETH_UNWRAP)
            } else if query.sell_token == BUY_ETH_ADDRESS && query.buy_token != self.native_token {
                tracing::debug!(?query, "estimate price for selling native asset");
                adjusted_query.sell_token = self.native_token;
                Modification::AddGas(GAS_PER_WETH_WRAP)
            } else {
                Modification::NoOp
            };
        (adjusted_query, modification)
    }

    /// Applies the gas adjustment computed by [`Self::adjust_query`] to a
    /// single estimate returned by the inner estimator.
    fn apply_modification(
        modification: Modification,
        mut estimate: Estimate,
    ) -> PriceEstimateResult {
        match modification {
            Modification::AddGas(gas) => {
                estimate.gas = estimate.gas.checked_add(gas).ok_or_else(|| {
                    PriceEstimationError::ProtocolInternal(anyhow!(
                        "cost of converting native asset would overflow gas price"
                    ))
                })?;
                tracing::debug!(
                    ?estimate,
                    "added cost of converting native asset to price estimation"
                );
                Ok(estimate)
            }
            Modification::NoOp => Ok(estimate),
        }
    }
}

impl SanitizedPriceEstimator {
    /// Handles the deny-list check and the trivial 1:1 cases shared by
    /// `estimate` and `estimate_stream`. Returns `Some(result)` when the query
    /// can be answered without consulting the inner estimator, `None`
    /// otherwise.
    fn try_trivial_estimate(&self, query: &Query) -> Option<PriceEstimateResult> {
        if let Err(err) = self.handle_deny_listed_tokens(query) {
            return Some(Err(err));
        }

        // When estimating native price the sell token is substituted by
        // native one. In that case, the output amount of the price
        // estimation can be trivially computed as the same amount as input
        if self.is_estimating_native_price && query.buy_token == query.sell_token {
            let estimation = Estimate {
                out_amount: query.in_amount.get(),
                gas: 0,
                solver: Default::default(),
                verified: true,
                supports_fast_path: false,
                execution: Default::default(),
            };
            tracing::debug!(?query, ?estimation, "generate trivial price estimation");
            return Some(Ok(estimation));
        }

        // sell WETH for ETH => 1 to 1 conversion with cost for unwrapping
        // The resulting gas is the sum of unwrap and the settlement itself
        if query.sell_token == self.native_token && query.buy_token == BUY_ETH_ADDRESS {
            let estimation = Estimate {
                out_amount: query.in_amount.get(),
                gas: GAS_PER_WETH_UNWRAP + SETTLEMENT_OVERHEAD,
                solver: Default::default(),
                verified: true,
                supports_fast_path: false,
                execution: Default::default(),
            };
            tracing::debug!(?query, ?estimation, "generate trivial unwrap estimation");
            return Some(Ok(estimation));
        }

        // sell ETH for WETH => 1 to 1 conversion with cost for wrapping
        // The resulting gas is the sum of unwrap and the settlement itself
        if query.sell_token == BUY_ETH_ADDRESS && query.buy_token == self.native_token {
            let estimation = Estimate {
                out_amount: query.in_amount.get(),
                gas: GAS_PER_WETH_WRAP + SETTLEMENT_OVERHEAD,
                solver: Default::default(),
                verified: true,
                supports_fast_path: false,
                execution: Default::default(),
            };
            tracing::debug!(?query, ?estimation, "generate trivial wrap estimation");
            return Some(Ok(estimation));
        }

        None
    }
}

impl PriceEstimating for SanitizedPriceEstimator {
    #[instrument(skip_all)]
    fn estimate(
        &self,
        query: Arc<Query>,
    ) -> futures::future::BoxFuture<'_, super::PriceEstimateResult> {
        async move {
            if let Some(result) = self.try_trivial_estimate(&query) {
                return result;
            }

            let (adjusted_query, modification) = self.adjust_query(&query);
            let estimate = self.inner.estimate(Arc::new(adjusted_query)).await?;
            Self::apply_modification(modification, estimate)
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{HEALTHY_PRICE_ESTIMATION_TIME, MockPriceEstimating},
        alloy::primitives::{Address, U256},
        model::order::OrderKind,
        number::nonzero::NonZeroU256,
    };

    const BAD_TOKEN: Address = Address::repeat_byte(0x12);

    #[tokio::test]
    async fn handles_trivial_estimates_on_its_own() {
        let deny_listed_tokens = DenyListedTokens::new(vec![BAD_TOKEN]);

        let native_token = Address::with_last_byte(42);

        let queries = [
            // This is the common case (Tokens are supported, distinct and not ETH).
            // Will be estimated by the wrapped_estimator.
            (
                Query {
                    verification: Default::default(),
                    sell_token: Address::with_last_byte(1),
                    buy_token: Address::with_last_byte(2),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    gas: 100,
                    solver: Default::default(),
                    verified: false,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // `sanitized_estimator` will replace `buy_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of unwrapping ETH to Estimate.
            (
                Query {
                    verification: Default::default(),
                    sell_token: Address::with_last_byte(1),
                    buy_token: BUY_ETH_ADDRESS,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    //sanitized_estimator will add ETH_UNWRAP_COST to the gas of any
                    //Query with ETH as the buy_token.
                    gas: GAS_PER_WETH_UNWRAP + 100,
                    solver: Default::default(),
                    verified: false,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // Will cause buffer overflow of gas price in `sanitized_estimator`.
            (
                Query {
                    verification: Default::default(),
                    sell_token: Address::with_last_byte(1),
                    buy_token: BUY_ETH_ADDRESS,
                    in_amount: NonZeroU256::MAX,
                    kind: OrderKind::Buy,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                    "cost of converting native asset would overflow gas price"
                ))),
            ),
            // `sanitized_estimator` will replace `sell_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of wrapping ETH to Estimate.
            (
                Query {
                    verification: Default::default(),
                    sell_token: BUY_ETH_ADDRESS,
                    buy_token: Address::with_last_byte(1),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    //sanitized_estimator will add ETH_WRAP_COST to the gas of any
                    //Query with ETH as the sell_token.
                    gas: GAS_PER_WETH_WRAP + 100,
                    solver: Default::default(),
                    verified: false,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // Can be estimated by `sanitized_estimator` because `buy_token` and `sell_token` are
            // identical.
            (
                Query {
                    verification: Default::default(),
                    sell_token: Address::with_last_byte(1),
                    buy_token: Address::with_last_byte(1),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    gas: 0,
                    solver: Default::default(),
                    verified: true,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // Can be estimated by `sanitized_estimator` because both tokens are the native token.
            (
                Query {
                    verification: Default::default(),
                    sell_token: BUY_ETH_ADDRESS,
                    buy_token: BUY_ETH_ADDRESS,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    gas: 0,
                    solver: Default::default(),
                    verified: true,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // Can be estimated by `sanitized_estimator` because it is a native token unwrap.
            (
                Query {
                    verification: Default::default(),
                    sell_token: native_token,
                    buy_token: BUY_ETH_ADDRESS,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    // Sanitized estimator will report a 1:1 estimate when unwrapping native token.
                    gas: GAS_PER_WETH_UNWRAP + SETTLEMENT_OVERHEAD,
                    solver: Default::default(),
                    verified: true,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // Can be estimated by `sanitized_estimator` because it is a native token wrap.
            (
                Query {
                    verification: Default::default(),
                    sell_token: BUY_ETH_ADDRESS,
                    buy_token: native_token,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    // Sanitized estimator will report a 1:1 estimate when wrapping native token.
                    gas: GAS_PER_WETH_WRAP + SETTLEMENT_OVERHEAD,
                    solver: Default::default(),
                    verified: true,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            (
                Query {
                    verification: Default::default(),
                    sell_token: BAD_TOKEN,
                    buy_token: Address::with_last_byte(1),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Err(PriceEstimationError::UnsupportedToken {
                    token: BAD_TOKEN,
                    reason: "".to_string(),
                }),
            ),
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            (
                Query {
                    verification: Default::default(),
                    sell_token: Address::with_last_byte(1),
                    buy_token: BAD_TOKEN,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Err(PriceEstimationError::UnsupportedToken {
                    token: BAD_TOKEN,
                    reason: "".to_string(),
                }),
            ),
        ];

        // SanitizedPriceEstimator will simply forward the Query in the common case
        let first_forwarded_query = queries[0].0.clone();
        // SanitizedPriceEstimator replaces ETH buy token with native token
        let second_forwarded_query = Query {
            buy_token: native_token,
            ..queries[1].0.clone()
        };
        // SanitizedPriceEstimator replaces ETH buy token with native token
        let third_forwarded_query = Query {
            buy_token: native_token,
            ..queries[2].0.clone()
        };
        // SanitizedPriceEstimator replaces ETH sell token with native token
        let forth_forwarded_query = Query {
            sell_token: native_token,
            ..queries[3].0.clone()
        };

        let mut wrapped_estimator = MockPriceEstimating::new();
        wrapped_estimator
            .expect_estimate()
            .times(1)
            .withf(move |query| **query == first_forwarded_query)
            .returning(|_| {
                async {
                    Ok(Estimate {
                        out_amount: U256::ONE,
                        gas: 100,
                        solver: Default::default(),
                        verified: false,
                        supports_fast_path: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });
        wrapped_estimator
            .expect_estimate()
            .times(1)
            .withf(move |query| **query == second_forwarded_query)
            .returning(|_| {
                async {
                    Ok(Estimate {
                        out_amount: U256::ONE,
                        gas: 100,
                        solver: Default::default(),
                        verified: false,
                        supports_fast_path: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });
        wrapped_estimator
            .expect_estimate()
            .times(1)
            .withf(move |query| **query == third_forwarded_query)
            .returning(|_| {
                async {
                    Ok(Estimate {
                        out_amount: U256::ONE,
                        gas: u64::MAX,
                        solver: Default::default(),
                        verified: false,
                        supports_fast_path: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });
        wrapped_estimator
            .expect_estimate()
            .times(1)
            .withf(move |query| **query == forth_forwarded_query)
            .returning(|_| {
                async {
                    Ok(Estimate {
                        out_amount: U256::ONE,
                        gas: 100,
                        solver: Default::default(),
                        verified: false,
                        supports_fast_path: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });
        let sanitized_estimator = SanitizedPriceEstimator::new(
            Arc::new(wrapped_estimator),
            native_token,
            deny_listed_tokens.clone(),
            true,
        );

        for (query, expectation) in queries {
            let result = sanitized_estimator.estimate(Arc::new(query)).await;
            match result {
                Ok(estimate) => assert_eq!(estimate, expectation.unwrap()),
                Err(err) => {
                    // we only compare the error variant; everything else would be a PITA
                    let reported_error = std::mem::discriminant(&err);
                    let expected_error = std::mem::discriminant(&expectation.unwrap_err());
                    assert_eq!(reported_error, expected_error);
                }
            }
        }

        let queries = [
            // Can be estimated by `sanitized_estimator` because `buy_token` and `sell_token` are
            // identical.
            (
                Query {
                    verification: Default::default(),
                    sell_token: Address::with_last_byte(1),
                    buy_token: Address::with_last_byte(1),
                    in_amount: Default::default(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    gas: 100,
                    solver: Default::default(),
                    verified: true,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
            (
                Query {
                    verification: Default::default(),
                    sell_token: native_token,
                    buy_token: native_token,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                },
                Ok(Estimate {
                    out_amount: U256::ONE,
                    gas: 100,
                    solver: Default::default(),
                    verified: true,
                    supports_fast_path: false,
                    execution: Default::default(),
                }),
            ),
        ];

        // SanitizedPriceEstimator will simply forward the Query in the sell=buy case
        // if it is not calculating native price
        let first_forwarded_query = queries[0].0.clone();

        // SanitizedPriceEstimator will simply forward the Query if sell=buy of native
        // token case if it is not calculating the native price
        let second_forwarded_query = queries[1].0.clone();

        let mut wrapped_estimator = MockPriceEstimating::new();
        wrapped_estimator
            .expect_estimate()
            .times(1)
            .withf(move |query| **query == first_forwarded_query)
            .returning(|_| {
                async {
                    Ok(Estimate {
                        out_amount: U256::ONE,
                        gas: 100,
                        solver: Default::default(),
                        verified: true,
                        supports_fast_path: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });
        wrapped_estimator
            .expect_estimate()
            .times(1)
            .withf(move |query| **query == second_forwarded_query)
            .returning(|_| {
                async {
                    Ok(Estimate {
                        out_amount: U256::ONE,
                        gas: 100,
                        solver: Default::default(),
                        verified: true,
                        supports_fast_path: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });

        let sanitized_estimator_non_native = SanitizedPriceEstimator::new(
            Arc::new(wrapped_estimator),
            native_token,
            deny_listed_tokens,
            false,
        );

        for (query, expectation) in queries {
            let result = sanitized_estimator_non_native
                .estimate(Arc::new(query))
                .await;
            match result {
                Ok(estimate) => assert_eq!(estimate, expectation.unwrap()),
                Err(err) => {
                    // we only compare the error variant; everything else would be a PITA
                    let reported_error = std::mem::discriminant(&err);
                    let expected_error = std::mem::discriminant(&expectation.unwrap_err());
                    assert_eq!(reported_error, expected_error);
                }
            }
        }
    }
}
