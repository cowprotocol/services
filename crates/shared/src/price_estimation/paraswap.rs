use super::{
    trade_finder::{TradeEstimator, TradeVerifier},
    PriceEstimateResult, PriceEstimating, Query,
};
use crate::{
    paraswap_api::ParaswapApi, rate_limiter::RateLimiter, token_info::TokenInfoFetching,
    trade_finding::paraswap::ParaswapTradeFinder,
};
use ethcontract::H160;
use std::sync::Arc;

pub struct ParaswapPriceEstimator(TradeEstimator);

impl ParaswapPriceEstimator {
    pub fn new(
        api: Arc<dyn ParaswapApi>,
        token_info: Arc<dyn TokenInfoFetching>,
        disabled_paraswap_dexs: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
        settlement: H160,
    ) -> Self {
        Self(TradeEstimator::new(
            settlement,
            Arc::new(ParaswapTradeFinder::new(
                api,
                token_info,
                disabled_paraswap_dexs,
            )),
            rate_limiter,
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
    use super::*;
    use crate::{
        ethrpc::{create_env_test_transport, Web3},
        paraswap_api::DefaultParaswapApi,
        price_estimation::single_estimate,
        token_info::TokenInfoFetcher,
    };
    use model::order::OrderKind;
    use reqwest::Client;

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let web3 = Web3::new(create_env_test_transport());
        let tokens = TokenInfoFetcher { web3 };
        let paraswap = DefaultParaswapApi {
            client: Client::new(),
            partner: "Test".to_string(),
            rate_limiter: None,
        };
        let estimator = ParaswapPriceEstimator::new(
            Arc::new(paraswap),
            Arc::new(tokens),
            Vec::default(),
            Arc::new(RateLimiter::from_strategy(
                Default::default(),
                "test".into(),
            )),
            testlib::protocol::SETTLEMENT,
        );

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;
        let query = Query {
            from: None,
            sell_token: weth,
            buy_token: gno,
            in_amount: 10u128.pow(18).into(),
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
