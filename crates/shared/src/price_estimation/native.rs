use crate::price_estimation::{PriceEstimating, PriceEstimationError, Query};
use futures::{stream::BoxStream, StreamExt};
use model::order::OrderKind;
use primitive_types::{H160, U256};
use std::sync::Arc;

pub type NativePriceEstimateResult = Result<f64, PriceEstimationError>;

pub fn default_amount_to_estimate_native_prices_with(chain_id: u64) -> Option<U256> {
    match chain_id {
        // Mainnet, Rinkeby, Göŕli
        1 | 4 | 5 => Some(10u128.pow(18).into()),
        // Xdai
        100 => Some(10u128.pow(21).into()),
        _ => None,
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait NativePriceEstimating: Send + Sync {
    /// Like `PriceEstimating::estimates`.
    ///
    /// Prices are denominated in native token (i.e. the amount of native token
    /// that is needed to buy 1 unit of the specified token).
    fn estimate_native_prices<'a>(
        &'a self,
        tokens: &'a [H160],
    ) -> BoxStream<'_, (usize, NativePriceEstimateResult)>;
}

/// Wrapper around price estimators specialized to estimate a token's price compared to the current
/// chain's native token.
pub struct NativePriceEstimator {
    inner: Arc<dyn PriceEstimating>,
    native_token: H160,
    price_estimation_amount: U256,
}

impl NativePriceEstimator {
    pub fn new(
        inner: Arc<dyn PriceEstimating>,
        native_token: H160,
        price_estimation_amount: U256,
    ) -> Self {
        Self {
            inner,
            native_token,
            price_estimation_amount,
        }
    }

    fn query(&self, token: &H160) -> Query {
        Query {
            from: None,
            sell_token: *token,
            buy_token: self.native_token,
            in_amount: self.price_estimation_amount,
            kind: OrderKind::Buy,
        }
    }
}

#[async_trait::async_trait]
impl NativePriceEstimating for NativePriceEstimator {
    fn estimate_native_prices<'a>(
        &'a self,
        tokens: &'a [H160],
    ) -> BoxStream<'_, (usize, NativePriceEstimateResult)> {
        let stream = async_stream::stream!({
            let queries: Vec<_> = tokens.iter().map(|token| self.query(token)).collect();
            let mut inner = self.inner.estimates(&queries);
            while let Some((i, result)) = inner.next().await {
                let result = result.map(|estimate| estimate.price_in_buy_token_f64(&queries[i]));
                yield (i, result)
            }
        });
        stream.boxed()
    }
}

pub async fn native_single_estimate(
    estimator: &dyn NativePriceEstimating,
    token: &H160,
) -> NativePriceEstimateResult {
    estimator
        .estimate_native_prices(std::slice::from_ref(token))
        .next()
        .await
        .unwrap()
        .1
}

pub async fn native_vec_estimates(
    estimator: &dyn NativePriceEstimating,
    queries: &[H160],
) -> Vec<NativePriceEstimateResult> {
    let mut results = vec![None; queries.len()];
    let mut stream = estimator.estimate_native_prices(queries);
    while let Some((index, result)) = stream.next().await {
        results[index] = Some(result);
    }
    let results = results.into_iter().flatten().collect::<Vec<_>>();
    // Check that every query has a result.
    debug_assert_eq!(results.len(), queries.len());
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{Estimate, MockPriceEstimating};
    use futures::{FutureExt, StreamExt};
    use primitive_types::H160;

    #[test]
    fn prices_dont_get_modified() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimates().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].buy_token.to_low_u64_be() == 7);
            assert!(queries[0].sell_token.to_low_u64_be() == 3);
            futures::stream::iter([Ok(Estimate {
                out_amount: 123_456_789_000_000_000u128.into(),
                gas: 0,
            })])
            .enumerate()
            .boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: U256::exp10(18),
        };

        let result = native_price_estimator
            .estimate_native_prices(&[H160::from_low_u64_be(3)])
            .next()
            .now_or_never()
            .unwrap()
            .unwrap()
            .1;
        assert_eq!(result.unwrap(), 1. / 0.123456789);
    }

    #[test]
    fn errors_get_propagated() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimates().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].buy_token.to_low_u64_be() == 7);
            assert!(queries[0].sell_token.to_low_u64_be() == 2);
            futures::stream::iter([Err(PriceEstimationError::NoLiquidity)])
                .enumerate()
                .boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: U256::exp10(18),
        };

        let result = native_price_estimator
            .estimate_native_prices(&[H160::from_low_u64_be(2)])
            .next()
            .now_or_never()
            .unwrap()
            .unwrap()
            .1;
        assert!(matches!(result, Err(PriceEstimationError::NoLiquidity)));
    }
}
