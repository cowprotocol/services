//! Uniswap API implementation.
//!
//! This is the same API used by the Uniswap front end, which routes trades to
//! Uniswap V3 and V2 in batched "multi-calls".

use anyhow::{Context as _, Result};
use ethcontract::{H160, U256};
use http::header;
use model::{order::OrderKind, u256_decimal};
use reqwest::{Client, IntoUrl, Url};
use serde::Deserialize;

use crate::price_estimation;

/// A trait abstracting Uniswap V3 client implementation.
#[mockall::automock]
#[async_trait::async_trait]
pub trait UniswapApi: Send + Sync + 'static {
    /// Retrieves a quote for the specified query.
    async fn get_quote(&self, query: &QuoteQuery) -> Result<Quote>;
}

/// 0x API Client implementation.
#[derive(Debug)]
pub struct UniswapHttpApi {
    client: Client,
    base_url: Url,
}

impl UniswapHttpApi {
    /// Default 0x API URL.
    pub const DEFAULT_URL: &'static str = "https://api.uniswap.org/";

    /// Create a new 0x HTTP API client with the specified base URL.
    pub fn new(client: Client) -> Self {
        Self::with_url(client, Self::DEFAULT_URL).unwrap()
    }

    /// Create a new 0x HTTP API client with the specified base URL.
    pub fn with_url(client: Client, base_url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            client,
            base_url: base_url.into_url().context("invalid Uniswap API url")?,
        })
    }
}

#[async_trait::async_trait]
impl UniswapApi for UniswapHttpApi {
    async fn get_quote(&self, query: &QuoteQuery) -> Result<Quote> {
        let url = query.encode_url(&self.base_url);
        tracing::debug!(%url, "querying Uniswap API");

        let request = self
            .client
            .get(url)
            .header(header::ORIGIN, "https://app.uniswap.org");
        let response = request
            .send()
            .await
            .context("error sending Uniswap QPI request")?
            .text()
            .await
            .context("error receiving Uniswap API response")?;
        tracing::debug!(%response, "received Uniswap API response");

        let quote = serde_json::from_str(&response)?;
        Ok(quote)
    }
}

/// Quote query parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuoteQuery {
    pub protocols: Vec<Protocol>,
    pub token_in_address: H160,
    pub token_in_chain_id: u64,
    pub token_out_address: H160,
    pub token_out_chain_id: u64,
    pub amount: U256,
    pub kind: QuoteKind,
}

impl QuoteQuery {
    /// Contruct a Uniswap API quote query from a internal price estimation query.
    pub fn from_price_query(query: price_estimation::Query, chain_id: u64) -> Self {
        Self {
            protocols: Protocol::all(),
            token_in_address: query.sell_token,
            token_in_chain_id: chain_id,
            token_out_address: query.buy_token,
            token_out_chain_id: chain_id,
            amount: query.in_amount,
            kind: query.kind.into(),
        }
    }

    fn encode_url(&self, base_url: &Url) -> Url {
        let mut url = base_url
            .join("/v1/quote")
            .expect("unexpectedly invalid URL segment");

        url.query_pairs_mut()
            .append_pair("protocols", &join_protocols(&self.protocols))
            .append_pair("tokenInAddress", &format!("{:#x}", self.token_in_address))
            .append_pair("tokenInChainId", &self.token_in_chain_id.to_string())
            .append_pair("tokenOutAddress", &format!("{:#x}", self.token_out_address))
            .append_pair("tokenOutChainId", &self.token_out_chain_id.to_string())
            .append_pair("amount", &self.amount.to_string())
            .append_pair("type", self.kind.name());

        url
    }
}

/// Uniswap API protocols
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Protocol {
    V2,
    V3,
}

impl Protocol {
    /// Returns a collection of all protocols.
    pub fn all() -> Vec<Self> {
        vec![Self::V2, Self::V3]
    }

    /// Returns the protocol as a string.
    fn name(&self) -> &'static str {
        match self {
            Self::V2 => "v2",
            Self::V3 => "v3",
        }
    }
}

fn join_protocols(protocols: &[Protocol]) -> String {
    let mut buffer = String::with_capacity(protocols.len() * 3);
    for (i, protocol) in protocols.iter().enumerate() {
        if i > 0 {
            buffer.push(',');
        }
        buffer.push_str(protocol.name());
    }
    buffer
}

/// The quote kind.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum QuoteKind {
    ExactIn,
    ExactOut,
}

impl QuoteKind {
    /// Returns the protocol as a string.
    fn name(&self) -> &'static str {
        match self {
            Self::ExactIn => "exactIn",
            Self::ExactOut => "exactOut",
        }
    }
}

