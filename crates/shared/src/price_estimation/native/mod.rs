use {
    crate::price_estimation::{PriceEstimating, PriceEstimationError, Query},
    alloy::primitives::Address,
    bigdecimal::{BigDecimal, ToPrimitive},
    futures::FutureExt,
    model::order::OrderKind,
    number::nonzero::NonZeroU256,
    std::{
        sync::{Arc, LazyLock},
        time::Duration,
    },
    tracing::instrument,
};

mod coingecko;
pub mod fallback;
mod forwarder;
mod oneinch;

pub use self::{
    coingecko::CoinGecko,
    fallback::FallbackNativePriceEstimator,
    forwarder::Forwarder,
    oneinch::OneInch,
};

pub type NativePrice = f64;
pub type NativePriceEstimateResult = Result<NativePrice, PriceEstimationError>;

/// Convert from normalized price to floating point price
pub fn from_normalized_price(price: BigDecimal) -> Option<f64> {
    static ONE_E18: LazyLock<BigDecimal> = LazyLock::new(|| BigDecimal::try_from(1e18).unwrap());

    // Divide by 1e18 to reverse the multiplication by 1e18
    let normalized_price = price / ONE_E18.clone();

    // Convert U256 to f64
    let normalized_price = normalized_price.to_f64()?;

    // Ensure the price is in the normal float range
    normalized_price.is_normal().then_some(normalized_price)
}

/// Convert from floating point price to normalized price
pub fn to_normalized_price(price: f64) -> Option<alloy::primitives::U256> {
    let uint_max = 2.0_f64.powi(256);

    let price_in_eth = 1e18 * price;
    (price_in_eth.is_normal() && price_in_eth >= 1. && price_in_eth < uint_max)
        .then_some(alloy::primitives::U256::saturating_from(price_in_eth))
}

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
pub trait NativePriceEstimating: Send + Sync {
    /// Like `PriceEstimating::estimate`.
    ///
    /// Prices are denominated in native token (i.e. the amount of native token
    /// that is needed to buy 1 unit of the specified token).
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult>;
}

/// Wrapper around price estimators specialized to estimate a token's price
/// compared to the current chain's native token.
pub struct NativePriceEstimator {
    inner: Arc<dyn PriceEstimating>,
    native_token: Address,
    price_estimation_amount: NonZeroU256,
}

impl NativePriceEstimator {
    pub fn new(
        inner: Arc<dyn PriceEstimating>,
        native_token: Address,
        price_estimation_amount: NonZeroU256,
    ) -> Self {
        Self {
            inner,
            native_token,
            price_estimation_amount,
        }
    }

    // TODO explain why we use BUY order type (shallow liquidity)
    fn query(&self, token: &Address, timeout: Duration) -> Query {
        Query {
            sell_token: *token,
            buy_token: self.native_token,
            in_amount: self.price_estimation_amount,
            kind: OrderKind::Buy,
            verification: Default::default(),
            block_dependent: false,
            timeout,
        }
    }
}

impl NativePriceEstimating for NativePriceEstimator {
    #[instrument(skip_all)]
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let query = Arc::new(self.query(&token, timeout));
            let estimate = self.inner.estimate(query.clone()).await?;
            let price = estimate.price_in_buy_token_f64(&query);
            if is_price_malformed(price) {
                let err = anyhow::anyhow!("estimator returned malformed price: {price}");
                Err(PriceEstimationError::EstimatorInternal(err))
            } else {
                Ok(price)
            }
        }
        .boxed()
    }
}

pub(crate) fn is_price_malformed(price: f64) -> bool {
    !price.is_normal()
        || price <= 0.
        // To convert the f64 native price into a format usable in the auction
        // the autopilot calls `to_normalized_price()`. Orders placed using a
        // native price that fails this conversion will likely time out because
        // the autopilot will not put them into the auction. To prevent that we
        // already check the conversion here.
        || to_normalized_price(price).is_none()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{Estimate, HEALTHY_PRICE_ESTIMATION_TIME, MockPriceEstimating},
        alloy::primitives::{Address, U256},
        std::str::FromStr,
    };

    #[tokio::test]
    async fn prices_dont_get_modified() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimate().times(1).returning(|query| {
            assert!(query.buy_token == Address::with_last_byte(7));
            assert!(query.sell_token == Address::with_last_byte(3));
            async {
                Ok(Estimate {
                    out_amount: U256::from(123_456_789_000_000_000u128),
                    gas: 0,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    execution: Default::default(),
                })
            }
            .boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: Address::with_last_byte(7),
            price_estimation_amount: NonZeroU256::try_from(U256::from(10).pow(U256::from(18)))
                .unwrap(),
        };

        let result = native_price_estimator
            .estimate_native_price(Address::with_last_byte(3), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.unwrap(), 1. / 0.123456789);
    }

    #[tokio::test]
    async fn errors_get_propagated() {
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimate().times(1).returning(|query| {
            assert!(query.buy_token == Address::with_last_byte(7));
            assert!(query.sell_token == Address::with_last_byte(2));
            async { Err(PriceEstimationError::NoLiquidity) }.boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: Address::with_last_byte(7),
            price_estimation_amount: NonZeroU256::try_from(U256::from(10).pow(U256::from(18)))
                .unwrap(),
        };

        let result = native_price_estimator
            .estimate_native_price(Address::with_last_byte(2), HEALTHY_PRICE_ESTIMATION_TIME)
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
