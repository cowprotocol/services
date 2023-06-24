use {
    crate::{core, core::eth},
    futures::{future::BoxFuture, FutureExt},
    std::sync::Arc,
    url::Url,
};

/// An estimator which simply delegates to the legacy code.
pub struct Estimator {
    inner: Box<dyn shared::price_estimation::PriceEstimating>,
    /// The name of the legacy estimator, used for debugging purposes.
    name: &'static str,
}

impl core::Estimator for Estimator {
    fn estimate(
        &self,
        swap: core::Swap,
    ) -> BoxFuture<'_, Result<(core::Price, eth::Gas), core::EstimatorError>> {
        async move {
            shared::price_estimation::single_estimate(
                self.inner.as_ref(),
                &shared::price_estimation::Query {
                    sell_token: swap.from.into(),
                    buy_token: swap.to.into(),
                    in_amount: swap.amount.into(),
                    kind: model::order::OrderKind::Sell,
                    verification: None,
                },
            )
            .await
            .map(|estimate| {
                (
                    core::Price::new(swap.amount, estimate.out_amount.into()),
                    eth::U256::from(estimate.gas).into(),
                )
            })
            .map_err(core::EstimatorError::new)
        }
        .boxed()
    }
}

impl std::fmt::Debug for Estimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Estimator")
            .field("name", &self.name)
            .finish()
    }
}

#[derive(Debug)]
pub struct Zeroex {
    pub api_key: Option<String>,
    pub endpoint: Option<Url>,
    pub timeout: std::time::Duration,
}

impl Zeroex {
    pub fn estimator(self) -> Estimator {
        Estimator {
            inner: Box::new(shared::price_estimation::zeroex::ZeroExPriceEstimator::new(
                Arc::new(
                    shared::zeroex_api::DefaultZeroExApi::new(
                        &shared::http_client::HttpClientFactory::new(
                            &shared::http_client::Arguments {
                                http_timeout: self.timeout,
                            },
                        ),
                        self.endpoint.unwrap_or_else(|| {
                            shared::zeroex_api::DefaultZeroExApi::DEFAULT_URL
                                .parse()
                                .unwrap()
                        }),
                        self.api_key,
                    )
                    .unwrap(),
                ),
                vec![],
                Arc::new(shared::rate_limiter::RateLimiter {
                    strategy: Default::default(),
                    name: "zeroex".to_owned(),
                }),
                false,
            )),
            name: "zeroex",
        }
    }
}
