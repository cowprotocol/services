//! Ox HTTP API client implementation.
//!
//! For more information on the HTTP API, consult:
//! <https://0x.org/docs/api#request-1>
//! <https://api.0x.org/>

use crate::debug_bytes;
use crate::solver_utils::{deserialize_decimal_f64, Slippage};
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use derivative::Derivative;
use ethcontract::{H160, U256};
use model::u256_decimal;
use primitive_types::H256;
use reqwest::{Client, IntoUrl, Url};
use serde::{Deserialize, Deserializer};
use std::collections::HashSet;
use thiserror::Error;
use web3::types::Bytes;

const ORDERS_MAX_PAGE_SIZE: usize = 1_000;

// 0x requires an address as an affiliate.
// Hence we hand over the settlement contract address
const AFFILIATE_ADDRESS: &str = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41";

// The `Display` implementation for `H160` unfortunately does not print
// the full address ad instead uses ellipsis (e.g. "0xeeee…eeee"). This
// helper just works around that.
fn addr2str(addr: H160) -> String {
    format!("{:#x}", addr)
}

/// A 0x API quote query parameters.
///
/// These parameters are currently incomplete, and missing parameters can be
/// added incrementally as needed.
#[derive(Clone, Copy, Debug, Default)]
pub struct SwapQuery {
    /// Contract address of a token to sell.
    pub sell_token: H160,
    /// Contract address of a token to buy.
    pub buy_token: H160,
    /// Amount of a token to sell, set in atoms.
    pub sell_amount: Option<U256>,
    /// Amount of a token to sell, set in atoms.
    pub buy_amount: Option<U256>,
    /// Limit of price slippage you are willing to accept.
    pub slippage_percentage: Slippage,
}

impl SwapQuery {
    /// Encodes the swap query as a url with get parameters.
    fn format_url(&self, base_url: &Url, endpoint: &str) -> Url {
        let mut url = base_url
            .join("/swap/v1/")
            .expect("unexpectedly invalid URL segment")
            .join(endpoint)
            .expect("unexpectedly invalid URL segment");
        url.query_pairs_mut()
            .append_pair("sellToken", &addr2str(self.sell_token))
            .append_pair("buyToken", &addr2str(self.buy_token))
            .append_pair("slippagePercentage", &self.slippage_percentage.to_string());
        if let Some(amount) = self.sell_amount {
            url.query_pairs_mut()
                .append_pair("sellAmount", &amount.to_string());
        }
        if let Some(amount) = self.buy_amount {
            url.query_pairs_mut()
                .append_pair("buyAmount", &amount.to_string());
        }
        url.query_pairs_mut()
            .append_pair("affiliateAddress", AFFILIATE_ADDRESS);
        // We do not provide a takerAddress so validation does not make sense.
        url.query_pairs_mut().append_pair("skipValidation", "true");
        // Ensure that we do not request binding quotes that we might be penalized for not taking.
        url.query_pairs_mut()
            .append_pair("intentOnFilling", "false");
        url
    }
}

/// 0x API orders query parameters.
///
/// These parameters are currently incomplete, and missing parameters can be
/// added incrementally as needed.
/// https://0x.org/docs/api#signed-order
#[derive(Clone, Debug)]
pub struct OrdersQuery {
    /// The address of the party that is allowed to fill the order.
    /// If set to a specific party, the order cannot be filled by anyone else.
    /// If left unspecified, anyone can fill the order.
    pub taker: Option<H160>,
    /// Allows the maker to enforce that the order flow through some
    /// additional logic before it can be filled (e.g., a KYC whitelist).
    pub sender: Option<H160>,
    /// Address of the contract where the transaction should be sent,
    /// usually this is the 0x exchange proxy contract.
    pub verifying_contract: Option<H160>,
}

impl OrdersQuery {
    /// Encodes the orders query as a url with parameters.
    fn format_url(&self, base_url: &Url) -> Url {
        let mut url = base_url
            .join("/orderbook/v1/orders")
            .expect("unexpectedly invalid URL segment");

        if let Some(taker) = self.taker {
            url.query_pairs_mut().append_pair("taker", &addr2str(taker));
        }
        if let Some(sender) = self.sender {
            url.query_pairs_mut()
                .append_pair("sender", &addr2str(sender));
        }
        if let Some(verifying_contract) = self.verifying_contract {
            url.query_pairs_mut()
                .append_pair("verifyingContract", &addr2str(verifying_contract));
        }

        url
    }
}

