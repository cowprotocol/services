use crate::{
    bad_token::{BadTokenDetecting, TokenQuality},
    price_estimation::{
        gas::{GAS_PER_WETH_UNWRAP, GAS_PER_WETH_WRAP},
        Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
    },
};
use anyhow::anyhow;
use futures::StreamExt;
use model::order::BUY_ETH_ADDRESS;
use primitive_types::H160;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
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

    async fn get_token_quality_errors(
        &self,
        queries: impl Iterator<Item = &Query>,
    ) -> HashMap<H160, PriceEstimationError> {
        let mut token_quality_errors: HashMap<H160, PriceEstimationError> = Default::default();
        let mut checked_tokens = HashSet::<H160>::default();

        // TODO should this be parallelised?
        for token in queries.flat_map(|query| [query.buy_token, query.sell_token]) {
            if checked_tokens.contains(&token) {
                continue;
            }

            match self.bad_token_detector.detect(token).await {
                Err(err) => {
                    token_quality_errors.insert(token, PriceEstimationError::Other(err));
                }
                Ok(TokenQuality::Bad { .. }) => {
                    token_quality_errors
                        .insert(token, PriceEstimationError::UnsupportedToken(token));
                }
                _ => (),
            };
            checked_tokens.insert(token);
        }
        token_quality_errors
    }

    /// Removes easy queries from the input and returns their estimates.
    async fn estimate_easy_queries(
        &self,
        queries: &mut Vec<(usize, Query)>,
    ) -> Vec<(usize, PriceEstimateResult)> {
        let token_quality_errors = self
            .get_token_quality_errors(queries.iter().map(|(_, query)| query))
            .await;
        let mut results = Vec::new();
        queries.retain(|(index, query)| {
            for token in [&query.buy_token, &query.sell_token] {
                if let Some(err) = token_quality_errors.get(token) {
                    results.push((*index, Err(err.clone())));
                    return false;
                }
            }

            if query.buy_token == query.sell_token {
                let estimation = Estimate {
                    out_amount: query.in_amount,
                    gas: 0,
                };
                tracing::debug!(?query, ?estimation, "generate trivial price estimation");
                results.push((*index, Ok(estimation)));
                return false;
            }

            if query.sell_token == self.native_token && query.buy_token == BUY_ETH_ADDRESS {
                let estimation = Estimate {
                    out_amount: query.in_amount,
                    gas: GAS_PER_WETH_UNWRAP,
                };
                tracing::debug!(?query, ?estimation, "generate trivial unwrap estimation");
                results.push((*index, Ok(estimation)));
                return false;
            }

            if query.sell_token == BUY_ETH_ADDRESS && query.buy_token == self.native_token {
                let estimation = Estimate {
                    out_amount: query.in_amount,
                    gas: GAS_PER_WETH_WRAP,
                };
                tracing::debug!(?query, ?estimation, "generate trivial wrap estimation");
                results.push((*index, Ok(estimation)));
                return false;
            }

            true
        });

        results
    }
}

