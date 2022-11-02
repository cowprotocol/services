use super::{
    trade_finder::{TradeEstimator, TradeVerifier},
    PriceEstimateResult, PriceEstimating, Query,
};
use crate::{
    oneinch_api::OneInchClient, rate_limiter::RateLimiter,
    trade_finding::oneinch::OneInchTradeFinder,
};
use primitive_types::H160;
use std::sync::Arc;

pub struct OneInchPriceEstimator(TradeEstimator);

impl OneInchPriceEstimator {
    pub fn new(
        api: Arc<dyn OneInchClient>,
        disabled_protocols: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
        referrer_address: Option<H160>,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(OneInchTradeFinder::new(
                api,
                disabled_protocols,
                referrer_address,
            )),
            rate_limiter,
        ))
    }

    pub fn verified(&self, verifier: TradeVerifier) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for OneInchPriceEstimator {
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
        oneinch_api::{MockOneInchClient, OneInchClientImpl, RestError, SellOrderQuote, Token},
        price_estimation::{single_estimate, PriceEstimationError},
    };
    use futures::FutureExt as _;
    use model::order::OrderKind;
    use reqwest::Client;

    impl OneInchPriceEstimator {
        fn test(api: impl OneInchClient) -> Self {
            Self::new(
                Arc::new(api),
                Vec::default(),
                Arc::new(RateLimiter::from_strategy(
                    Default::default(),
                    "test".into(),
                )),
                None,
            )
        }

        async fn estimate(&self, query: &Query) -> PriceEstimateResult {
            single_estimate(self, query).await
        }
    }

    #[tokio::test]
    async fn estimate_sell_order_succeeds() {
        // How much GNO can you buy for 1 WETH
        let mut one_inch = MockOneInchClient::new();

        // Response was generated with:
        //
        // curl 'https://api.1inch.io/v4.0/1/quote?\
        //     fromTokenAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
        //     toTokenAddress=0x6810e776880c02933d47db1b9fc05908e5386b96&\
        //     amount=100000000000000000'
        one_inch.expect_get_sell_order_quote().return_once(|_| {
            async {
                Ok(SellOrderQuote {
                    from_token: Token {
                        address: testlib::tokens::WETH,
                    },
                    to_token: Token {
                        address: testlib::tokens::GNO,
                    },
                    to_token_amount: 808_069_760_400_778_577u128.into(),
                    from_token_amount: 100_000_000_000_000_000u128.into(),
                    protocols: Vec::default(),
                    estimated_gas: 189_386,
                })
            }
            .boxed()
        });

        let estimator = OneInchPriceEstimator::test(one_inch);

        let est = estimator
            .estimate(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap();

        assert_eq!(est.out_amount, 808_069_760_400_778_577u128.into());
        assert!(est.gas > 189_386);
    }

    #[tokio::test]
    async fn estimating_buy_order_fails() {
        let mut one_inch = MockOneInchClient::new();

        one_inch.expect_get_sell_order_quote().times(0);

        let estimator = OneInchPriceEstimator::test(one_inch);

        let est = estimator
            .estimate(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Buy,
            })
            .await;

        assert!(matches!(
            est,
            Err(PriceEstimationError::UnsupportedOrderType)
        ));
    }

    #[tokio::test]
    async fn rest_api_errors_get_propagated() {
        let mut one_inch = MockOneInchClient::new();
        one_inch
            .expect_get_sell_order_quote()
            .times(1)
            .return_once(|_| {
                async {
                    Err(RestError {
                        status_code: 500,
                        description: "Internal Server Error".to_string(),
                    }
                    .into())
                }
                .boxed()
            });

        let estimator = OneInchPriceEstimator::test(one_inch);

        let est = estimator
            .estimate(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await;

        assert!(matches!(
            est,
            Err(PriceEstimationError::Other(e)) if e.to_string().contains("Internal Server Error")
        ));
    }

    #[tokio::test]
    async fn request_errors_get_propagated() {
        let mut one_inch = MockOneInchClient::new();
        one_inch
            .expect_get_sell_order_quote()
            .times(1)
            .return_once(|_| async { Err(anyhow::anyhow!("malformed JSON").into()) }.boxed());

        let estimator = OneInchPriceEstimator::test(one_inch);

        let est = estimator
            .estimate(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await;

        assert!(matches!(
            est,
            Err(PriceEstimationError::Other(e)) if e.to_string() == "malformed JSON"
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let one_inch =
            OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1).unwrap();
        let estimator = OneInchPriceEstimator::test(one_inch);

        let result = estimator
            .estimate(&Query {
                from: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: 10u128.pow(18).into(),
                kind: OrderKind::Sell,
            })
            .await;

        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "1 WETH buys {} GNO, costing {} gas",
            estimate.out_amount.to_f64_lossy() / 1e18,
            estimate.gas,
        );
    }
}
