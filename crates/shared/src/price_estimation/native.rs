use {
    crate::price_estimation::{PriceEstimating, PriceEstimationError, Query},
    async_trait::async_trait,
    model::order::OrderKind,
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    std::sync::Arc,
};

mod coingecko;
mod oneinch;

pub use self::{coingecko::CoinGecko, oneinch::OneInch};

pub type NativePrice = f64;
pub type NativePriceEstimateResult = Result<NativePrice, PriceEstimationError>;

pub fn default_amount_to_estimate_native_prices_with(chain_id: u64) -> Option<U256> {
    match chain_id {
        // Mainnet, Göŕli, Sepolia, Arbitrum
        1 | 5 | 11155111 | 42161 => Some(10u128.pow(18).into()),
        // Gnosis chain
        100 => Some(10u128.pow(21).into()),
        _ => None,
    }
}

#[mockall::automock]
#[async_trait]
pub trait NativePriceEstimating: Send + Sync {
    /// Like `PriceEstimating::estimate`.
    ///
    /// Prices are denominated in native token (i.e. the amount of native token
    /// that is needed to buy 1 unit of the specified token).
    async fn estimate_native_price(&self, token: H160) -> NativePriceEstimateResult;
}

/// Wrapper around price estimators specialized to estimate a token's price
/// compared to the current chain's native token.
pub struct NativePriceEstimator {
    inner: Arc<dyn PriceEstimating>,
    native_token: H160,
    price_estimation_amount: NonZeroU256,
}

impl NativePriceEstimator {
    pub fn new(
        inner: Arc<dyn PriceEstimating>,
        native_token: H160,
        price_estimation_amount: NonZeroU256,
    ) -> Self {
        Self {
            inner,
            native_token,
            price_estimation_amount,
        }
    }

    fn query(&self, token: &H160) -> Query {
        Query {
            sell_token: *token,
            buy_token: self.native_token,
            in_amount: self.price_estimation_amount,
            kind: OrderKind::Buy,
            verification: Default::default(),
            block_dependent: false,
        }
    }
}

#[async_trait]
impl NativePriceEstimating for NativePriceEstimator {
    async fn estimate_native_price(&self, token: H160) -> NativePriceEstimateResult {
        let query = Arc::new(self.query(&token));
        let estimate = self.inner.estimate(query.clone()).await?;
        Ok(estimate.price_in_buy_token_f64(&query))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{Estimate, MockPriceEstimating},
        primitive_types::H160,
    };

    #[tokio::test]
    async fn prices_dont_get_modified() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimate().times(1).returning(|query| {
            assert!(query.buy_token.to_low_u64_be() == 7);
            assert!(query.sell_token.to_low_u64_be() == 3);
            async {
                Ok(Estimate {
                    out_amount: 123_456_789_000_000_000u128.into(),
                    gas: 0,
                    solver: H160([1; 20]),
                    verified: false,
                })
            }
            .boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: NonZeroU256::try_from(U256::exp10(18)).unwrap(),
        };

        let result = native_price_estimator
            .estimate_native_price(H160::from_low_u64_be(3))
            .await;
        assert_eq!(result.unwrap(), 1. / 0.123456789);
    }

    #[tokio::test]
    async fn errors_get_propagated() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimate().times(1).returning(|query| {
            assert!(query.buy_token.to_low_u64_be() == 7);
            assert!(query.sell_token.to_low_u64_be() == 2);
            async { Err(PriceEstimationError::NoLiquidity) }.boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: NonZeroU256::try_from(U256::exp10(18)).unwrap(),
        };

        let result = native_price_estimator
            .estimate_native_price(H160::from_low_u64_be(2))
            .await;
        assert!(matches!(result, Err(PriceEstimationError::NoLiquidity)));
    }
}
