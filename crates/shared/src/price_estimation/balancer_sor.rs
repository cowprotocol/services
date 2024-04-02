use {
    super::{
        gas::{GAS_PER_BALANCER_SWAP, SETTLEMENT_SINGLE_TRADE},
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    crate::{
        balancer_sor_api::{self, BalancerSorApi},
        request_sharing::RequestSharing,
    },
    anyhow::Result,
    futures::{future::BoxFuture, FutureExt},
    gas_estimation::GasPriceEstimating,
    primitive_types::{H160, U256},
    rate_limit::RateLimiter,
    std::sync::Arc,
};

pub struct BalancerSor {
    api: Arc<dyn BalancerSorApi>,
    sharing: RequestSharing<
        Arc<Query>,
        BoxFuture<'static, Result<balancer_sor_api::Quote, PriceEstimationError>>,
    >,
    rate_limiter: Arc<RateLimiter>,
    gas: Arc<dyn GasPriceEstimating>,
    solver: H160,
}

impl BalancerSor {
    pub fn new(
        api: Arc<dyn BalancerSorApi>,
        rate_limiter: Arc<RateLimiter>,
        gas: Arc<dyn GasPriceEstimating>,
        solver: H160,
    ) -> Self {
        Self {
            api,
            sharing: RequestSharing::labelled("balancer_sor".into()),
            rate_limiter,
            gas,
            solver,
        }
    }

    async fn estimate(&self, query: Arc<Query>) -> PriceEstimateResult {
        let gas_price = self
            .gas
            .estimate()
            .await
            .map_err(PriceEstimationError::ProtocolInternal)?;
        let query_ = balancer_sor_api::Query {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            order_kind: query.kind,
            amount: query.in_amount.get(),
            gas_price: U256::from_f64_lossy(gas_price.effective_gas_price()),
        };
        let api = self.api.clone();
        let future = async move {
            match api.quote(query_).await {
                Ok(Some(quote)) => Ok(quote),
                Ok(None) => Err(PriceEstimationError::NoLiquidity),
                Err(err) => Err(PriceEstimationError::from(err)),
            }
        };
        let future = super::rate_limited(self.rate_limiter.clone(), future);
        let future = self.sharing.shared(query.clone(), future.boxed());
        let quote = future.await?;
        Ok(Estimate {
            out_amount: quote.return_amount,
            gas: SETTLEMENT_SINGLE_TRADE + (quote.swaps.len() as u64) * GAS_PER_BALANCER_SWAP,
            solver: self.solver,
            verified: false,
        })
    }
}

impl PriceEstimating for BalancerSor {
    fn estimate(&self, query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult> {
        tracing::info!("newlog BalancerSor query={:?}", query);
        self.estimate(query).boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::balancer_sor_api::DefaultBalancerSorApi,
        gas_estimation::GasPrice1559,
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
        std::time::Duration,
    };

    struct FixedGasPriceEstimator(f64);

    #[async_trait::async_trait]
    impl GasPriceEstimating for FixedGasPriceEstimator {
        async fn estimate_with_limits(&self, _: f64, _: Duration) -> Result<GasPrice1559> {
            Ok(GasPrice1559 {
                base_fee_per_gas: self.0,
                max_fee_per_gas: self.0,
                max_priority_fee_per_gas: 0.,
            })
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        let url = std::env::var("BALANCER_SOR_URL").unwrap();
        let api = Arc::new(DefaultBalancerSorApi::new(Default::default(), url, 1).unwrap());
        let rate_limiter = Arc::new(RateLimiter::from_strategy(
            Default::default(),
            "test".into(),
        ));
        let gas = Arc::new(FixedGasPriceEstimator(1e7));
        let estimator = BalancerSor::new(api, rate_limiter, gas, Default::default());
        let query = Arc::new(Query {
            verification: None,
            sell_token: testlib::tokens::WETH,
            buy_token: testlib::tokens::DAI,
            in_amount: NonZeroU256::try_from(U256::from_f64_lossy(1e18)).unwrap(),
            kind: OrderKind::Sell,
            block_dependent: false,
        });
        let result = estimator.estimate(query).await;
        println!("{result:?}");
        result.unwrap();
    }
}
