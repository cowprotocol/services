use super::{Query, Quote, TradeError, TradeFinding};
use crate::{
    paraswap_api::{
        ParaswapApi, ParaswapResponseError, PriceQuery, PriceResponse, Side, TradeAmount,
        TransactionBuilderQuery,
    },
    price_estimation::gas,
    token_info::{TokenInfo, TokenInfoFetching},
    trade_finding::{Interaction, Trade},
};
use anyhow::{Context, Result};
use model::order::OrderKind;
use primitive_types::H160;
use std::{collections::HashMap, sync::Arc};

pub struct ParaswapTradeFinder {
    paraswap: Arc<dyn ParaswapApi>,
    tokens: Arc<dyn TokenInfoFetching>,
    disabled_paraswap_dexs: Vec<String>,
}

type Tokens = HashMap<H160, TokenInfo>;

impl ParaswapTradeFinder {
    pub fn new(
        api: Arc<dyn ParaswapApi>,
        tokens: Arc<dyn TokenInfoFetching>,
        disabled_paraswap_dexs: Vec<String>,
    ) -> Self {
        Self {
            paraswap: api,
            tokens,
            disabled_paraswap_dexs,
        }
    }

    async fn quote(&self, query: &Query) -> Result<(Quote, Tokens, PriceResponse), TradeError> {
        let tokens = self
            .tokens
            .get_token_infos(&[query.sell_token, query.buy_token])
            .await;

        let price_query = PriceQuery {
            src_token: query.sell_token,
            dest_token: query.buy_token,
            src_decimals: decimals(&tokens, &query.sell_token)?,
            dest_decimals: decimals(&tokens, &query.buy_token)?,
            amount: query.in_amount,
            side: match query.kind {
                OrderKind::Buy => Side::Buy,
                OrderKind::Sell => Side::Sell,
            },
            exclude_dexs: Some(self.disabled_paraswap_dexs.clone()),
        };

        let price = self.paraswap.price(price_query).await?;
        let quote = Quote {
            out_amount: match query.kind {
                OrderKind::Buy => price.src_amount,
                OrderKind::Sell => price.dest_amount,
            },
            gas_estimate: gas::SETTLEMENT_OVERHEAD + price.gas_cost,
        };

        Ok((quote, tokens, price))
    }

    // Default to 1% slippage - same as the ParaSwap UI.
    const DEFAULT_SLIPPAGE: u32 = 10_000;
    // Use a default non-zero user address, otherwise the API will return an
    // error.
    const DEFAULT_USER: H160 = addr!("BEeFbeefbEefbeEFbeEfbEEfBEeFbeEfBeEfBeef");

    async fn trade(&self, query: &Query) -> Result<Trade, TradeError> {
        let (quote, tokens, mut price) = self.quote(query).await?;
        let tx_query = TransactionBuilderQuery {
            src_token: query.sell_token,
            dest_token: query.buy_token,
            trade_amount: match query.kind {
                OrderKind::Buy => TradeAmount::Buy {
                    dest_amount: query.in_amount,
                },
                OrderKind::Sell => TradeAmount::Sell {
                    src_amount: query.in_amount,
                },
            },
            slippage: Self::DEFAULT_SLIPPAGE,
            src_decimals: decimals(&tokens, &query.sell_token)?,
            dest_decimals: decimals(&tokens, &query.buy_token)?,
            price_route: price.price_route_raw.take(),
            user_address: query.from.unwrap_or(Self::DEFAULT_USER),
        };

        let tx = self.paraswap.transaction(tx_query).await?;

        Ok(Trade {
            out_amount: quote.out_amount,
            gas_estimate: quote.gas_estimate,
            approval: Some((query.sell_token, price.token_transfer_proxy)),
            interaction: Interaction {
                target: tx.to,
                value: tx.value,
                data: tx.data,
            },
        })
    }
}

#[async_trait::async_trait]
impl TradeFinding for ParaswapTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        let (quote, ..) = self.quote(query).await?;
        Ok(quote)
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.trade(query).await
    }
}

impl From<ParaswapResponseError> for TradeError {
    fn from(err: ParaswapResponseError) -> Self {
        match err {
            ParaswapResponseError::InsufficientLiquidity(_) => Self::NoLiquidity,
            _ => Self::Other(err.into()),
        }
    }
}

fn decimals(tokens: &Tokens, token: &H160) -> Result<u8, TradeError> {
    Ok(tokens
        .get(token)
        .and_then(|info| info.decimals)
        .context("failed to get decimals")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        paraswap_api::DefaultParaswapApi, token_info::TokenInfoFetcher,
        transport::create_env_test_transport, Web3,
    };
    use reqwest::Client;

    #[tokio::test]
    #[ignore]
    async fn real_trade() {
        let web3 = Web3::new(create_env_test_transport());
        let tokens = TokenInfoFetcher { web3 };
        let paraswap = DefaultParaswapApi {
            client: Client::new(),
            partner: "Test".to_string(),
            rate_limiter: None,
        };
        let finder = ParaswapTradeFinder {
            paraswap: Arc::new(paraswap),
            tokens: Arc::new(tokens),
            disabled_paraswap_dexs: Vec::new(),
        };

        let trade = finder
            .get_trade(&Query {
                from: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::COW,
                in_amount: 10u128.pow(18).into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap();

        println!("1 ETH buys {} COW", trade.out_amount.to_f64_lossy() / 1e18);
    }
}
