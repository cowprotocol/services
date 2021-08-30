//! 1Inch HTTP API client implementation.
//!
//! For more information on the HTTP API, consult:
//! <https://docs.1inch.io/api/quote-swap>
//! <https://api.1inch.exchange/swagger/ethereum/>
use crate::solver::{
    single_order_solver::SettlementError,
    solver_utils::{deserialize_prefixed_hex, Slippage},
};
use anyhow::{anyhow, ensure, Context, Result};
use derive_more::From;
use ethcontract::{H160, U256};
use model::u256_decimal;
use reqwest::{Client, IntoUrl, Url};
use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

/// Parts to split a swap.
///
/// This type is generic on the maximum number of splits allowed.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Amount<const MIN: usize, const MAX: usize>(usize);

impl<const MIN: usize, const MAX: usize> Amount<MIN, MAX> {
    /// Creates a parts amount from the specified count.
    pub fn new(amount: usize) -> Result<Self> {
        // 1Inch API only accepts a slippage from 0 to 50.
        ensure!(
            (MIN..=MAX).contains(&amount),
            "parts outside of [{}, {}] range",
            MIN,
            MAX,
        );
        Ok(Amount(amount))
    }
}

impl<const MIN: usize, const MAX: usize> Display for Amount<MIN, MAX> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A 1Inch API quote query parameters.
///
/// These parameters are currently incomplete, and missing parameters can be
/// added incrementally as needed.
#[derive(Clone, Debug)]
pub struct SwapQuery {
    /// Contract address of a token to sell.
    pub from_token_address: H160,
    /// Contract address of a token to buy.
    pub to_token_address: H160,
    /// Amount of a token to sell, set in atoms.
    pub amount: U256,
    /// Address of a seller.
    ///
    /// Make sure that this address has approved to spend `from_token_address`
    /// in needed amount.
    pub from_address: H160,
    /// Limit of price slippage you are willing to accept.
    pub slippage: Slippage,
    /// List of protocols to use for the swap.
    pub protocols: Option<Vec<String>>,
    /// Flag to disable checks of the required quantities.
    pub disable_estimate: Option<bool>,
    /// Maximum number of token-connectors to be used in a transaction.
    pub complexity_level: Option<Amount<0, 3>>,
    /// Maximum amount of gas for a swap.
    pub gas_limit: Option<u64>,
    /// Limit maximum number of main route parts.
    pub main_route_parts: Option<Amount<1, 50>>,
    /// Limit maximum number of parts each main route part can be split into.
    pub parts: Option<Amount<1, 100>>,
}

impl SwapQuery {
    /// Encodes the swap query as
    fn into_url(self, base_url: &Url) -> Url {
        // The `Display` implementation for `H160` unfortunately does not print
        // the full address and instead uses ellipsis (e.g. "0xeeee…eeee"). This
        // helper just works around that.
        fn addr2str(addr: H160) -> String {
            format!("{:?}", addr)
        }

        let mut url = base_url
            .join("v3.0/1/swap")
            .expect("unexpectedly invalid URL segment");
        url.query_pairs_mut()
            .append_pair("fromTokenAddress", &addr2str(self.from_token_address))
            .append_pair("toTokenAddress", &addr2str(self.to_token_address))
            .append_pair("amount", &self.amount.to_string())
            .append_pair("fromAddress", &addr2str(self.from_address))
            .append_pair("slippage", &self.slippage.to_string());

        if let Some(protocols) = self.protocols {
            url.query_pairs_mut()
                .append_pair("protocols", &protocols.join(","));
        }
        if let Some(disable_estimate) = self.disable_estimate {
            url.query_pairs_mut()
                .append_pair("disableEstimate", &disable_estimate.to_string());
        }
        if let Some(complexity_level) = self.complexity_level {
            url.query_pairs_mut()
                .append_pair("complexityLevel", &complexity_level.to_string());
        }
        if let Some(gas_limit) = self.gas_limit {
            url.query_pairs_mut()
                .append_pair("gasLimit", &gas_limit.to_string());
        }
        if let Some(main_route_parts) = self.main_route_parts {
            url.query_pairs_mut()
                .append_pair("mainRouteParts", &main_route_parts.to_string());
        }
        if let Some(parts) = self.parts {
            url.query_pairs_mut()
                .append_pair("parts", &parts.to_string());
        }

        url
    }
}

