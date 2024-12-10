use {
    crate::{
        bad_token::{BadTokenDetecting, TokenQuality},
        price_estimation::{
            gas::{GAS_PER_WETH_UNWRAP, GAS_PER_WETH_WRAP},
            Estimate,
            PriceEstimating,
            PriceEstimationError,
            Query,
        },
    },
    anyhow::anyhow,
    futures::FutureExt,
    model::order::BUY_ETH_ADDRESS,
    primitive_types::H160,
    std::sync::Arc,
};

/// Verifies that buy and sell tokens are supported and handles
/// ETH as buy token appropriately.
pub struct SanitizedPriceEstimator {
    inner: Arc<dyn PriceEstimating>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    native_token: H160,
}

impl SanitizedPriceEstimator {
    pub fn new(
        inner: Arc<dyn PriceEstimating>,
        native_token: H160,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
    ) -> Self {
        Self {
            inner,
            native_token,
            bad_token_detector,
        }
    }

    /// Checks if the traded tokens are supported by the protocol.
    async fn handle_bad_tokens(&self, query: &Query) -> Result<(), PriceEstimationError> {
        for token in [query.sell_token, query.buy_token] {
            match self.bad_token_detector.detect(token).await {
                Err(err) => return Err(PriceEstimationError::ProtocolInternal(err)),
                Ok(TokenQuality::Bad { reason }) => {
                    return Err(PriceEstimationError::UnsupportedToken { token, reason })
                }
                _ => (),
            }
        }
        Ok(())
    }
}