impl Default for OrdersQuery {
    fn default() -> Self {
        Self {
            taker: Some(H160::zero()),
            sender: Some(H160::zero()),
            verifying_contract: Some(DefaultZeroExApi::DEFAULT_VERIFICATION_CONTRACT),
        }
    }
}

#[derive(Debug, Derivative, Clone, Deserialize, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "camelCase")]
pub struct OrderMetaData {
    #[derivative(Default(value = "chrono::MIN_DATETIME"))]
    pub created_at: DateTime<Utc>,
    pub order_hash: Bytes,
    pub remaining_fillable_taker_amount: U256,
}

fn deserialize_epoch_timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let epoch: &str = Deserialize::deserialize(deserializer)?;
    let naive = NaiveDateTime::from_timestamp(epoch.parse().map_err(serde::de::Error::custom)?, 0);
    Ok(DateTime::from_utc(naive, Utc))
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZeroExSignature {
    r: H256,
    s: H256,
    v: u8,
    signature_type: u8,
}

#[derive(Debug, Derivative, Clone, Deserialize, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// The ID of the Ethereum chain where the `verifying_contract` is located.
    pub chain_id: u64,
    /// Timestamp in seconds of when the order expires. Expired orders cannot be filled.
    #[serde(deserialize_with = "deserialize_epoch_timestamp")]
    #[derivative(Default(value = "chrono::MAX_DATETIME"))]
    pub expiry: DateTime<Utc>,
    /// The address of the entity that will receive any fees stipulated by the order.
    /// This is typically used to incentivize off-chain order relay.
    pub fee_recipient: H160,
    /// The address of the party that creates the order. The maker is also one of the
    /// two parties that will be involved in the trade if the order gets filled.
    pub maker: H160,
    /// The amount of `maker_token` being sold by the maker.
    pub maker_amount: U256,
    /// The address of the ERC20 token the maker is selling to the taker.
    pub maker_token: H160,
    /// The staking pool to attribute the 0x protocol fee from this order. Set to zero
    /// to attribute to the default pool, not owned by anyone.
    pub pool: Bytes,
    /// A value that can be used to guarantee order uniqueness. Typically it is set
    /// to a random number.
    pub salt: String,
    /// It allows the maker to enforce that the order flow through some additional
    /// logic before it can be filled (e.g., a KYC whitelist).
    pub sender: H160,
    /// The signature of the signed order.
    pub signature: ZeroExSignature,
    /// The address of the party that is allowed to fill the order. If set to a
    /// specific party, the order cannot be filled by anyone else. If left unspecified,
    /// anyone can fill the order.
    pub taker: H160,
    /// The amount of `taker_token` being sold by the taker.
    pub taker_amount: U256,
    /// The address of the ERC20 token the taker is selling to the maker.
    pub taker_token: H160,
    /// Amount of takerToken paid by the taker to the feeRecipient.
    pub taker_token_fee_amount: U256,
    /// Address of the contract where the transaction should be sent, usually this is
    /// the 0x exchange proxy contract.
    pub verifying_contract: H160,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub meta_data: OrderMetaData,
    pub order: Order,
}

/// A Ox API `orders` response.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrdersResponse {
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub records: Vec<OrderRecord>,
}

/// A Ox API `price` response.
#[derive(Clone, Default, Derivative, Deserialize, PartialEq)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct PriceResponse {
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub allowance_target: H160,
    #[serde(deserialize_with = "deserialize_decimal_f64")]
    pub price: f64,
    #[serde(with = "u256_decimal")]
    pub estimated_gas: U256,
}

/// A Ox API `swap` response.
#[derive(Clone, Default, Derivative, Deserialize, PartialEq)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[serde(flatten)]
    pub price: PriceResponse,
    pub to: H160,
    #[derivative(Debug(format_with = "debug_bytes"))]
    pub data: Bytes,
    #[serde(with = "u256_decimal")]
    pub value: U256,
}

/// Abstract 0x API. Provides a mockable implementation.
#[mockall::automock]
#[async_trait::async_trait]
pub trait ZeroExApi: Send + Sync {
    /// Retrieve a swap for the specified parameters from the 1Inch API.
    ///
    /// See [`/swap/v1/quote`](https://0x.org/docs/api#get-swapv1quote).
    async fn get_swap(&self, query: SwapQuery) -> Result<SwapResponse, ZeroExResponseError>;

