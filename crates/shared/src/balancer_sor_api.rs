//! Module for interacting with the Balancer SOR HTTP API.
//!
//! For more information how the SOR solver works, check out
//! https://dev.balancer.fi/resources/smart-order-router

use {
    crate::price_estimation::PriceEstimationError,
    anyhow::{Context, Result},
    ethcontract::{H160, H256},
    model::order::OrderKind,
    reqwest::{Client, IntoUrl, StatusCode, Url},
    serde::{Deserialize, Serialize},
};

/// Trait for mockable Balancer SOR API.
#[mockall::automock]
#[async_trait::async_trait]
pub trait BalancerSorApi: Send + Sync + 'static {
    /// Quotes a price.
    async fn quote(&self, query: Query) -> Result<Option<Quote>, Error>;
}

/// Balancer SOR API.
pub struct DefaultBalancerSorApi {
    client: Client,
    url: Url,
}

impl DefaultBalancerSorApi {
    /// Creates a new Balancer SOR API instance.
    pub fn new(client: Client, base_url: impl IntoUrl, chain_id: u64) -> Result<Self> {
        let url = crate::url::join(&base_url.into_url()?, &chain_id.to_string());
        Ok(Self { client, url })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error("Rate limited")]
    RateLimited,
}

impl From<Error> for PriceEstimationError {
    fn from(err: Error) -> Self {
        match err {
            Error::RateLimited => Self::RateLimited,
            Error::Other(err) => Self::EstimatorInternal(err),
        }
    }
}

#[async_trait::async_trait]
impl BalancerSorApi for DefaultBalancerSorApi {
    async fn quote(&self, query: Query) -> Result<Option<Quote>, Error> {
        tracing::debug!(url =% self.url, ?query, "querying Balancer SOR");
        let response = self
            .client
            .post(self.url.clone())
            .json(&query)
            .send()
            .await
            .context("request failed")?;
        let status = response.status();
        let response = response.text().await.context("fetching content failed")?;
        tracing::debug!(%response, %status, "received Balancer SOR quote");
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }

        let quote = serde_json::from_str::<Quote>(&response).context("deserialization failed")?;
        if quote.is_empty() {
            return Ok(None);
        }

        Ok(Some(quote))
    }
}

/// An SOR query.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    /// The sell token to quote.
    pub sell_token: H160,
    /// The buy token to quote.
    pub buy_token: H160,
    /// The order kind to use.
    pub order_kind: OrderKind,
    /// The amount to quote
    ///
    /// For sell orders this is the exact amount of sell token to trade, for buy
    /// orders, this is the amount of buy tokens to buy.
    pub amount: number::U256,
    /// The current gas price estimate used for determining how the trading
    /// route should be split.
    pub gas_price: number::U256,
}

/// The swap route found by the Balancer SOR service.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// The token addresses included in the swap route.
    pub token_addresses: Vec<H160>,
    /// The swap route.
    pub swaps: Vec<Swap>,
    /// The swapped token amount.
    ///
    /// In sell token for sell orders or buy token for buy orders.
    pub swap_amount: number::U256,
    /// The returned token amount.
    ///
    /// In buy token for sell orders or sell token for buy orders.
    pub return_amount: number::U256,
    /// The input (sell) token.
    #[serde(with = "address_default_when_empty")]
    pub token_in: H160,
    /// The output (buy) token.
    #[serde(with = "address_default_when_empty")]
    pub token_out: H160,
}

/// A swap included in a larger batched swap.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    /// The ID of the pool swapping in this step.
    pub pool_id: H256,
    /// The index in `token_addresses` for the input token.
    #[serde(with = "value_or_string")]
    pub asset_in_index: usize,
    /// The index in `token_addresses` for the ouput token.
    #[serde(with = "value_or_string")]
    pub asset_out_index: usize,
    /// The amount to swap.
    pub amount: number::U256,
    /// Additional user data to pass to the pool.
    #[serde(with = "model::bytes_hex")]
    pub user_data: Vec<u8>,
}

impl Quote {
    /// Check for "empty" quotes - i.e. all 0's with no swaps. Balancer SOR API
    /// returns this in case it fails to find a route for whatever reason (not
    /// enough liquidity, no trading path, etc.). We don't consider this an
    /// error case.
    fn is_empty(&self) -> bool {
        *self == Quote::default()
    }
}

/// Balancer SOR responds with `address: ""` on error cases. Instead of using an
/// `<Option<H160>>::None` just use `H160::default()` in those cases to simplify
/// using resulting `Quote`s.
mod address_default_when_empty {
    use {
        ethcontract::H160,
        serde::{de, Deserialize as _, Deserializer},
        std::borrow::Cow,
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<H160, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Cow::<str>::deserialize(deserializer)?;
        if value == "" {
            return Ok(H160::default());
        }
        value.parse().map_err(de::Error::custom)
    }
}

/// Tries to either parse the `T` directly or tries to convert the value in case
/// it's a string. This is intended for deserializing number/string but is
/// generic enough to be used for any value that can be converted from a string.
mod value_or_string {
    use {
        serde::{de, Deserialize, Deserializer},
        std::borrow::Cow,
    };

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de> + std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Content<'a, T> {
            Value(T),
            String(Cow<'a, str>),
        }

        match <Content<T>>::deserialize(deserializer) {
            Ok(Content::Value(value)) => Ok(value),
            Ok(Content::String(s)) => s.parse().map_err(de::Error::custom),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex, number::U256, serde_json::json, std::env};