impl PriceEstimating for SanitizedPriceEstimator {
    fn estimate(
        &self,
        query: Arc<Query>,
    ) -> futures::future::BoxFuture<'_, super::PriceEstimateResult> {
        async move {
            self.handle_bad_tokens(&query).await?;

            // buy_token == sell_token => 1 to 1 conversion
            if query.buy_token == query.sell_token {
                let estimation = Estimate {
                    out_amount: query.in_amount.get(),
                    gas: 0,
                    solver: Default::default(),
                    verified: true,
                    execution: Default::default(),
                };
                tracing::debug!(?query, ?estimation, "generate trivial price estimation");
                return Ok(estimation);
            }

            // sell WETH for ETH => 1 to 1 conversion with cost for unwrapping
            if query.sell_token == self.native_token && query.buy_token == BUY_ETH_ADDRESS {
                let estimation = Estimate {
                    out_amount: query.in_amount.get(),
                    gas: GAS_PER_WETH_UNWRAP,
                    solver: Default::default(),
                    verified: true,
                    execution: Default::default(),
                };
                tracing::debug!(?query, ?estimation, "generate trivial unwrap estimation");
                return Ok(estimation);
            }

            // sell ETH for WETH => 1 to 1 conversion with cost for wrapping
            if query.sell_token == BUY_ETH_ADDRESS && query.buy_token == self.native_token {
                let estimation = Estimate {
                    out_amount: query.in_amount.get(),
                    gas: GAS_PER_WETH_WRAP,
                    solver: Default::default(),
                    verified: true,
                    execution: Default::default(),
                };
                tracing::debug!(?query, ?estimation, "generate trivial wrap estimation");
                return Ok(estimation);
            }

            enum Modification {
                AddGas(u64),
            }

            let mut adjusted_query = Query::clone(&*query);
            let modification = if query.sell_token != self.native_token
                && query.buy_token == BUY_ETH_ADDRESS
            {
                tracing::debug!(?query, "estimate price for buying native asset");
                adjusted_query.buy_token = self.native_token;
                Some(Modification::AddGas(GAS_PER_WETH_UNWRAP))
            } else if query.sell_token == BUY_ETH_ADDRESS && query.buy_token != self.native_token {
                tracing::debug!(?query, "estimate price for selling native asset");
                adjusted_query.sell_token = self.native_token;
                Some(Modification::AddGas(GAS_PER_WETH_WRAP))
            } else {
                None
            };

            let mut estimate = self.inner.estimate(Arc::new(adjusted_query)).await?;

            match modification {
                Some(Modification::AddGas(gas)) => {
                    estimate.gas = estimate.gas.checked_add(gas).ok_or_else(|| {
                        PriceEstimationError::ProtocolInternal(anyhow!(
                            "cost of converting native asset would overflow gas price"
                        ))
                    })?;
                    tracing::debug!(
                        ?query,
                        ?estimate,
                        "added cost of converting native asset to price estimation"
                    );
                    Ok(estimate)
                }
                None => Ok(estimate),
            }
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            bad_token::{MockBadTokenDetecting, TokenQuality},
            price_estimation::MockPriceEstimating,
        },
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
        primitive_types::{H160, U256},
    };

    const BAD_TOKEN: H160 = H160([0x12; 20]);

    #[tokio::test]
    async fn handles_trivial_estimates_on_its_own() {
        let mut bad_token_detector = MockBadTokenDetecting::new();
        bad_token_detector.expect_detect().returning(|token| {
            if token == BAD_TOKEN {
                Ok(TokenQuality::Bad {
                    reason: "Token not supported".into(),
                })
            } else {
                Ok(TokenQuality::Good)
            }
        });

        let native_token = H160::from_low_u64_le(42);

        let queries = [
            // This is the common case (Tokens are supported, distinct and not ETH).
            // Will be estimated by the wrapped_estimator.
            (
                Query {
                    verification: Default::default(),
                    sell_token: H160::from_low_u64_le(1),
                    buy_token: H160::from_low_u64_le(2),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    gas: 100,
                    solver: Default::default(),
                    verified: false,
                    execution: Default::default(),
                }),
            ),
            // `sanitized_estimator` will replace `buy_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of unwrapping ETH to Estimate.
            (
                Query {
                    verification: Default::default(),
                    sell_token: H160::from_low_u64_le(1),
                    buy_token: BUY_ETH_ADDRESS,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    //sanitized_estimator will add ETH_UNWRAP_COST to the gas of any
                    //Query with ETH as the buy_token.
                    gas: GAS_PER_WETH_UNWRAP + 100,
                    solver: Default::default(),
                    verified: false,
                    execution: Default::default(),
                }),
            ),
            // Will cause buffer overflow of gas price in `sanitized_estimator`.
            (
                Query {
                    verification: Default::default(),
                    sell_token: H160::from_low_u64_le(1),
                    buy_token: BUY_ETH_ADDRESS,
                    in_amount: NonZeroU256::try_from(U256::MAX).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
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
                    buy_token: H160::from_low_u64_le(1),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    //sanitized_estimator will add ETH_WRAP_COST to the gas of any
                    //Query with ETH as the sell_token.
                    gas: GAS_PER_WETH_WRAP + 100,
                    solver: Default::default(),
                    verified: false,
                    execution: Default::default(),
                }),
            ),
            // Can be estimated by `sanitized_estimator` because `buy_token` and `sell_token` are
            // identical.
            (
                Query {
                    verification: Default::default(),
                    sell_token: H160::from_low_u64_le(1),
                    buy_token: H160::from_low_u64_le(1),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: false,
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    gas: 0,
                    solver: Default::default(),
                    verified: true,
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
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    gas: 0,
                    solver: Default::default(),
                    verified: true,
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
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    // Sanitized estimator will report a 1:1 estimate when unwrapping native token.
                    gas: GAS_PER_WETH_UNWRAP,
                    solver: Default::default(),
                    verified: true,
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
                },
                Ok(Estimate {
                    out_amount: 1.into(),
                    // Sanitized estimator will report a 1:1 estimate when wrapping native token.
                    gas: GAS_PER_WETH_WRAP,
                    solver: Default::default(),
                    verified: true,
                    execution: Default::default(),
                }),
            ),
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            (
                Query {
                    verification: Default::default(),
                    sell_token: BAD_TOKEN,
                    buy_token: H160::from_low_u64_le(1),
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
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
                    sell_token: H160::from_low_u64_le(1),
                    buy_token: BAD_TOKEN,
                    in_amount: NonZeroU256::try_from(1).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: false,
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
                        out_amount: 1.into(),
                        gas: 100,
                        solver: Default::default(),
                        verified: false,
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
                        out_amount: 1.into(),
                        gas: 100,
                        solver: Default::default(),
                        verified: false,
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
                        out_amount: 1.into(),
                        gas: u64::MAX,
                        solver: Default::default(),
                        verified: false,
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
                        out_amount: 1.into(),
                        gas: 100,
                        solver: Default::default(),
                        verified: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });

        let sanitized_estimator = SanitizedPriceEstimator {
            inner: Arc::new(wrapped_estimator),
            bad_token_detector: Arc::new(bad_token_detector),
            native_token,
        };

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
    }
}
