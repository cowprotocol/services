use super::{Estimate, PriceEstimating, PriceEstimationError, Query};
use crate::bad_token::{BadTokenDetecting, TokenQuality};
use crate::price_estimation::gas::GAS_PER_WETH_UNWRAP;
use anyhow::Result;
use model::order::BUY_ETH_ADDRESS;
use primitive_types::H160;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Verifies that buy and sell tokens are supported and handles
/// ETH as buy token appropriately.
pub struct SanitizedPriceEstimator {
    inner: Box<dyn PriceEstimating>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    native_token: H160,
}

type EstimationResult = Result<Estimate, PriceEstimationError>;

enum EstimationProgress<'a> {
    TrivialSolution(EstimationResult),
    AwaitingEthEstimation(&'a Query),
    AwaitingErc20Estimation,
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
        queries: &[Query],
    ) -> HashMap<H160, PriceEstimationError> {
        let mut token_quality_errors: HashMap<H160, PriceEstimationError> = Default::default();
        let mut checked_tokens = HashSet::<H160>::default();

        // TODO should this be parallelised?
        for token in queries
            .iter()
            .copied()
            .flat_map(|query| [query.buy_token, query.sell_token])
        {
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
}

#[async_trait::async_trait]
impl PriceEstimating for SanitizedPriceEstimator {
    // This function will estimate easy queries on its own and forward "difficult" queries to the
    // inner estimator. When the inner estimator did its job, solutions get merged back together
    // while preserving the correct order.
    //
    // TQ: Trivial Query, DQ: Difficult Query, P: Placeholder, TE: Trivial Estimate, DE: Difficult Estimate
    // numbers are the index within the original slice of queries
    // A => B: Code which turns A into B
    //
    // 1) Solve trivial queries and split off difficult ones.
    // [TQ0, DQ1, DQ2, TQ3, DQ4] =>
    // [TE0, P,   P,   TE3, P  ] and [DQ1, DQ2, DQ4]
    //
    // 2) Let inner estimator estimate difficult queries.
    // [DQ1, DQ2, DQ4] => [DE1, DE2, DE4]
    //
    // 3) Fill placeholders by merging difficult estimations back into all estimates.
    // [TE0, P,   P,   TE3, P  ] + [DE1, DE2, DE4] =>
    // [TE0, DE1, DE2, TE3, DE4]
    async fn estimates(&self, queries: &[Query]) -> Vec<EstimationResult> {
        use EstimationProgress::*;

        let token_quality_errors = self.get_token_quality_errors(queries).await;

        let mut difficult_queries = Vec::new();

        // If we don't collect here the borrow checker starts yelling :(
        #[allow(clippy::needless_collect)]
        // 1) Solve trivial queries and split off difficult ones.
        let all_estimates = queries
            .iter()
            .map(|query| {
                if query.sell_token == BUY_ETH_ADDRESS {
                    return TrivialSolution(Err(PriceEstimationError::NoLiquidity));
                }
                if let Some(err) = token_quality_errors.get(&query.buy_token) {
                    return TrivialSolution(Err(err.clone()));
                }
                if let Some(err) = token_quality_errors.get(&query.sell_token) {
                    return TrivialSolution(Err(err.clone()));
                }

                if query.buy_token == query.sell_token {
                    let estimation = Estimate {
                        out_amount: query.in_amount,
                        gas: 0.into(),
                    };

                    tracing::debug!(?query, ?estimation, "generate trivial price estimation");
                    return TrivialSolution(Ok(estimation));
                }

                if query.buy_token == BUY_ETH_ADDRESS {
                    let sanitized_query = Query {
                        buy_token: self.native_token,
                        ..*query
                    };

                    tracing::debug!(
                        ?query,
                        ?sanitized_query,
                        "estimate price for wrapped native asset"
                    );

                    difficult_queries.push(sanitized_query);
                    return AwaitingEthEstimation(query);
                }

                difficult_queries.push(*query);
                AwaitingErc20Estimation
            })
            .collect::<Vec<_>>();

        // 2) Let inner estimator estimate difficult queries.
        let mut difficult_estimates = self
            .inner
            .estimates(&difficult_queries[..])
            .await
            .into_iter();

        // 3) Fill placeholders by merging difficult estimations back into the result.
        let merged_results = all_estimates
            .into_iter()
            .map(|progress| match progress {
                TrivialSolution(res) => res,
                AwaitingErc20Estimation => difficult_estimates
                    .next()
                    .expect("there is a result for every forwarded query"),
                AwaitingEthEstimation(query) => {
                    let mut final_estimation = difficult_estimates
                        .next()
                        .expect("there is a result for every forwarded query")?;
                    final_estimation.gas = final_estimation
                        .gas
                        .checked_add(GAS_PER_WETH_UNWRAP.into())
                        .ok_or(anyhow::anyhow!(
                            "cost of unwrapping ETH would overflow gas price"
                        ))?;
                    tracing::debug!(
                        ?query,
                        ?final_estimation,
                        "added cost of unwrapping WETH to price estimation"
                    );
                    Ok(final_estimation)
                }
            })
            .collect();

        // All results of difficult queries have been merged.
        debug_assert!(difficult_estimates.next().is_none());

        merged_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::{MockBadTokenDetecting, TokenQuality};
    use crate::price_estimation::MockPriceEstimating;
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

        let native_token = H160::from_low_u64_le(1);

        let queries = [
            // This is the common case (Tokens are supported, distinct and not ETH).
            // Will be estimated by the wrapped_estimator.
            Query {
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(2),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // `sanitized_estimator` will replace `buy_token` with `native_token` before querying
            // `wrapped_estimator`.
            // `sanitized_estimator` will add cost of unwrapping ETH to Estimate.
            Query {
                sell_token: H160::from_low_u64_le(1),
                buy_token: BUY_ETH_ADDRESS,
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // Will cause buffer overflow of gas price in `sanitized_estimator`.
            Query {
                sell_token: H160::from_low_u64_le(1),
                buy_token: BUY_ETH_ADDRESS,
                in_amount: U256::MAX,
                kind: OrderKind::Buy,
            },
            // Can be estimated by `sanitized_estimator` because `buy_token` and `sell_token` are identical.
            Query {
                sell_token: H160::from_low_u64_le(1),
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Sell,
            },
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            Query {
                sell_token: BAD_TOKEN,
                buy_token: H160::from_low_u64_le(1),
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // Will throw `UnsupportedToken` error in `sanitized_estimator`.
            Query {
                sell_token: H160::from_low_u64_le(1),
                buy_token: BAD_TOKEN,
                in_amount: 1.into(),
                kind: OrderKind::Buy,
            },
            // Will throw `NoLiquidity` error in `sanitized_estimator`.
            Query {
                sell_token: BUY_ETH_ADDRESS,
                buy_token: H160::from_low_u64_le(1),
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
        ];

        let mut wrapped_estimator = Box::new(MockPriceEstimating::new());
        wrapped_estimator
            .expect_estimates()
            .times(1)
            .withf(move |arg: &[Query]| arg.iter().eq(expected_forwarded_queries.iter()))
            .returning(|_| {
                Box::pin(futures::future::ready(vec![
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100.into(),
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: 100.into(),
                    }),
                    Ok(Estimate {
                        out_amount: 1.into(),
                        gas: U256::MAX,
                    }),
                ]))
            });

        let sanitized_estimator = SanitizedPriceEstimator {
            inner: wrapped_estimator,
            bad_token_detector: Arc::new(bad_token_detector),
            native_token,
        };

        let result = sanitized_estimator.estimates(&queries).await;
        assert_eq!(result.len(), 7);
        assert_eq!(
            result[0].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 100.into()
            }
        );
        assert_eq!(
            result[1].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                //sanitized_estimator will add ETH_UNWRAP_COST to the gas of any
                //Query with ETH as the buy_token.
                gas: U256::from(GAS_PER_WETH_UNWRAP)
                    .checked_add(100.into())
                    .unwrap()
            }
        );
        assert!(matches!(
            result[2].as_ref().unwrap_err(),
            PriceEstimationError::Other(err)
                if err.to_string() == "cost of unwrapping ETH would overflow gas price",
        ));
        assert_eq!(
            result[3].as_ref().unwrap(),
            &Estimate {
                out_amount: 1.into(),
                gas: 0.into()
            }
        );
        assert!(matches!(
            result[4].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken(_)
        ));
        assert!(matches!(
            result[5].as_ref().unwrap_err(),
            PriceEstimationError::UnsupportedToken(_)
        ));
        assert!(matches!(
            result[6].as_ref().unwrap_err(),
            PriceEstimationError::NoLiquidity
        ));
    }
}
