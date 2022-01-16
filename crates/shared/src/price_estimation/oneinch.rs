use super::gas;
use crate::oneinch_api::{OneInchClient, RestResponse, SellOrderQuoteQuery};
use crate::price_estimation::{Estimate, PriceEstimating, PriceEstimationError, Query};
use anyhow::Result;
use cached::{Cached, TimedSizedCache};
use futures::future;
use model::order::OrderKind;
use primitive_types::U256;
use std::sync::{Arc, Mutex};

pub struct OneInchPriceEstimator {
    api: Arc<dyn OneInchClient>,
    disabled_protocols: Vec<String>,
    allowed_protocols: Arc<Mutex<TimedSizedCache<Vec<String>, Vec<String>>>>,
}

impl OneInchPriceEstimator {
    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        if query.kind == OrderKind::Buy {
            return Err(PriceEstimationError::UnsupportedOrderType);
        }

        let quote = self
            .api
            .get_sell_order_quote(SellOrderQuoteQuery {
                from_token_address: query.sell_token,
                to_token_address: query.buy_token,
                amount: query.in_amount,
                protocols: self.get_protocol_argument().await?,
                fee: None,
                gas_limit: None,
                connector_tokens: None,
                complexity_level: None,
                main_route_parts: None,
                virtual_parts: None,
                parts: None,
                gas_price: None,
            })
            .await
            .map_err(PriceEstimationError::Other)?;

        match quote {
            RestResponse::Ok(quote) => Ok(Estimate {
                out_amount: quote.to_token_amount,
                gas: U256::from(gas::SETTLEMENT_OVERHEAD) + quote.estimated_gas,
            }),
            RestResponse::Err(e) => {
                Err(PriceEstimationError::Other(anyhow::anyhow!(e.description)))
            }
        }
    }

    pub fn new(api: Arc<dyn OneInchClient>, disabled_protocols: Vec<String>) -> Self {
        Self {
            api,
            disabled_protocols,
            allowed_protocols: Arc::new(Mutex::new(
                TimedSizedCache::with_size_and_lifespan_and_refresh(1, 60, false),
            )),
        }
    }

    async fn get_protocol_argument(&self) -> Result<Option<Vec<String>>> {
        if self.disabled_protocols.is_empty() {
            return Ok(None);
        }

        if let Some(allowed_protocols) = self
            .allowed_protocols
            .lock()
            .unwrap()
            .cache_get(&self.disabled_protocols)
        {
            return Ok(Some(allowed_protocols.clone()));
        }

        let current_protocols = self.api.get_protocols().await?.protocols;
        let allowed_protocols: Vec<String> = current_protocols
            .into_iter()
            // linear search through the Vec is okay because it's very small
            .filter(|protocol| !self.disabled_protocols.contains(protocol))
            .collect();

        self.allowed_protocols
            .lock()
            .unwrap()
            .cache_set(self.disabled_protocols.clone(), allowed_protocols.clone());

        Ok(Some(allowed_protocols))
    }
}

#[async_trait::async_trait]
impl PriceEstimating for OneInchPriceEstimator {
    async fn estimates(
        &self,
        queries: &[Query],
    ) -> Vec<anyhow::Result<Estimate, PriceEstimationError>> {
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

        future::join_all(
            queries
                .iter()
                .map(|query| async move { self.estimate(query).await }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oneinch_api::{
        MockOneInchClient, OneInchClientImpl, Protocols, RestError, SellOrderQuote, Token,
    };
    use reqwest::Client;

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

        let estimator = OneInchPriceEstimator::new(Arc::new(one_inch), Vec::default());

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
        assert!(est.gas > 189_386.into());
    }

    #[tokio::test]
    async fn estimating_buy_order_fails() {
        let mut one_inch = MockOneInchClient::new();

        one_inch.expect_get_sell_order_quote().times(0);

        let estimator = OneInchPriceEstimator::new(Arc::new(one_inch), Vec::default());

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

        let estimator = OneInchPriceEstimator::new(Arc::new(one_inch), Vec::default());

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

        let estimator = OneInchPriceEstimator::new(Arc::new(one_inch), Vec::default());

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
    async fn filter_out_disabled_protocols_and_cache_them() {
        let mut one_inch = MockOneInchClient::new();
        // We are estimating 2 orders but fetch the protocols only once
        one_inch.expect_get_protocols().times(1).returning(|| {
            Ok(Protocols {
                protocols: vec!["PMM1".into(), "UNISWAP_V3".into()],
            })
        });
        one_inch
            .expect_get_sell_order_quote()
            .times(2)
            .withf(|query| {
                let protocols = query.protocols.as_ref().unwrap();
                protocols.len() == 1 && protocols[0] == "UNISWAP_V3"
            })
            .returning(|_| {
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

        let estimator = OneInchPriceEstimator::new(Arc::new(one_inch), vec!["PMM1".to_string()]);

        for _ in 0..=1 {
            estimator
                .estimate(&Query {
                    sell_token: testlib::tokens::WETH,
                    buy_token: testlib::tokens::GNO,
                    in_amount: 1_000_000_000_000_000_000u128.into(),
                    kind: OrderKind::Sell,
                })
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = OneInchPriceEstimator::new(
            Arc::new(
                OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new()).unwrap(),
            ),
            Vec::default(),
        );

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