    #[test]
    fn serialize_query() {
        assert_eq!(
            serde_json::to_value(Query {
                sell_token: addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                buy_token: addr!("6b175474e89094c44da98b954eedeac495271d0f"),
                order_kind: OrderKind::Sell,
                amount: 1_000_000_000_000_000_000_u128.into(),
                gas_price: 10_000_000_u128.into(),
            })
            .unwrap(),
            json!({
                "sellToken": "0xba100000625a3754423978a60c9317c58a424e3d",
                "buyToken": "0x6b175474e89094c44da98b954eedeac495271d0f",
                "orderKind": "sell",
                "amount": "1000000000000000000",
                "gasPrice": "10000000",
            }),
        );
    }

    #[test]
    fn deserialize_quote() {
        assert_eq!(
            serde_json::from_value::<Quote>(json!({
                "tokenAddresses": [
                    "0xba100000625a3754423978a60c9317c58a424e3d",
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "0x6b175474e89094c44da98b954eedeac495271d0f"
                ],
                "swaps": [
                    {
                        "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                        "assetInIndex": 0,
                        "assetOutIndex": 1,
                        "amount": "1000000000000000000",
                        "userData": "0x"
                    },
                    {
                        "poolId": "0x0b09dea16768f0799065c475be02919503cb2a3500020000000000000000001a",
                        "assetInIndex": 1,
                        "assetOutIndex": 2,
                        "amount": "0",
                        "userData": "0x"
                    }
                ],
                "swapAmount": "1000000000000000000",
                "swapAmountForSwaps": "1000000000000000000",
                "returnAmount": "15520274244171816967",
                "returnAmountFromSwaps": "15520274244171816967",
                "returnAmountConsideringFees": "15517420194930649326",
                "tokenIn": "0xba100000625a3754423978a60c9317c58a424e3d",
                "tokenOut": "0x6b175474e89094c44da98b954eedeac495271d0f",
                "marketSp": "0.0644318002071386807508916699095248"
            })).unwrap(),
            Quote {
                token_addresses: vec![
                    addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                    addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                    addr!("6b175474e89094c44da98b954eedeac495271d0f"),
                ],
                swaps: vec![
                    Swap {
                        pool_id: H256(hex!("5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014")),
                        asset_in_index: 0,
                        asset_out_index: 1,
                        amount: 1_000_000_000_000_000_000_u128.into(),
                        user_data: Default::default(),
                    },
                    Swap {
                        pool_id: H256(hex!("0b09dea16768f0799065c475be02919503cb2a3500020000000000000000001a")),
                        asset_in_index: 1,
                        asset_out_index: 2,
                        amount: 0_u32.into(),
                        user_data: Default::default(),
                    },
                ],
                swap_amount: 1_000_000_000_000_000_000_u128.into(),
                return_amount: 15_520_274_244_171_816_967_u128.into(),
                token_in: addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                token_out: addr!("6b175474e89094c44da98b954eedeac495271d0f"),
            },
        );
    }

    #[test]
    fn deserialize_empty_quote() {
        assert!(serde_json::from_value::<Quote>(json!({
            "tokenAddresses": [],
            "swaps": [],
            "swapAmount": "0",
            "swapAmountForSwaps": "0",
            "returnAmount": "0",
            "returnAmountFromSwaps": "0",
            "returnAmountConsideringFees": "0",
            "tokenIn": "",
            "tokenOut": "",
            "marketSp": "0",
        }))
        .unwrap()
        .is_empty());
    }

    #[test]
    fn deserializes_negative_and_large_return_amount_after_fee_values() {
        for amount in [
            "-1337",
            "10000000000000000000000000000000000000000000000000000000000000000000000000000000",
        ] {
            assert!(U256::from_dec_str(amount).is_err());
            assert!(serde_json::from_value::<Quote>(json!({
                "tokenAddresses": [],
                "swaps": [],
                "swapAmount": "0",
                "swapAmountForSwaps": "0",
                "returnAmount": "0",
                "returnAmountFromSwaps": "0",
                "returnAmountConsideringFees": amount,
                "tokenIn": "",
                "tokenOut": "",
                "marketSp": "0",
            }))
            .is_ok());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_sor_quote() {
        let url = env::var("BALANCER_SOR_URL").unwrap();
        let api = DefaultBalancerSorApi::new(Client::new(), url, 1).unwrap();

        fn base(atoms: U256) -> String {
            let base = atoms.to_f64_lossy() / 1e18;
            format!("{base:.6}")
        }

        let sell_quote = api
            .quote(Query {
                sell_token: addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                buy_token: addr!("6b175474e89094c44da98b954eedeac495271d0f"),
                order_kind: OrderKind::Sell,
                amount: 1_000_000_000_000_000_000_u128.into(),
                gas_price: 10_000_000_u128.into(),
            })
            .await
            .unwrap()
            .unwrap();
        println!("Sell 1.0 BAL for {:.4} DAI", base(sell_quote.return_amount));

        let buy_quote = api
            .quote(Query {
                sell_token: addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                buy_token: addr!("6b175474e89094c44da98b954eedeac495271d0f"),
                order_kind: OrderKind::Buy,
                amount: 100_000_000_000_000_000_000_u128.into(),
                gas_price: 10_000_000_u128.into(),
            })
            .await
            .unwrap()
            .unwrap();
        println!("Buy {:.4} BAL for 100.0 DAI", base(buy_quote.return_amount));
    }

    #[test]
    fn deserialize_value_or_string() {
        #[derive(Deserialize)]
        struct TestType {
            #[serde(with = "value_or_string")]
            value: usize,
        }
        let from_string: TestType = serde_json::from_value(json!({"value": "12"})).unwrap();
        assert_eq!(from_string.value, 12);
        let from_number: TestType = serde_json::from_value(json!({"value": 12})).unwrap();
        assert_eq!(from_number.value, 12);
    }
}
