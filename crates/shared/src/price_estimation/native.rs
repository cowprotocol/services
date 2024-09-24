use {
    crate::price_estimation::{PriceEstimating, PriceEstimationError, Query},
    bigdecimal::{BigDecimal, ToPrimitive},
    cached::once_cell::sync::Lazy,
    futures::FutureExt,
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

/// Convert from normalized price to floating point price
pub fn from_normalized_price(price: BigDecimal) -> Option<f64> {
    static ONE_E18: Lazy<BigDecimal> = Lazy::new(|| BigDecimal::try_from(1e18).unwrap());

    // Divide by 1e18 to reverse the multiplication by 1e18
    let normalized_price = price / ONE_E18.clone();

    // Convert U256 to f64
    let normalized_price = normalized_price.to_f64()?;

    // Ensure the price is in the normal float range
    normalized_price.is_normal().then_some(normalized_price)
}

/// Convert from floating point price to normalized price
pub fn to_normalized_price(price: f64) -> Option<U256> {
    let uint_max = 2.0_f64.powi(256);

    let price_in_eth = 1e18 * price;
    if price_in_eth.is_normal() && price_in_eth >= 1. && price_in_eth < uint_max {
        Some(U256::from_f64_lossy(price_in_eth))
    } else {
        None
    }
}

#[mockall::automock]
pub trait NativePriceEstimating: Send + Sync {
    /// Like `PriceEstimating::estimate`.
    ///
    /// Prices are denominated in native token (i.e. the amount of native token
    /// that is needed to buy 1 unit of the specified token).
    fn estimate_native_price(
        &self,
        token: H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult>;
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

impl NativePriceEstimating for NativePriceEstimator {
    fn estimate_native_price(
        &self,
        token: H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let query = Arc::new(self.query(&token));
            let estimate = self.inner.estimate(query.clone()).await?;
            Ok(estimate.price_in_buy_token_f64(&query))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{Estimate, MockPriceEstimating},
        primitive_types::H160,
        std::str::FromStr,
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

    #[test]
    fn computes_price_from_normalized_price() {
        assert_eq!(
            from_normalized_price(BigDecimal::from_str("500000000000000000").unwrap()).unwrap(),
            0.5
        );
    }

    #[test]
    fn computes_u256_prices_normalized_to_1e18() {
        assert_eq!(
            to_normalized_price(0.5).unwrap(),
            U256::from(500_000_000_000_000_000_u128),
        );
    }

    #[test]
    fn normalize_prices_fail_when_outside_valid_input_range() {
        assert!(to_normalized_price(0.).is_none());
        assert!(to_normalized_price(-1.).is_none());
        assert!(to_normalized_price(f64::INFINITY).is_none());

        let min_price = 1. / 1e18;
        assert!(to_normalized_price(min_price).is_some());
        assert!(to_normalized_price(min_price * (1. - f64::EPSILON)).is_none());

        let uint_max = 2.0_f64.powi(256);
        let max_price = uint_max / 1e18;
        assert!(to_normalized_price(max_price).is_none());
        assert!(to_normalized_price(max_price * (1. - f64::EPSILON)).is_some());
    }
}