    /// Pricing for RFQT liquidity.
    /// - https://0x.org/docs/guides/rfqt-in-the-0x-api
    /// - https://0x.org/docs/api#get-swapv1price
    async fn get_price(&self, query: SwapQuery) -> Result<PriceResponse, ZeroExResponseError>;

    /// Retrieves all current limit orders.
    async fn get_orders(
        &self,
        query: &OrdersQuery,
    ) -> Result<Vec<OrderRecord>, ZeroExResponseError>;
}

/// 0x API Client implementation.
#[derive(Debug)]
pub struct DefaultZeroExApi {
    client: Client,
    base_url: Url,
    api_key: Option<String>,
}

impl DefaultZeroExApi {
    /// Default 0x API URL.
    pub const DEFAULT_URL: &'static str = "https://api.0x.org/";

    /// Default 0x verifying contract.
    /// The currently latest 0x v4 contract.
    pub const DEFAULT_VERIFICATION_CONTRACT: H160 =
        addr!("Def1C0ded9bec7F1a1670819833240f027b25EfF");

    /// Create a new 0x HTTP API client with the specified base URL.
    pub fn new(base_url: impl IntoUrl, api_key: Option<String>, client: Client) -> Result<Self> {
        Ok(Self {
            client,
            base_url: base_url.into_url().context("zeroex api url")?,
            api_key,
        })
    }

    /// Create a new 0x HTTP API client using the default URL.
    pub fn with_default_url(client: Client) -> Self {
        Self::new(Self::DEFAULT_URL, None, client).unwrap()
    }

    /// Retrieves specific page of current limit orders.
    async fn get_orders_with_pagination(
        &self,
        query: &OrdersQuery,
        results_per_page: usize,
        page: usize,
    ) -> Result<OrdersResponse, ZeroExResponseError> {
        let mut url = query.format_url(&self.base_url);
        url.query_pairs_mut()
            .append_pair("page", &page.to_string())
            .append_pair("perPage", &results_per_page.to_string());
        self.request(url).await
    }
}