impl From<OrderKind> for QuoteKind {
    fn from(kind: OrderKind) -> Self {
        match kind {
            OrderKind::Buy => QuoteKind::ExactOut,
            OrderKind::Sell => QuoteKind::ExactIn,
        }
    }
}

/// A Uniswap API quote.
///
/// Note that this does not contain all response fields. This type can be
/// augmented to include them as needed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub block_number: u64,
    #[serde(with = "u256_decimal")]
    pub amount: U256,
    #[serde(with = "u256_decimal")]
    pub quote: U256,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub gas_use_estimate: u64,
    pub route: Vec<Vec<Hop>>,
    pub quote_id: String,
}

/// A hop along a Uniswap route.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hop {
    #[serde(rename = "type")]
    pub kind: HopKind,
    pub address: H160,
    pub token_in: Token,
    pub token_out: Token,
    #[serde(with = "u256_decimal")]
    pub amount_in: U256,
    #[serde(with = "u256_decimal")]
    pub amount_out: U256,
}

/// The type of the hop in a Uniswap route.
#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum HopKind {
    #[serde(rename = "v3-pool")]
    V3Pool,
}

/// Rich token data included in API responses.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub chain_id: u64,
    pub address: H160,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn encode_quote_query_url() {
        let query = QuoteQuery {
            protocols: Protocol::all(),
            token_in_address: addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            token_in_chain_id: 1,
            token_out_address: addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            token_out_chain_id: 1,
            amount: 1_000_000_000_000_000_000_u128.into(),
            kind: QuoteKind::ExactIn,
        };
        assert_eq!(
            query
                .encode_url(&Url::parse(UniswapHttpApi::DEFAULT_URL).unwrap())
                .to_string(),
            "https://api.uniswap.org/v1/quote\
             ?protocols=v2%2Cv3\
             &tokenInAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
             &tokenInChainId=1\
             &tokenOutAddress=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48\
             &tokenOutChainId=1\
             &amount=1000000000000000000\
             &type=exactIn",
        );
    }

    #[test]
    fn deserialize_quote_response() {
        assert_eq!(
            serde_json::from_value::<Quote>(json!({
                "blockNumber": "14313444",
                "amount": "90197095236522892794",
                "amountDecimals": "90.197095236522892794",
                "quote": "260200056940",
                "quoteDecimals": "260200.05694",
                "quoteGasAdjusted": "260190833785",
                "quoteGasAdjustedDecimals": "260190.833785",
                "gasUseEstimateQuote": "9223154",
                "gasUseEstimateQuoteDecimals": "9.223154",
                "gasUseEstimate": "113000",
                "gasUseEstimateUSD": "9.223154",
                "gasPriceWei": "28269599992",
                "route": [
                    [
                        {
                            "type": "v3-pool",
                            "address": "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640",
                            "tokenIn": {
                                "chainId": 1,
                                "decimals": "18",
                                "address": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                                "symbol": "WETH"
                            },
                            "tokenOut": {
                                "chainId": 1,
                                "decimals": "6",
                                "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                                "symbol": "USDC"
                            },
                            "fee": "500",
                            "liquidity": "22814965866008577850",
                            "sqrtRatioX96": "1474979455923742951209742844657601",
                            "tickCurrent": "196646",
                            "amountIn": "90197095236522892794",
                            "amountOut": "260200056940"
                        }
                    ]
                ],
                "routeString": "[V3] 100.00% = WETH -- \
                    0.05% [0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640] \
                    --> USDC",
                "quoteId": "b9d64"
            }))
            .unwrap(),
            Quote {
                block_number: 14313444,
                amount: 90_197_095_236_522_892_794_u128.into(),
                quote: 260_200_056_940_u128.into(),
                gas_use_estimate: 113000,
                route: vec![vec![Hop {
                    kind: HopKind::V3Pool,
                    address: addr!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
                    token_in: Token {
                        chain_id: 1,
                        address: addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    },
                    token_out: Token {
                        chain_id: 1,
                        address: addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    },
                    amount_in: 90_197_095_236_522_892_794_u128.into(),
                    amount_out: 260_200_056_940_u128.into(),
                }]],
                quote_id: "b9d64".to_owned(),
            }
        );
    }

    #[tokio::test]
    #[ignore]
    async fn uniswap_api_quote() {
        let client = UniswapHttpApi::new(Client::new());
        println!(
            "{:#?}",
            client
                .get_quote(&QuoteQuery {
                    protocols: Protocol::all(),
                    token_in_address: addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    token_in_chain_id: 1,
                    token_out_address: addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    token_out_chain_id: 1,
                    amount: 1_000_000_000_000_000_000_u128.into(),
                    kind: QuoteKind::ExactIn,
                })
                .await
                .unwrap()
        );
    }
}