/// A 1Inch API swap response.
#[derive(Clone, Debug, Deserialize, PartialEq, From)]
#[serde(untagged)]
pub enum SwapResponse {
    Swap(Swap),
    Error(SwapResponseError),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseError {
    pub status_code: u32,
    pub message: String,
}

impl From<SwapResponseError> for SettlementError {
    fn from(error: SwapResponseError) -> Self {
        SettlementError {
            inner: anyhow!(error.message),
            retryable: matches!(error.status_code, 500),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    pub from_token: Token,
    pub to_token: Token,
    #[serde(with = "u256_decimal")]
    pub from_token_amount: U256,
    #[serde(with = "u256_decimal")]
    pub to_token_amount: U256,
    pub protocols: Vec<Vec<Vec<Protocol>>>,
    pub tx: Transaction,
}

/// Metadata associated with a token.
///
/// The response data is currently incomplete, and missing fields can be added
/// incrementally as needed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Default)]
pub struct Token {
    pub address: H160,
}

/// Metadata associated with a protocol used for part of a 1Inch swap.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Protocol {
    pub name: String,
    pub part: f64,
    pub from_token_address: H160,
    pub to_token_address: H160,
}

/// Swap transaction generated by the 1Inch API.
#[derive(Clone, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub from: H160,
    pub to: H160,
    #[serde(deserialize_with = "deserialize_prefixed_hex")]
    pub data: Vec<u8>,
    #[serde(with = "u256_decimal")]
    pub value: U256,
    #[serde(with = "u256_decimal")]
    pub gas_price: U256,
    pub gas: u64,
}

impl std::fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transaction")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("data", &format_args!("0x{}", hex::encode(&self.data)))
            .field("value", &self.value)
            .field("gas_price", &self.gas_price)
            .field("gas", &self.gas)
            .finish()
    }
}
/// Approve spender response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Spender {
    pub address: H160,
}

/// Protocols query response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Protocols {
    pub protocols: Vec<String>,
}

// Mockable version of API Client
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OneInchClient: Send + Sync {
    /// Retrieves a swap for the specified parameters from the 1Inch API.
    async fn get_swap(&self, query: SwapQuery) -> Result<SwapResponse>;

    /// Retrieves the address of the spender to use for token approvals.
    async fn get_spender(&self) -> Result<Spender>;

    /// Retrieves a list of the on-chain protocols supported by 1Inch.
    async fn get_protocols(&self) -> Result<Protocols>;
}

/// 1Inch API Client implementation.
#[derive(Debug)]
pub struct OneInchClientImpl {
    client: Client,
    base_url: Url,
}

impl OneInchClientImpl {
    pub const DEFAULT_URL: &'static str = "https://api.1inch.exchange/";

    /// Create a new 1Inch HTTP API client with the specified base URL.
    pub fn new(base_url: impl IntoUrl, client: Client) -> Result<Self> {
        Ok(Self {
            client,
            base_url: base_url.into_url()?,
        })
    }
}

#[async_trait::async_trait]
impl OneInchClient for OneInchClientImpl {
    async fn get_swap(&self, query: SwapQuery) -> Result<SwapResponse> {
        logged_query(&self.client, query.into_url(&self.base_url)).await
    }

    async fn get_spender(&self) -> Result<Spender> {
        let url = self
            .base_url
            .join("v3.0/1/approve/spender")
            .expect("unexpectedly invalid URL");
        logged_query(&self.client, url).await
    }

    async fn get_protocols(&self) -> Result<Protocols> {
        let url = self
            .base_url
            .join("v3.0/1/protocols")
            .expect("unexpectedly invalid URL");
        logged_query(&self.client, url).await
    }
}

