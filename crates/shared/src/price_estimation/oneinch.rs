use crate::{
    oneinch_api::{
        OneInchClient, ProtocolCache, RestResponse, SellOrderQuote, SellOrderQuoteQuery,
    },
    price_estimation::{
        gas, rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError,
        Query,
    },
    rate_limiter::RateLimiter,
    request_sharing::RequestSharing,
};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use model::order::OrderKind;
use std::sync::Arc;

pub struct OneInchPriceEstimator {
    api: Arc<dyn OneInchClient>,
    sharing: RequestSharing<
        Query,
        BoxFuture<'static, Result<RestResponse<SellOrderQuote>, PriceEstimationError>>,
    >,
    disabled_protocols: Vec<String>,
    protocol_cache: ProtocolCache,
    rate_limiter: Arc<RateLimiter>,
}

impl OneInchPriceEstimator {
    async fn estimate(&self, query: &Query) -> PriceEstimateResult {
        if query.kind == OrderKind::Buy {
            return Err(PriceEstimationError::UnsupportedOrderType);
        }

        let allowed_protocols = self
            .protocol_cache
            .get_allowed_protocols(&self.disabled_protocols, self.api.as_ref())
            .await?;

        let api = self.api.clone();
        let oneinch_query = SellOrderQuoteQuery::with_default_options(
            query.sell_token,
            query.buy_token,
            allowed_protocols,
            query.in_amount,
        );
        let quote_future = async move {
            api.get_sell_order_quote(oneinch_query)
                .await
                .map_err(PriceEstimationError::Other)
        };
        let quote_future = rate_limited(self.rate_limiter.clone(), quote_future);
        let quote = self.sharing.shared(*query, quote_future.boxed()).await?;

        match quote {
            RestResponse::Ok(quote) => Ok(Estimate {
                out_amount: quote.to_token_amount,
                gas: gas::SETTLEMENT_OVERHEAD + quote.estimated_gas,
            }),
            RestResponse::Err(e) => {
                Err(PriceEstimationError::Other(anyhow::anyhow!(e.description)))
            }
        }
    }

    pub fn new(
        api: Arc<dyn OneInchClient>,
        disabled_protocols: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            api,
            disabled_protocols,
            protocol_cache: ProtocolCache::default(),
            sharing: Default::default(),
            rate_limiter,
        }
    }
}

impl PriceEstimating for OneInchPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(
            queries.iter().all(|query| {
                query.buy_token != model::order::BUY_ETH_ADDRESS
                    && query.sell_token != model::order::BUY_ETH_ADDRESS
                    && query.sell_token != query.buy_token
            }),
            "the hierarchy of price estimators should be set up \
            such that OneInchPriceEstimator is a descendant of \
            a SanitizedPriceEstimator"
        );

        futures::stream::iter(queries)
            .then(|query| self.estimate(query))
            .enumerate()
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oneinch_api::{
        MockOneInchClient, OneInchClientImpl, RestError, SellOrderQuote, Token,
    };
    use reqwest::Client;

    fn create_estimator<T: OneInchClient + 'static>(api: T) -> OneInchPriceEstimator {
        OneInchPriceEstimator::new(
            Arc::new(api),
            Vec::default(),
            Arc::new(RateLimiter::from_strategy(
                Default::default(),
                "test".into(),
            )),
        )
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
            Ok(RestResponse::<_>::Ok(SellOrderQuote {
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
            }))
        });

        let estimator = create_estimator(one_inch);

        let est = estimator
            .estimate(&Query {
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

        let estimator = create_estimator(one_inch);

        let est = estimator
            .estimate(&Query {
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
                Ok(RestResponse::<SellOrderQuote>::Err(RestError {
                    status_code: 500,
                    description: "Internal Server Error".to_string(),
                }))
            });

        let estimator = create_estimator(one_inch);

        let est = estimator
            .estimate(&Query {
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await;

        assert!(matches!(
            est,
            Err(PriceEstimationError::Other(e)) if e.to_string() == "Internal Server Error"
        ));
    }

    #[tokio::test]
    async fn request_errors_get_propagated() {
        let mut one_inch = MockOneInchClient::new();
        one_inch
            .expect_get_sell_order_quote()
            .times(1)
            .return_once(|_| Err(anyhow::anyhow!("malformed JSON")));

        let estimator = create_estimator(one_inch);

        let est = estimator
            .estimate(&Query {
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
        let estimator = create_estimator(one_inch);

        let result = estimator
            .estimate(&Query {
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
