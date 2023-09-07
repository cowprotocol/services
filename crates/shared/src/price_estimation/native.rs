use {
    crate::price_estimation::{PriceEstimating, PriceEstimationError, Query},
    futures::{FutureExt, StreamExt},
    model::order::OrderKind,
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    std::sync::Arc,
};

pub type NativePriceEstimateResult = Result<f64, PriceEstimationError>;

pub fn default_amount_to_estimate_native_prices_with(chain_id: u64) -> Option<U256> {
    match chain_id {
        // Mainnet, Göŕli
        1 | 5 => Some(10u128.pow(18).into()),
        // Gnosis chain
        100 => Some(10u128.pow(21).into()),
        _ => None,
    }
}

#[mockall::automock]
pub trait NativePriceEstimating: Send + Sync {
    /// Like `PriceEstimating::estimate`.
    ///
    /// Prices are denominated in native token (i.e. the amount of native token
    /// that is needed to buy 1 unit of the specified token).
    fn estimate_native_price<'a>(
        &'a self,
        token: &'a H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult>;

    /// Estimates multiple queries. It can be configured to execute queries
    /// in a buffered manner. If `parallelism` is `0` or `1` queries are
    /// executed sequentially. A number greater than `1` will result in that
    /// many parallel requests.
    fn estimate_all<'a>(
        &'a self,
        tokens: &'a [H160],
        parallelism: usize,
    ) -> futures::future::BoxFuture<'_, Vec<NativePriceEstimateResult>> {
        async move {
            let mut results: Vec<_> = self.estimate_streaming(tokens, parallelism).collect().await;
            results.sort_by_key(|(index, _)| *index);
            results.into_iter().map(|(_, result)| result).collect()
        }
        .boxed()
    }

    /// Estimates multiple queries in a streaming manner. It can be configured
    /// to execute queries in a buffered manner. If `parallelism` is `0` or
    /// `1` queries are executed sequentially. A number greater than `1`
    /// will result in that many parallel requests.
    fn estimate_streaming<'a>(
        &'a self,
        tokens: &'a [H160],
        parallelism: usize,
    ) -> futures::stream::BoxStream<'_, (usize, NativePriceEstimateResult)> {
        match parallelism {
            0 | 1 => futures::stream::iter(tokens.iter().enumerate())
                .then(move |(index, token)| async move {
                    (index, self.estimate_native_price(token).await)
                })
                .boxed(),
            parallelism => {
                futures::stream::iter(tokens.iter().enumerate().map(
                    move |(index, token)| async move {
                        (index, self.estimate_native_price(token).await)
                    },
                ))
                .buffer_unordered(parallelism)
                .boxed()
            }
        }
    }
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
            verification: None,
        }
    }
}

#[async_trait::async_trait]
impl NativePriceEstimating for NativePriceEstimator {
    fn estimate_native_price<'a>(
        &'a self,
        token: &'a H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async {
            let query = self.query(token);
            let estimate = self.inner.estimate(&query).await?;
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
        futures::{FutureExt, StreamExt},
        primitive_types::H160,
    };

    #[test]
    fn prices_dont_get_modified() {
        let mut inner = MockPriceEstimating::new();
        inner.estimate().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].buy_token.to_low_u64_be() == 7);
            assert!(queries[0].sell_token.to_low_u64_be() == 3);
            futures::stream::iter([Ok(Estimate {
                out_amount: 123_456_789_000_000_000u128.into(),
                gas: 0,
                solver: H160([1; 20]),
            })])
            .enumerate()
            .boxed()
        });

        let native_price_estimator = NativePriceEstimator {
            inner: Arc::new(inner),
            native_token: H160::from_low_u64_be(7),
            price_estimation_amount: NonZeroU256::try_from(U256::exp10(18)).unwrap(),
        };

        let result = native_price_estimator
            .estimate_native_price(&[H160::from_low_u64_be(3)])
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
        inner.estimate().times(1).returning(|queries| {
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
            price_estimation_amount: NonZeroU256::try_from(U256::exp10(18)).unwrap(),
        };

        let result = native_price_estimator
            .estimate_native_price(&[H160::from_low_u64_be(2)])
            .next()
            .now_or_never()
            .unwrap()
            .unwrap()
            .1;
        assert!(matches!(result, Err(PriceEstimationError::NoLiquidity)));
    }
}