impl PriceEstimating for SanitizedPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, super::PriceEstimateResult)> {
        let stream = async_stream::stream! {
            // Handle easy estimates first.
            let mut queries: Vec<(usize, Query)> = queries.iter().copied().enumerate().collect();
            for easy in self.estimate_easy_queries(&mut queries).await {
                yield easy;
            }

            // The remaining queries are difficult and need to be forwarded to the inner estimator. Some
            // of the queries need to be changed to handle BUY_ETH_ADDRESS.

            struct DifficultQuery {
                original_query_index: usize,
                query: Query,
                modification: Option<Modification>,
            }

            enum Modification {
                AddGas(u64),
            }

            let difficult_queries: Vec<DifficultQuery> = queries
                .into_iter()
                .map(|(index, mut query)| {
                    let modification = if query.sell_token != self.native_token
                        && query.buy_token == BUY_ETH_ADDRESS
                    {
                        tracing::debug!(?query, "estimate price for buying native asset");
                        query.buy_token = self.native_token;
                        Some(Modification::AddGas(GAS_PER_WETH_UNWRAP))
                    } else if query.sell_token == BUY_ETH_ADDRESS
                        && query.buy_token != self.native_token
                    {
                        tracing::debug!(?query, "estimate price for selling native asset");
                        query.sell_token = self.native_token;
                        Some(Modification::AddGas(GAS_PER_WETH_WRAP))
                    } else {
                        None
                    };
                    DifficultQuery {
                        original_query_index: index,
                        query,
                        modification,
                    }
                })
                .collect();

            let inner_queries: Vec<Query> =
                difficult_queries.iter().map(|query| query.query).collect();
            let mut stream = self.inner.estimates(&inner_queries);

            while let Some((i, mut estimate)) = stream.next().await {
                let query = &difficult_queries[i];
                if let Some(Modification::AddGas(gas)) = query.modification {
                    if let Ok(estimate) = &mut estimate {
                        estimate.gas = match estimate.gas.checked_add(gas) {
                            Some(gas) => gas,
                            None => {
                                let err = PriceEstimationError::Other(anyhow!(
                                    "cost of converting native asset would overflow gas price"
                                ));
                                yield (query.original_query_index, Err(err));
                                continue;
                            }
                        };
                        tracing::debug!(
                            query = ?query.query,
                            ?estimate,
                            "added cost of converting native asset to price estimation"
                        );
                    }
                }
                yield (query.original_query_index, estimate);
            }
        };
        stream.boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::{MockBadTokenDetecting, TokenQuality};
    use crate::price_estimation::{vec_estimates, MockPriceEstimating};
    use futures::StreamExt;
    use model::order::OrderKind;
    use primitive_types::{H160, U256};

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
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(2),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // `sanitized_estimator` will replace `buy_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of unwrapping ETH to Estimate.
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: BUY_ETH_ADDRESS,
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // Will cause buffer overflow of gas price in `sanitized_estimator`.
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: BUY_ETH_ADDRESS,
                in_amount: U256::MAX,
                kind: OrderKind::Buy,
            },
            // `sanitized_estimator` will replace `sell_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of wrapping ETH to Estimate.
            Query {
                from: None,
                sell_token: BUY_ETH_ADDRESS,
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // Can be estimated by `sanitized_estimator` because `buy_token` and `sell_token` are identical.
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Sell,
            },
            // Can be estimated by `sanitized_estimator` because both tokens are the native token.
            Query {
                from: None,
                sell_token: BUY_ETH_ADDRESS,
                buy_token: BUY_ETH_ADDRESS,
                in_amount: 1.into(),
                kind: OrderKind::Sell,
            },
            // Can be estimated by `sanitized_estimator` because it is a native token unwrap.
            Query {
                from: None,
                sell_token: native_token,
                buy_token: BUY_ETH_ADDRESS,
                in_amount: 1.into(),
                kind: OrderKind::Sell,
            },
            // Can be estimated by `sanitized_estimator` because it is a native token wrap.
            Query {
                from: None,
                sell_token: BUY_ETH_ADDRESS,
                buy_token: native_token,
                in_amount: 1.into(),
                kind: OrderKind::Sell,
            },
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            Query {
                from: None,
                sell_token: BAD_TOKEN,
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: BAD_TOKEN,
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
        ];

        let expected_forwarded_queries = [
            // SanitizedPriceEstimator will simply forward the Query in the common case
            queries[0],
            Query {
                // SanitizedPriceEstimator replaces ETH buy token with native token
                buy_token: native_token,
                ..queries[1]
            },
            Query {
                // SanitizedPriceEstimator replaces ETH buy token with native token
                buy_token: native_token,
                ..queries[2]
            },
            Query {
                // SanitizedPriceEstimator replaces ETH sell token with native token
                sell_token: native_token,
                ..queries[3]
            },
        ];

        let mut wrapped_estimator = Box::new(MockPriceEstimating::new());
        wrapped_estimator
            .expect_estimates()
            .times(1)
            .withf(move |arg: &[Query]| arg.iter().eq(expected_forwarded_queries.iter()))
            .returning(|_| {
                futures::stream::iter([
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100,
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100,
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: u64::MAX,
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100,
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

        let result = vec_estimates(&sanitized_estimator, &queries).await;
        assert_eq!(result.len(), 10);
        assert_eq!(
            result[0].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 100
            }
        );
        assert_eq!(
            result[1].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                //sanitized_estimator will add ETH_UNWRAP_COST to the gas of any
                //Query with ETH as the buy_token.
                gas: GAS_PER_WETH_UNWRAP + 100,
            }
        );
        assert!(matches!(
            result[2].as_ref().unwrap_err(),
            PriceEstimationError::Other(err)
                if err.to_string() == "cost of converting native asset would overflow gas price",
        ));
        assert_eq!(
            result[3].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                //sanitized_estimator will add ETH_WRAP_COST to the gas of any
                //Query with ETH as the sell_token.
                gas: GAS_PER_WETH_WRAP + 100,
            }
        );
        assert_eq!(
            result[4].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 0,
            }
        );
        assert_eq!(
            result[5].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 0,
            }
        );
        assert_eq!(
            result[6].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                // Sanitized estimator will report a 1:1 estimate when unwrapping native token.
                gas: GAS_PER_WETH_UNWRAP,
            }
        );
        assert_eq!(
            result[7].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                // Sanitized estimator will report a 1:1 estimate when wrapping native token.
                gas: GAS_PER_WETH_WRAP,
            }
        );
        assert!(matches!(
            result[8].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken(_)
        ));
        assert!(matches!(
            result[9].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken(_)
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
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(2),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            //easy
            Query {
                from: None,
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
        ];

        let expected_forwarded_queries = [queries[0]];

        let mut wrapped_estimator = Box::new(MockPriceEstimating::new());
        wrapped_estimator
            .expect_estimates()
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
        let mut stream = sanitized_estimator.estimates(&queries);

        let (index, result) = stream.next().await.unwrap();
        assert_eq!(index, 1);
        assert!(result.is_ok());

        let (index, result) = stream.next().await.unwrap();
        assert_eq!(index, 0);
        assert!(result.is_err());

        assert!(stream.next().await.is_none());
    }
}
