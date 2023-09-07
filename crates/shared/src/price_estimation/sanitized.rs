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
    inner: Box<dyn PriceEstimating>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    native_token: H160,
}

impl SanitizedPriceEstimator {
    pub fn new(
        inner: Box<dyn PriceEstimating>,
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
    fn estimate<'a>(
        &'a self,
        query: &'a Query,
    ) -> futures::future::BoxFuture<'_, super::PriceEstimateResult> {
        async move {
            self.handle_bad_tokens(query).await?;

            // buy_token == sell_token => 1 to 1 conversion
            if query.buy_token == query.sell_token {
                let estimation = Estimate {
                    out_amount: query.in_amount.get(),
                    gas: 0,
                    solver: Default::default(),
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
                };
                tracing::debug!(?query, ?estimation, "generate trivial wrap estimation");
                return Ok(estimation);
            }

            enum Modification {
                AddGas(u64),
            }

            let mut adjusted_query = query.clone();
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

            let mut estimate = self.inner.estimate(&adjusted_query).await?;

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
        futures::StreamExt,
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
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(2),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
            // `sanitized_estimator` will replace `buy_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of unwrapping ETH to Estimate.
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: BUY_ETH_ADDRESS,
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
            // Will cause buffer overflow of gas price in `sanitized_estimator`.
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: BUY_ETH_ADDRESS,
                in_amount: NonZeroU256::try_from(U256::MAX).unwrap(),
                kind: OrderKind::Buy,
            },
            // `sanitized_estimator` will replace `sell_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of wrapping ETH to Estimate.
            Query {
                verification: None,
                sell_token: BUY_ETH_ADDRESS,
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
            // Can be estimated by `sanitized_estimator` because `buy_token` and `sell_token` are
            // identical.
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
            },
            // Can be estimated by `sanitized_estimator` because both tokens are the native token.
            Query {
                verification: None,
                sell_token: BUY_ETH_ADDRESS,
                buy_token: BUY_ETH_ADDRESS,
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
            },
            // Can be estimated by `sanitized_estimator` because it is a native token unwrap.
            Query {
                verification: None,
                sell_token: native_token,
                buy_token: BUY_ETH_ADDRESS,
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
            },
            // Can be estimated by `sanitized_estimator` because it is a native token wrap.
            Query {
                verification: None,
                sell_token: BUY_ETH_ADDRESS,
                buy_token: native_token,
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Sell,
            },
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            Query {
                verification: None,
                sell_token: BAD_TOKEN,
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: BAD_TOKEN,
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
        ];

        let expected_forwarded_queries = [
            // SanitizedPriceEstimator will simply forward the Query in the common case
            queries[0].clone(),
            Query {
                // SanitizedPriceEstimator replaces ETH buy token with native token
                buy_token: native_token,
                ..queries[1].clone()
            },
            Query {
                // SanitizedPriceEstimator replaces ETH buy token with native token
                buy_token: native_token,
                ..queries[2].clone()
            },
            Query {
                // SanitizedPriceEstimator replaces ETH sell token with native token
                sell_token: native_token,
                ..queries[3].clone()
            },
        ];

        let mut wrapped_estimator = Box::new(MockPriceEstimating::new());
        wrapped_estimator
            .estimate()
            .times(1)
            .withf(move |arg: &[Query]| arg.iter().eq(expected_forwarded_queries.iter()))
            .returning(|_| {
                futures::stream::iter([
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100,
                        solver: Default::default(),
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100,
                        solver: Default::default(),
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: u64::MAX,
                        solver: Default::default(),
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100,
                        solver: Default::default(),
                    }),
                ])
                .enumerate()
                .boxed()
            });

        let sanitized_estimator = SanitizedPriceEstimator {
            inner: wrapped_estimator,
            bad_token_detector: Arc::new(bad_token_detector),
            native_token,
        };

        let result = sanitized_estimator.estimate_all(&queries, 1).await;
        assert_eq!(result.len(), 10);
        assert_eq!(
            result[0].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 100,
                solver: Default::default(),
            }
        );
        assert_eq!(
            result[1].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                //sanitized_estimator will add ETH_UNWRAP_COST to the gas of any
                //Query with ETH as the buy_token.
                gas: GAS_PER_WETH_UNWRAP + 100,
                solver: Default::default(),
            }
        );
        assert!(matches!(
            result[2].as_ref().unwrap_err(),
            PriceEstimationError::ProtocolInternal(err)
                if err.to_string() == "cost of converting native asset would overflow gas price",
        ));
        assert_eq!(
            result[3].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                //sanitized_estimator will add ETH_WRAP_COST to the gas of any
                //Query with ETH as the sell_token.
                gas: GAS_PER_WETH_WRAP + 100,
                solver: Default::default(),
            }
        );
        assert_eq!(
            result[4].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 0,
                solver: Default::default(),
            }
        );
        assert_eq!(
            result[5].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 0,
                solver: Default::default(),
            }
        );
        assert_eq!(
            result[6].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                // Sanitized estimator will report a 1:1 estimate when unwrapping native token.
                gas: GAS_PER_WETH_UNWRAP,
                solver: Default::default(),
            }
        );
        assert_eq!(
            result[7].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                // Sanitized estimator will report a 1:1 estimate when wrapping native token.
                gas: GAS_PER_WETH_WRAP,
                solver: Default::default(),
            }
        );
        assert!(matches!(
            result[8].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken { .. }
        ));
        assert!(matches!(
            result[9].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken { .. }
        ));
    }

    #[tokio::test]
    async fn easy_queries_come_first() {
        let mut bad_token_detector = MockBadTokenDetecting::new();
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));

        let queries = [
            // difficult
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(2),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
            //easy
            Query {
                verification: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(1),
                in_amount: NonZeroU256::try_from(1).unwrap(),
                kind: OrderKind::Buy,
            },
        ];

        let expected_forwarded_queries = [queries[0].clone()];

        let mut wrapped_estimator = Box::new(MockPriceEstimating::new());
        wrapped_estimator
            .estimate()
            .times(1)
            .withf(move |arg: &[Query]| arg.iter().eq(expected_forwarded_queries.iter()))
            .returning(|_| {
                futures::stream::iter([Err(PriceEstimationError::NoLiquidity)])
                    .enumerate()
                    .boxed()
            });

        let sanitized_estimator = SanitizedPriceEstimator {
            inner: wrapped_estimator,
            bad_token_detector: Arc::new(bad_token_detector),
            native_token: H160::from_low_u64_le(42),
        };
        let mut stream = sanitized_estimator.estimate(&queries);

        let (index, result) = stream.next().await.unwrap();
        assert_eq!(index, 1);
        assert!(result.is_ok());

        let (index, result) = stream.next().await.unwrap();
        assert_eq!(index, 0);
        assert!(result.is_err());

        assert!(stream.next().await.is_none());
    }
}
