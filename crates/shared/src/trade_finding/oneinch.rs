//! A 1Inch-based trade finder.

use super::{Interaction, Query, Quote, Trade, TradeError, TradeFinding};
use crate::{
    oneinch_api::{OneInchClient, OneInchError, ProtocolCache, SellOrderQuoteQuery, SwapQuery},
    price_estimation::gas,
    solver_utils::Slippage,
};
use model::order::OrderKind;
use primitive_types::H160;
use std::sync::Arc;

pub struct OneInchTradeFinder {
    api: Arc<dyn OneInchClient>,
    disabled_protocols: Vec<String>,
    protocol_cache: ProtocolCache,
    referrer_address: Option<H160>,
}

impl OneInchTradeFinder {
    async fn verify_query_and_get_protocols(
        &self,
        query: &Query,
    ) -> Result<Option<Vec<String>>, TradeError> {
        if query.kind == OrderKind::Buy {
            return Err(TradeError::UnsupportedOrderType);
        }

        let allowed_protocols = self
            .protocol_cache
            .get_allowed_protocols(&self.disabled_protocols, self.api.as_ref())
            .await?;

        Ok(allowed_protocols)
    }

    async fn quote(&self, query: &Query) -> Result<Quote, TradeError> {
        let allowed_protocols = self.verify_query_and_get_protocols(query).await?;
        Ok(self.perform_quote(query, allowed_protocols).await?)
    }

    async fn perform_quote(
        &self,
        query: &Query,
        allowed_protocols: Option<Vec<String>>,
    ) -> Result<Quote, OneInchError> {
        let quote = self
            .api
            .get_sell_order_quote(SellOrderQuoteQuery::with_default_options(
                query.sell_token,
                query.buy_token,
                allowed_protocols,
                query.in_amount,
                self.referrer_address,
            ))
            .await?;

        Ok(Quote {
            out_amount: quote.to_token_amount,
            gas_estimate: gas::SETTLEMENT_OVERHEAD + quote.estimated_gas,
        })
    }

    async fn quote_and_swap(&self, query: &Query) -> Result<Trade, TradeError> {
        let allowed_protocols = self.verify_query_and_get_protocols(query).await?;
        let (quote, spender, swap) = futures::try_join!(
            self.perform_quote(query, allowed_protocols.clone()),
            self.api.get_spender(),
            self.api.get_swap(SwapQuery::with_default_options(
                query.sell_token,
                query.buy_token,
                query.in_amount,
                query.from.unwrap_or_default(),
                allowed_protocols,
                Slippage::ONE_PERCENT,
                self.referrer_address,
            )),
        )?;

        Ok(Trade {
            out_amount: quote.out_amount,
            gas_estimate: quote.gas_estimate,
            approval: Some((query.sell_token, spender.address)),
            interaction: Interaction {
                target: swap.tx.to,
                value: swap.tx.value,
                data: swap.tx.data,
            },
        })
    }

    pub fn new(
        api: Arc<dyn OneInchClient>,
        disabled_protocols: Vec<String>,
        referrer_address: Option<H160>,
    ) -> Self {
        Self {
            api,
            disabled_protocols,
            protocol_cache: ProtocolCache::default(),
            referrer_address,
        }
    }
}

impl From<OneInchError> for TradeError {
    fn from(err: OneInchError) -> Self {
        match err {
            err if err.is_insuffucient_liquidity() => Self::NoLiquidity,
            err => Self::Other(err.into()),
        }
    }
}

