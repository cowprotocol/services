use super::{trade_finder::TradeEstimator, PriceEstimateResult, PriceEstimating, Query};
use crate::{
    paraswap_api::ParaswapApi, rate_limiter::RateLimiter, token_info::TokenInfoFetching,
    trade_finding::paraswap::ParaswapTradeFinder,
};
use std::sync::Arc;

pub struct ParaswapPriceEstimator(TradeEstimator);

impl ParaswapPriceEstimator {
    pub fn new(
        api: Arc<dyn ParaswapApi>,
        token_info: Arc<dyn TokenInfoFetching>,
        disabled_paraswap_dexs: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ParaswapTradeFinder::new(
                api,
                token_info,
                disabled_paraswap_dexs,
            )),
            rate_limiter,
        ))
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
        paraswap_api::DefaultParaswapApi, price_estimation::single_estimate,
        token_info::TokenInfoFetcher, transport::create_env_test_transport, Web3,
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
