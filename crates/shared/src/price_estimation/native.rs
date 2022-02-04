use super::{PriceEstimating, PriceEstimationError, Query};
use model::order::OrderKind;
use primitive_types::{H160, U256};
use std::sync::Arc;

#[mockall::automock]
#[async_trait::async_trait]
pub trait NativePriceEstimating: Send + Sync {
    /// The resulting price is how many units of token needs to be sold for one unit of
    /// the chain's native token (sell_amount / buy_amount).
    async fn estimate_native_price(&self, token: &H160) -> Result<f64, PriceEstimationError> {
        self.estimate_native_prices(std::slice::from_ref(token))
            .await
            .into_iter()
            .next()
            .unwrap()
    }

    /// The resulting price is how many units of token needs to be sold for one unit of
    /// the chain's native token (sell_amount / buy_amount).
    /// Returns one result for each query.
    async fn estimate_native_prices(
        &self,
        tokens: &[H160],
    ) -> Vec<Result<f64, PriceEstimationError>>;
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
}

#[async_trait::async_trait]
impl NativePriceEstimating for NativePriceEstimator {
    async fn estimate_native_prices(
        &self,
        tokens: &[H160],
    ) -> Vec<Result<f64, PriceEstimationError>> {
        let native_token_queries: Vec<_> = tokens
            .iter()
            .map(|token| Query {
                sell_token: *token,
                buy_token: self.native_token,
                in_amount: self.price_estimation_amount,
                kind: OrderKind::Buy,
            })
            .collect();

        let estimates = self.inner.estimates(&native_token_queries).await;

        estimates
            .into_iter()
            .zip(native_token_queries.iter())
            .map(|(estimate, query)| {
                estimate.map(|estimate| estimate.price_in_sell_token_f64(query))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{Estimate, MockPriceEstimating};
    use futures::FutureExt;
    use primitive_types::H160;

    #[test]
    fn prices_dont_get_modified() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimates().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].buy_token.to_low_u64_be() == 7);
            assert!(queries[0].sell_token.to_low_u64_be() == 3);
            vec![Ok(Estimate {
                out_amount: 123_456_789_000_000_000u128.into(),
                gas: 0.into(),
            })]
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: U256::exp10(18),
        };

        let price = native_price_estimator
            .estimate_native_price(&H160::from_low_u64_be(3))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(price, 0.123456789);
    }

    #[test]
    fn errors_get_propagated() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimates().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].buy_token.to_low_u64_be() == 7);
            assert!(queries[0].sell_token.to_low_u64_be() == 2);
            vec![Err(PriceEstimationError::NoLiquidity)]
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: U256::exp10(18),
        };

        let price = native_price_estimator
            .estimate_native_price(&H160::from_low_u64_be(2))
            .now_or_never()
            .unwrap();
        assert!(matches!(price, Err(PriceEstimationError::NoLiquidity)));
    }
}