impl Default for DefaultZeroExApi {
    fn default() -> Self {
        Self::new(Self::DEFAULT_URL, None, Client::new()).unwrap()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawResponse<Ok> {
    ResponseOk(Ok),
    ResponseErr { reason: String },
}

#[derive(Error, Debug)]
pub enum ZeroExResponseError {
    #[error("ServerError from query {0}")]
    ServerError(String),

    #[error("uncatalogued error message: {0}")]
    UnknownZeroExError(String),

    #[error("Error({0}) for response {1}")]
    DeserializeError(serde_json::Error, String),

    // Recovered Response but failed on async call of response.text()
    #[error(transparent)]
    TextFetch(reqwest::Error),

    // Connectivity or non-response error
    #[error("Failed on send")]
    Send(reqwest::Error),
}

#[async_trait::async_trait]
impl ZeroExApi for DefaultZeroExApi {
    async fn get_swap(&self, query: SwapQuery) -> Result<SwapResponse, ZeroExResponseError> {
        self.request(query.format_url(&self.base_url, "quote"))
            .await
    }

    async fn get_price(&self, query: SwapQuery) -> Result<PriceResponse, ZeroExResponseError> {
        self.request(query.format_url(&self.base_url, "price"))
            .await
    }

    async fn get_orders(
        &self,
        query: &OrdersQuery,
    ) -> Result<Vec<OrderRecord>, ZeroExResponseError> {
        let mut results = Vec::default();
        let mut page = 1;
        loop {
            let response = self
                .get_orders_with_pagination(query, ORDERS_MAX_PAGE_SIZE, page)
                .await?;
            if !expect_more_results_after_handling_response(&mut results, response) {
                break;
            }
            page += 1;
        }
        retain_valid_orders(&mut results);
        Ok(results)
    }
}

/// Append data of response to results and return whether another page should be fetched.
fn expect_more_results_after_handling_response(
    results: &mut Vec<OrderRecord>,
    mut response: OrdersResponse,
) -> bool {
    // only expect another page if this one was full
    let expect_more_results = response.records.len() as u64 == response.per_page;
    results.append(&mut response.records);
    expect_more_results
}

fn retain_valid_orders(orders: &mut Vec<OrderRecord>) {
    let mut included_orders = HashSet::new();
    let now = chrono::offset::Utc::now();
    orders.retain(|order| {
        // only keep orders which are still valid and unique
        order.order.expiry > now && included_orders.insert(order.meta_data.order_hash.clone())
    });
}

impl DefaultZeroExApi {
    async fn request<T: for<'a> serde::Deserialize<'a>>(
        &self,
        url: Url,
    ) -> Result<T, ZeroExResponseError> {
        tracing::debug!("Querying 0x API: {}", url);

        let mut request = self.client.get(url.clone());
        if let Some(key) = &self.api_key {
            request = request.header("0x-api-key", key);
        }
        let response_text = request
            .send()
            .await
            .map_err(ZeroExResponseError::Send)?
            .text()
            .await
            .map_err(ZeroExResponseError::TextFetch)?;
        tracing::debug!("Response from 0x API: {}", response_text);

        match serde_json::from_str::<RawResponse<T>>(&response_text) {
            Ok(RawResponse::ResponseOk(response)) => Ok(response),
            Ok(RawResponse::ResponseErr { reason: message }) => match &message[..] {
                "Server Error" => Err(ZeroExResponseError::ServerError(format!("{:?}", url))),
                _ => Err(ZeroExResponseError::UnknownZeroExError(message)),
            },
            Err(err) => Err(ZeroExResponseError::DeserializeError(
                err,
                response_text.parse().unwrap(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_api_e2e() {
        let zeroex_client = DefaultZeroExApi::default();
        let swap_query = SwapQuery {
            sell_token: testlib::tokens::WETH,
            buy_token: testlib::tokens::USDC,
            sell_amount: Some(U256::from_f64_lossy(1e18)),
            buy_amount: None,
            slippage_percentage: Slippage(0.1_f64),
        };

        let price_response = zeroex_client.get_swap(swap_query).await;
        dbg!(&price_response);
        assert!(price_response.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_api_e2e_private() {
        let url = std::env::var("ZEROEX_URL").unwrap();
        let api_key = std::env::var("ZEROEX_API_KEY").unwrap();
        let zeroex_client = DefaultZeroExApi::new(url, Some(api_key), Client::new()).unwrap();
        let swap_query = SwapQuery {
            sell_token: testlib::tokens::WETH,
            buy_token: testlib::tokens::USDC,
            sell_amount: Some(U256::from_f64_lossy(1e18)),
            buy_amount: None,
            slippage_percentage: Slippage(0.1_f64),
        };

        let price_response = zeroex_client.get_price(swap_query).await;
        dbg!(&price_response);
        assert!(price_response.is_ok());
        let swap_response = zeroex_client.get_swap(swap_query).await;
        dbg!(&swap_response);
        assert!(swap_response.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_orders() {
        let api =
            DefaultZeroExApi::new(DefaultZeroExApi::DEFAULT_URL, None, Client::new()).unwrap();
        let result = api.get_orders(&OrdersQuery::default()).await;
        dbg!(&result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_orders_paginated_with_empty_result() {
        let api =
            DefaultZeroExApi::new(DefaultZeroExApi::DEFAULT_URL, None, Client::new()).unwrap();
        // `get_orders()` relies on `get_orders_with_pagination()` not producing and error instead
        // of an response with 0 records. To test that we request a page which should never have a
        // any records and check that it doesn't throw an error.
        let result = api
            .get_orders_with_pagination(&OrdersQuery::default(), 100, 1000000)
            .await;
        dbg!(&result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_determining_end_of_paginated_results() {
        let mut results = Vec::default();
        let response = OrdersResponse {
            total: 1000,
            per_page: 1,
            page: 1,
            records: vec![OrderRecord::default()],
        };
        assert!(expect_more_results_after_handling_response(
            &mut results,
            response
        ));
        let response = OrdersResponse {
            total: 2,
            per_page: 2,
            page: 1,
            records: vec![OrderRecord::default()],
        };
        assert!(!expect_more_results_after_handling_response(
            &mut results,
            response
        ));
    }

    #[test]
    fn test_retaining_valid_orders() {
        let valid_order = OrderRecord::default();
        let mut orders = vec![
            valid_order.clone(),
            // valid but duplicate
            valid_order.clone(),
            OrderRecord {
                order: Order {
                    // already expired
                    expiry: chrono::MIN_DATETIME,
                    ..Default::default()
                },
                meta_data: OrderMetaData {
                    // unique order_hash
                    order_hash: [2].into(),
                    ..Default::default()
                },
            },
        ];
        retain_valid_orders(&mut orders);
        assert_eq!(vec![valid_order], orders);
    }

    #[test]
    fn deserialize_swap_response() {
        let swap = serde_json::from_str::<SwapResponse>(
                r#"{"price":"13.12100257517027783","guaranteedPrice":"12.98979254941857505","to":"0xdef1c0ded9bec7f1a1670819833240f027b25eff","data":"0xd9627aa40000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000016345785d8a00000000000000000000000000000000000000000000000000001206e6c0056936e100000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000006810e776880c02933d47db1b9fc05908e5386b96869584cd0000000000000000000000001000000000000000000000000000000000000011000000000000000000000000000000000000000000000092415e982f60d431ba","value":"0","gas":"111000","estimatedGas":"111000","gasPrice":"10000000000","protocolFee":"0","minimumProtocolFee":"0","buyTokenAddress":"0x6810e776880c02933d47db1b9fc05908e5386b96","sellTokenAddress":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","buyAmount":"1312100257517027783","sellAmount":"100000000000000000","sources":[{"name":"0x","proportion":"0"},{"name":"Uniswap","proportion":"0"},{"name":"Uniswap_V2","proportion":"0"},{"name":"Eth2Dai","proportion":"0"},{"name":"Kyber","proportion":"0"},{"name":"Curve","proportion":"0"},{"name":"Balancer","proportion":"0"},{"name":"Balancer_V2","proportion":"0"},{"name":"Bancor","proportion":"0"},{"name":"mStable","proportion":"0"},{"name":"Mooniswap","proportion":"0"},{"name":"Swerve","proportion":"0"},{"name":"SnowSwap","proportion":"0"},{"name":"SushiSwap","proportion":"1"},{"name":"Shell","proportion":"0"},{"name":"MultiHop","proportion":"0"},{"name":"DODO","proportion":"0"},{"name":"DODO_V2","proportion":"0"},{"name":"CREAM","proportion":"0"},{"name":"LiquidityProvider","proportion":"0"},{"name":"CryptoCom","proportion":"0"},{"name":"Linkswap","proportion":"0"},{"name":"MakerPsm","proportion":"0"},{"name":"KyberDMM","proportion":"0"},{"name":"Smoothy","proportion":"0"},{"name":"Component","proportion":"0"},{"name":"Saddle","proportion":"0"},{"name":"xSigma","proportion":"0"},{"name":"Uniswap_V3","proportion":"0"},{"name":"Curve_V2","proportion":"0"}],"orders":[{"makerToken":"0x6810e776880c02933d47db1b9fc05908e5386b96","takerToken":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","makerAmount":"1312100257517027783","takerAmount":"100000000000000000","fillData":{"tokenAddressPath":["0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","0x6810e776880c02933d47db1b9fc05908e5386b96"],"router":"0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f"},"source":"SushiSwap","sourcePathId":"0xf070a63548deb1c57a1540d63c986e01c1718a7a091d20da7020aa422c01b3de","type":0}],"allowanceTarget":"0xdef1c0ded9bec7f1a1670819833240f027b25eff","sellTokenToEthRate":"1","buyTokenToEthRate":"13.05137210499988309"}"#,
            )
            .unwrap();

        assert_eq!(
                swap,
                SwapResponse {
                    price: PriceResponse {
                        sell_amount: U256::from_dec_str("100000000000000000").unwrap(),
                        buy_amount: U256::from_dec_str("1312100257517027783").unwrap(),
                        allowance_target: crate::addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                        price: 13.121_002_575_170_278_f64,
                        estimated_gas: 111000.into(),
                    },
                    to: crate::addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    data: Bytes(hex::decode(
                        "d9627aa40000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000016345785d8a00000000000000000000000000000000000000000000000000001206e6c0056936e100000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000006810e776880c02933d47db1b9fc05908e5386b96869584cd0000000000000000000000001000000000000000000000000000000000000011000000000000000000000000000000000000000000000092415e982f60d431ba"
                    ).unwrap()),
                    value: U256::from_dec_str("0").unwrap(),
                }
            );
    }
}
