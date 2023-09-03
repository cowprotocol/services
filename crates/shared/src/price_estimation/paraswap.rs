use {
    super::{
        trade_finder::{TradeEstimator, TradeVerifier},
        PriceEstimateResult,
        PriceEstimating,
        Query,
    },
    crate::{
        paraswap_api::ParaswapApi,
        rate_limiter::RateLimiter,
        token_info::TokenInfoFetching,
        trade_finding::paraswap::ParaswapTradeFinder,
    },
    primitive_types::H160,
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

    pub fn verified(&self, verifier: TradeVerifier) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for ParaswapPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        self.0.estimates(queries)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            ethrpc::{create_env_test_transport, Web3},
            paraswap_api::DefaultParaswapApi,
            price_estimation::single_estimate,
            token_info::TokenInfoFetcher,
        },
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
        reqwest::Client,
    };

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let web3 = Web3::new(create_env_test_transport());
        let tokens = TokenInfoFetcher { web3 };
        let paraswap = DefaultParaswapApi {
            client: Client::new(),
            base_url: "https://apiv5.paraswap.io".to_string(),
            partner: "Test".to_string(),
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
        let query = Query {
            verification: None,
            sell_token: weth,
            buy_token: gno,
            in_amount: NonZeroU256::try_from(10u128.pow(18)).unwrap(),
            kind: OrderKind::Sell,
        };

        let result = single_estimate(&estimator, &query).await;
        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "1 eth buys {} gno",
            estimate.out_amount.to_f64_lossy() / 1e18
        );
    }
}