#[async_trait::async_trait]
impl TradeFinding for OneInchTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        self.quote(query).await
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.quote_and_swap(query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oneinch_api::{
        MockOneInchClient, OneInchClientImpl, RestError, SellOrderQuote, Spender, Swap, Token,
        Transaction,
    };
    use reqwest::Client;

    fn create_trade_finder<T: OneInchClient + 'static>(api: T) -> OneInchTradeFinder {
        OneInchTradeFinder::new(Arc::new(api), Vec::default(), None)
    }

    #[tokio::test]
    async fn quote_sell_order_succeeds() {
        // How much GNO can you buy for 1 WETH
        let mut one_inch = MockOneInchClient::new();

        // Response was generated with:
        //
        // curl 'https://api.1inch.io/v4.0/1/quote?\
        //     fromTokenAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
        //     toTokenAddress=0x6810e776880c02933d47db1b9fc05908e5386b96&\
        //     amount=100000000000000000'
        one_inch.expect_get_sell_order_quote().return_once(|_| {
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
        });

        let estimator = create_trade_finder(one_inch);

        let quote = estimator
            .get_quote(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap();

        assert_eq!(quote.out_amount, 808_069_760_400_778_577u128.into());
        assert!(quote.gas_estimate > 189_386);
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
        //
        // curl 'https://api.1inch.io/v4.0/1/swap?\
        //     fromTokenAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
        //     toTokenAddress=0x6810e776880c02933d47db1b9fc05908e5386b96&\
        //     amount=100000000000000000&\
        //     fromAddress=0x0000000000000000000000000000000000000000&\
        //     slippage=1&\
        //     disableEstimate=true'
        one_inch.expect_get_sell_order_quote().return_once(|_| {
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
        });
        one_inch.expect_get_spender().return_once(|| {
            Ok(Spender {
                address: addr!("11111112542d85b3ef69ae05771c2dccff4faa26"),
            })
        });
        one_inch.expect_get_swap().return_once(|_| {
            Ok(Swap {
                from_token: Token {
                    address: testlib::tokens::WETH,
                },
                to_token: Token {
                    address: testlib::tokens::GNO,
                },
                to_token_amount: 808_069_760_400_778_577u128.into(),
                from_token_amount: 100_000_000_000_000_000u128.into(),
                protocols: Default::default(),
                tx: Transaction {
                    from: Default::default(),
                    to: addr!("1111111254fb6c44bac0bed2854e76f90643097d"),
                    data: vec![0xe4, 0x49, 0x02, 0x2e],
                    value: Default::default(),
                    max_fee_per_gas: Default::default(),
                    max_priority_fee_per_gas: Default::default(),
                    gas: Default::default(),
                },
            })
        });

        let estimator = create_trade_finder(one_inch);

        let trade = estimator
            .get_trade(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap();

        assert_eq!(trade.out_amount, 808_069_760_400_778_577u128.into());
        assert!(trade.gas_estimate > 189_386);
        assert_eq!(
            trade.interaction,
            Interaction {
                target: addr!("1111111254fb6c44bac0bed2854e76f90643097d"),
                value: Default::default(),
                data: vec![0xe4, 0x49, 0x02, 0x2e],
            }
        );
    }

    #[tokio::test]
    async fn estimating_buy_order_fails() {
        let mut one_inch = MockOneInchClient::new();

        one_inch.expect_get_sell_order_quote().times(0);

        let estimator = create_trade_finder(one_inch);

        let est = estimator
            .get_trade(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Buy,
            })
            .await;

        assert!(matches!(est, Err(TradeError::UnsupportedOrderType)));
    }

    #[tokio::test]
    async fn rest_api_errors_get_propagated() {
        let mut one_inch = MockOneInchClient::new();
        one_inch
            .expect_get_sell_order_quote()
            .times(1)
            .return_once(|_| {
                Err(OneInchError::Api(RestError {
                    status_code: 500,
                    description: "Internal Server Error".to_string(),
                }))
            });

        let estimator = create_trade_finder(one_inch);

        let est = estimator
            .get_trade(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await;

        assert!(matches!(
            est,
            Err(TradeError::Other(e)) if e.to_string().contains("Internal Server Error")
        ));
    }

    #[tokio::test]
    async fn request_errors_get_propagated() {
        let mut one_inch = MockOneInchClient::new();
        one_inch
            .expect_get_sell_order_quote()
            .times(1)
            .return_once(|_| Err(OneInchError::Other(anyhow::anyhow!("malformed JSON"))));

        let estimator = create_trade_finder(one_inch);

        let est = estimator
            .get_trade(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 1_000_000_000_000_000_000u128.into(),
                kind: OrderKind::Sell,
            })
            .await;

        assert!(matches!(
            est,
            Err(TradeError::Other(e)) if e.to_string() == "malformed JSON"
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let one_inch =
            OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1).unwrap();
        let estimator = create_trade_finder(one_inch);

        let result = estimator
            .get_trade(&Query {
                from: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: 10u128.pow(18).into(),
                kind: OrderKind::Sell,
            })
            .await;

        let trade = result.unwrap();
        println!(
            "1 WETH buys {} GNO, costing {} gas",
            trade.out_amount.to_f64_lossy() / 1e18,
            trade.gas_estimate,
        );
    }
}