async fn logged_query<D>(client: &Client, url: Url) -> Result<D>
where
    D: for<'de> Deserialize<'de>,
{
    tracing::debug!("Query 1inch API for url {}", url);
    let response = client.get(url).send().await?.text().await;
    tracing::debug!("Response from 1inch API: {:?}", response);
    serde_json::from_str(&response?).context("1inch result parsing failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slippage_from_basis_points() {
        assert_eq!(
            Slippage::percentage_from_basis_points(50).unwrap(),
            Slippage::percentage(0.5).unwrap(),
        )
    }

    #[test]
    fn slippage_out_of_range() {
        assert!(Slippage::percentage(-1.).is_err());
        assert!(Slippage::percentage(1337.).is_err());
    }

    #[test]
    fn amounts_valid_range() {
        assert!(Amount::<42, 1337>::new(41).is_err());
        assert!(Amount::<42, 1337>::new(42).is_ok());
        assert!(Amount::<42, 1337>::new(1337).is_ok());
        assert!(Amount::<42, 1337>::new(1338).is_err());
    }

    #[test]
    fn swap_query_serialization() {
        let base_url = Url::parse("https://api.1inch.exchange/").unwrap();
        let url = SwapQuery {
            from_token_address: shared::addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
            to_token_address: shared::addr!("111111111117dc0aa78b770fa6a738034120c302"),
            amount: 1_000_000_000_000_000_000u128.into(),
            from_address: shared::addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
            slippage: Slippage::percentage_from_basis_points(50).unwrap(),
            protocols: None,
            disable_estimate: None,
            complexity_level: None,
            gas_limit: None,
            main_route_parts: None,
            parts: None,
        }
        .into_url(&base_url);

        assert_eq!(
            url.as_str(),
            "https://api.1inch.exchange/v3.0/1/swap\
                ?fromTokenAddress=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\
                &toTokenAddress=0x111111111117dc0aa78b770fa6a738034120c302\
                &amount=1000000000000000000\
                &fromAddress=0x00000000219ab540356cbb839cbe05303d7705fa\
                &slippage=0.5",
        );
    }

    #[test]
    fn swap_query_serialization_options_parameters() {
        let base_url = Url::parse("https://api.1inch.exchange/").unwrap();
        let url = SwapQuery {
            from_token_address: shared::addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
            to_token_address: shared::addr!("111111111117dc0aa78b770fa6a738034120c302"),
            amount: 1_000_000_000_000_000_000u128.into(),
            from_address: shared::addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
            slippage: Slippage::percentage_from_basis_points(50).unwrap(),
            protocols: Some(vec!["WETH".to_string(), "UNISWAP_V3".to_string()]),
            disable_estimate: Some(true),
            complexity_level: Some(Amount::new(1).unwrap()),
            gas_limit: Some(133700),
            main_route_parts: Some(Amount::new(28).unwrap()),
            parts: Some(Amount::new(42).unwrap()),
        }
        .into_url(&base_url);

        assert_eq!(
            url.as_str(),
            "https://api.1inch.exchange/v3.0/1/swap\
                ?fromTokenAddress=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\
                &toTokenAddress=0x111111111117dc0aa78b770fa6a738034120c302\
                &amount=1000000000000000000\
                &fromAddress=0x00000000219ab540356cbb839cbe05303d7705fa\
                &slippage=0.5\
                &protocols=WETH%2CUNISWAP_V3\
                &disableEstimate=true\
                &complexityLevel=1\
                &gasLimit=133700\
                &mainRouteParts=28\
                &parts=42",
        );
    }

    #[test]
    fn deserialize_swap_response() {
        let swap = serde_json::from_str::<SwapResponse>(
            r#"{
              "fromToken": {
                "symbol": "ETH",
                "name": "Ethereum",
                "decimals": 18,
                "address": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "logoURI": "https://tokens.1inch.exchange/0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee.png"
              },
              "toToken": {
                "symbol": "1INCH",
                "name": "1INCH Token",
                "decimals": 18,
                "address": "0x111111111117dc0aa78b770fa6a738034120c302",
                "logoURI": "https://tokens.1inch.exchange/0x111111111117dc0aa78b770fa6a738034120c302.png"
              },
              "toTokenAmount": "501739725821378713485",
              "fromTokenAmount": "1000000000000000000",
              "protocols": [
                [
                  [
                    {
                      "name": "WETH",
                      "part": 100,
                      "fromTokenAddress": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                      "toTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    }
                  ],
                  [
                    {
                      "name": "UNISWAP_V2",
                      "part": 100,
                      "fromTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                      "toTokenAddress": "0x111111111117dc0aa78b770fa6a738034120c302"
                    }
                  ]
                ]
              ],
              "tx": {
                "from": "0x00000000219ab540356cBB839Cbe05303d7705Fa",
                "to": "0x11111112542d85b3ef69ae05771c2dccff4faa26",
                "data": "0x2e95b6c800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000001b1038e63128bd548d0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000180000000000000003b6d034026aad2da94c59524ac0d93f6d6cbf9071d7086f2",
                "value": "1000000000000000000",
                "gasPrice": "154110000000",
                "gas": 143297
              }
            }"#,
        )
        .unwrap();

        assert_eq!(
            swap,
            SwapResponse::Swap(Swap {
                from_token: Token {
                    address: shared::addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
                },
                to_token: Token {
                    address: shared::addr!("111111111117dc0aa78b770fa6a738034120c302"),
                },
                from_token_amount: 1_000_000_000_000_000_000u128.into(),
                to_token_amount: 501_739_725_821_378_713_485u128.into(),
                protocols: vec![vec![
                    vec![Protocol {
                        name: "WETH".to_owned(),
                        part: 100.,
                        from_token_address: shared::addr!(
                            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                        ),
                        to_token_address: shared::addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                    }],
                    vec![Protocol {
                        name: "UNISWAP_V2".to_owned(),
                        part: 100.,
                        from_token_address: shared::addr!(
                            "c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        ),
                        to_token_address: shared::addr!("111111111117dc0aa78b770fa6a738034120c302"),
                    }],
                ]],
                tx: Transaction {
                    from: shared::addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
                    to: shared::addr!("11111112542d85b3ef69ae05771c2dccff4faa26"),
                    data: hex::decode(
                        "2e95b6c8\
                         0000000000000000000000000000000000000000000000000000000000000000\
                         0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                         00000000000000000000000000000000000000000000001b1038e63128bd548d\
                         0000000000000000000000000000000000000000000000000000000000000080\
                         0000000000000000000000000000000000000000000000000000000000000001\
                         80000000000000003b6d034026aad2da94c59524ac0d93f6d6cbf9071d7086f2"
                    )
                    .unwrap(),
                    value: 1_000_000_000_000_000_000u128.into(),
                    gas_price: 154_110_000_000u128.into(),
                    gas: 143297,
                },
            })
        );

        let swap_error = serde_json::from_str::<SwapResponse>(
            r#"{
            "statusCode":500,
            "message":"Internal server error"
        }"#,
        )
        .unwrap();

        assert_eq!(
            swap_error,
            SwapResponse::Error(SwapResponseError {
                status_code: 500,
                message: "Internal server error".into()
            })
        );
    }

    #[test]
    fn deserialize_spender_response() {
        let spender = serde_json::from_str::<Spender>(
            r#"{
              "address": "0x11111112542d85b3ef69ae05771c2dccff4faa26"
            }"#,
        )
        .unwrap();

        assert_eq!(
            spender,
            Spender {
                address: shared::addr!("11111112542d85b3ef69ae05771c2dccff4faa26"),
            }
        )
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_swap() {
        let swap = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new())
            .unwrap()
            .get_swap(SwapQuery {
                from_token_address: shared::addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                to_token_address: shared::addr!("111111111117dc0aa78b770fa6a738034120c302"),
                amount: 1_000_000_000_000_000_000u128.into(),
                from_address: shared::addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
                slippage: Slippage::percentage_from_basis_points(50).unwrap(),
                protocols: None,
                disable_estimate: None,
                complexity_level: None,
                gas_limit: None,
                main_route_parts: None,
                parts: None,
            })
            .await
            .unwrap();
        println!("{:#?}", swap);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_swap_fully_parameterized() {
        let swap = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new())
            .unwrap()
            .get_swap(SwapQuery {
                from_token_address: shared::addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                to_token_address: shared::addr!("a3BeD4E1c75D00fa6f4E5E6922DB7261B5E9AcD2"),
                amount: 100_000_000_000_000_000_000u128.into(),
                from_address: shared::addr!("4e608b7da83f8e9213f554bdaa77c72e125529d0"),
                slippage: Slippage::percentage_from_basis_points(50).unwrap(),
                protocols: Some(vec!["WETH".to_string(), "UNISWAP_V2".to_string()]),
                disable_estimate: Some(true),
                complexity_level: Some(Amount::new(2).unwrap()),
                gas_limit: Some(750_000),
                main_route_parts: Some(Amount::new(3).unwrap()),
                parts: Some(Amount::new(3).unwrap()),
            })
            .await
            .unwrap();
        println!("{:#?}", swap);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_protocols() {
        let protocols = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new())
            .unwrap()
            .get_protocols()
            .await
            .unwrap();
        println!("{:#?}", protocols);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_spender_address() {
        let spender = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new())
            .unwrap()
            .get_spender()
            .await
            .unwrap();
        println!("{:#?}", spender);
    }
}
