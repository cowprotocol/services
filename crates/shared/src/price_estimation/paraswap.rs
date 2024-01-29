use {
    super::{
        trade_finder::TradeEstimator,
        trade_verifier::TradeVerifying,
        PriceEstimateResult,
        PriceEstimating,
        Query,
    },
    crate::{
        paraswap_api::ParaswapApi,
        token_info::TokenInfoFetching,
        trade_finding::paraswap::ParaswapTradeFinder,
    },
    futures::FutureExt,
    primitive_types::H160,
    rate_limit::RateLimiter,
    std::sync::Arc,
};

pub struct ParaswapPriceEstimator(TradeEstimator);

impl ParaswapPriceEstimator {
    pub fn new(
        api: Arc<dyn ParaswapApi>,
        token_info: Arc<dyn TokenInfoFetching>,
        disabled_paraswap_dexs: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
        solver: H160,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ParaswapTradeFinder::new(
                api,
                token_info,
                disabled_paraswap_dexs,
                solver,
            )),
            rate_limiter,
            "paraswap".into(),
        ))
    }

    pub fn verified(&self, verifier: Arc<dyn TradeVerifying>) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for ParaswapPriceEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.0.estimate(query).boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            ethrpc::{create_env_test_transport, Web3},
            paraswap_api::DefaultParaswapApi,
            token_info::TokenInfoFetcher,
        },
        ethrpc::current_block::BlockInfo,
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
        reqwest::Client,
        tokio::sync::watch,
    };

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let web3 = Web3::new(create_env_test_transport());
        let tokens = TokenInfoFetcher { web3 };
        let (_, block_stream) = watch::channel(BlockInfo::default());
        let paraswap = DefaultParaswapApi {
            client: Client::new(),
            base_url: "https://apiv5.paraswap.io".to_string(),
            partner: "Test".to_string(),
            block_stream,
        };
        let estimator = ParaswapPriceEstimator::new(
            Arc::new(paraswap),
            Arc::new(tokens),
            Vec::default(),
            Arc::new(RateLimiter::from_strategy(
                Default::default(),
                "test".into(),
            )),
            H160([1; 20]),
        );

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;
        let query = Arc::new(Query {
            verification: None,
            sell_token: weth,
            buy_token: gno,
            in_amount: NonZeroU256::try_from(10u128.pow(18)).unwrap(),
            kind: OrderKind::Sell,
            block_dependent: false,
        });

        let result = estimator.estimate(query).await;
        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "1 eth buys {} gno",
            estimate.out_amount.to_f64_lossy() / 1e18
        );
    }
}
