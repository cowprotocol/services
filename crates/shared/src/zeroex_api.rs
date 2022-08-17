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
use ethcontract::{H160, H256, U256};
use model::u256_decimal;
use reqwest::{Client, IntoUrl, Url};
use serde::Deserialize;
use std::cmp;
use std::collections::HashSet;
use thiserror::Error;

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
#[derive(Clone, Debug, Default)]
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
    /// List of sources to exclude.
    pub excluded_sources: Vec<String>,
    /// Requests trade routes which aim to protect against high slippage and MEV attacks.
    pub enable_slippage_protection: bool,
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
            .append_pair("slippagePercentage", &self.slippage_percentage.to_string())
            .append_pair(
                "enableSlippageProtection",
                &self.enable_slippage_protection.to_string(),
            );
        if let Some(amount) = self.sell_amount {
            url.query_pairs_mut()
                .append_pair("sellAmount", &amount.to_string());
        }
        if let Some(amount) = self.buy_amount {
            url.query_pairs_mut()
                .append_pair("buyAmount", &amount.to_string());
        }
        if !self.excluded_sources.is_empty() {
            url.query_pairs_mut()
                .append_pair("excludedSources", &self.excluded_sources.join(","));
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
pub struct OrderMetadata {
    #[derivative(Default(value = "chrono::MIN_DATETIME"))]
    pub created_at: DateTime<Utc>,
    #[serde(with = "model::bytes_hex")]
    pub order_hash: Vec<u8>,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub remaining_fillable_taker_amount: u128,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZeroExSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
    pub signature_type: u8,
}

#[derive(Debug, Derivative, Clone, Deserialize, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// The ID of the Ethereum chain where the `verifying_contract` is located.
    pub chain_id: u64,
    /// Timestamp in seconds of when the order expires. Expired orders cannot be filled.
    #[derivative(Default(value = "chrono::naive::MAX_DATETIME.timestamp() as u64"))]
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub expiry: u64,
    /// The address of the entity that will receive any fees stipulated by the order.
    /// This is typically used to incentivize off-chain order relay.
    pub fee_recipient: H160,
    /// The address of the party that creates the order. The maker is also one of the
    /// two parties that will be involved in the trade if the order gets filled.
    pub maker: H160,
    /// The amount of `maker_token` being sold by the maker.
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub maker_amount: u128,
    /// The address of the ERC20 token the maker is selling to the taker.
    pub maker_token: H160,
    /// The staking pool to attribute the 0x protocol fee from this order. Set to zero
    /// to attribute to the default pool, not owned by anyone.
    pub pool: H256,
    /// A value that can be used to guarantee order uniqueness. Typically it is set
    /// to a random number.
    #[serde(with = "u256_decimal")]
    pub salt: U256,
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
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub taker_amount: u128,
    /// The address of the ERC20 token the taker is selling to the maker.
    pub taker_token: H160,
    /// Amount of takerToken paid by the taker to the feeRecipient.
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub taker_token_fee_amount: u128,
    /// Address of the contract where the transaction should be sent, usually this is
    /// the 0x exchange proxy contract.
    pub verifying_contract: H160,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct OrderRecord {
    #[serde(rename = "metaData")]
    pub metadata: OrderMetadata,
    pub order: Order,
}

impl OrderRecord {
    /// Scales the `maker_amount` according to how much of the partially fillable
    /// amount was already used.
    pub fn remaining_maker_amount(&self) -> Result<u128> {
        if self.metadata.remaining_fillable_taker_amount > self.order.taker_amount {
            anyhow::bail!("remaining taker amount bigger than total taker amount");
        }

        // all numbers are at most u128::MAX so none of these operations can overflow
        let scaled_maker_amount = U256::from(self.order.maker_amount)
            * U256::from(self.metadata.remaining_fillable_taker_amount)
            / U256::from(self.order.taker_amount);

        // `scaled_maker_amount` is at most as big as `maker_amount` which already fits in an u128
        Ok(scaled_maker_amount.as_u128())
    }
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
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub estimated_gas: u64,
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
    #[serde(with = "model::bytes_hex")]
    pub data: Vec<u8>,
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
        let expiry = NaiveDateTime::from_timestamp(order.order.expiry as i64, 0);
        let expiry: DateTime<Utc> = DateTime::from_utc(expiry, Utc);

        // only keep orders which are still valid and unique
        expiry > now && included_orders.insert(order.metadata.order_hash.clone())
    });
}

const MAX_RESPONSE_LOG_LENGTH: usize = 10 * 1024; // 10 KB.

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

        // 0x responses are HUGE when querying limit orders. This causes issues
        // when storing logs with Kibana/Elastic Search, so trim them
        {
            let sliced = if response_text.len() > MAX_RESPONSE_LOG_LENGTH {
                "..."
            } else {
                ""
            };
            let sliced_response_text = response_text
                .get(..cmp::min(response_text.len(), MAX_RESPONSE_LOG_LENGTH))
                // This can happen only if we slice in the middle of a UTF-8
                // codepoint. Since 0x limit order response is just ASCII, this
                // should never happen. We deal with this case to avoid panics,
                // but it isn't worth handling it more completely (by finding
                // the start/end of the UTF-8 codepoint for example) since it
                // really should never happen.
                .unwrap_or("UTF-8 SLICE ERROR");
            tracing::debug!("Respnse from 0x API: {sliced_response_text}{sliced}");
        }

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
    use crate::addr;
    use chrono::TimeZone;

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
            excluded_sources: Vec::new(),
            enable_slippage_protection: false,
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
            excluded_sources: Vec::new(),
            enable_slippage_protection: false,
        };

        let price_response = zeroex_client.get_price(swap_query.clone()).await;
        dbg!(&price_response);
        assert!(price_response.is_ok());
        let swap_response = zeroex_client.get_swap(swap_query).await;
        dbg!(&swap_response);
        assert!(swap_response.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn excluded_sources() {
        let zeroex = DefaultZeroExApi::default();
        let query = SwapQuery {
            sell_token: testlib::tokens::WETH,
            buy_token: addr!("c011a73ee8576fb46f5e1c5751ca3b9fe0af2a6f"), // SNX
            sell_amount: Some(U256::from_f64_lossy(1000e18)),
            buy_amount: None,
            slippage_percentage: Slippage(0.1_f64),
            excluded_sources: Vec::new(),
            enable_slippage_protection: false,
        };

        let swap = zeroex.get_swap(query.clone()).await;
        dbg!(&swap);
        assert!(swap.is_ok());

        let swap = zeroex
            .get_swap(SwapQuery {
                excluded_sources: vec!["Balancer_V2".to_string()],
                ..query
            })
            .await;
        dbg!(&swap);
        assert!(swap.is_ok());
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
                    expiry: 0,
                    ..Default::default()
                },
                metadata: OrderMetadata {
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
    fn deserialize_orders_response() {
        let orders = serde_json::from_str::<OrdersResponse>(
            r#"{"total":1015,"page":1,"perPage":1000,"records":[{"order":{"signature":{"signatureType":3,"r":"0xdb60e4fa2b4f2ee073d88eed3502149ba2231d699bc5d92d5627dcd21f915237","s":"0x4cb1e9c15788b86d5187b99c0d929ad61d2654c242095c26f9ace17e64aca0fd","v":28},"sender":"0x0000000000000000000000000000000000000000","maker":"0x683b2388d719e98874d1f9c16b42a7bb498efbeb","taker":"0x0000000000000000000000000000000000000000","takerTokenFeeAmount":"0","makerAmount":"500000000","takerAmount":"262467000000000000","makerToken":"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48","takerToken":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","salt":"1645858724","verifyingContract":"0xdef1c0ded9bec7f1a1670819833240f027b25eff","feeRecipient":"0x86003b044f70dac0abc80ac8957305b6370893ed","expiry":"1646463524","chainId":1,"pool":"0x0000000000000000000000000000000000000000000000000000000000000000"},"metaData":{"orderHash":"0x003427369d4c2a6b0aceeb7b315bb9a6086bc6fc4c887aa51efc73b662c9d127","remainingFillableTakerAmount":"262467000000000000","createdAt":"2022-02-26T06:59:00.440Z"}}]}"#,
        ).unwrap();
        assert_eq!(
            orders,
            OrdersResponse {
                total: 1015,
                page: 1,
                per_page: 1000,
                records: vec![OrderRecord {
                    metadata: OrderMetadata {
                        order_hash:
                            hex::decode(
                                "003427369d4c2a6b0aceeb7b315bb9a6086bc6fc4c887aa51efc73b662c9d127"
                            ).unwrap(),
                        remaining_fillable_taker_amount: 262467000000000000u128,
                        created_at: Utc.ymd(2022, 2, 26).and_hms_milli(6, 59, 0, 440)
                    },
                    order: Order {
                        chain_id: 1u64,
                        expiry: 1646463524u64,
                        fee_recipient: addr!("86003b044f70dac0abc80ac8957305b6370893ed"),
                        maker: addr!("683b2388d719e98874d1f9c16b42a7bb498efbeb"),
                        maker_amount: 500000000u128,
                        maker_token: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                        pool: H256::zero(),
                        salt: 1645858724.into(),
                        sender: H160::zero(),
                        signature: ZeroExSignature {
                            signature_type: 3,
                            r: H256::from_slice(
                                &hex::decode("db60e4fa2b4f2ee073d88eed3502149ba2231d699bc5d92d5627dcd21f915237")
                                    .unwrap()
                            ),
                            s: H256::from_slice(
                                &hex::decode("4cb1e9c15788b86d5187b99c0d929ad61d2654c242095c26f9ace17e64aca0fd")
                                    .unwrap()
                            ),
                            v: 28u8,
                        },
                        taker: H160::zero(),
                        taker_amount: 262467000000000000u128,
                        taker_token: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                        taker_token_fee_amount: 0u128,
                        verifying_contract: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    }
                }],
            }
        );
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
                        estimated_gas: 111000,
                    },
                    to: crate::addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    data: hex::decode(
                        "d9627aa40000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000016345785d8a00000000000000000000000000000000000000000000000000001206e6c0056936e100000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000006810e776880c02933d47db1b9fc05908e5386b96869584cd0000000000000000000000001000000000000000000000000000000000000011000000000000000000000000000000000000000000000092415e982f60d431ba"
                    ).unwrap(),
                    value: U256::from_dec_str("0").unwrap(),
                }
            );
    }

    #[test]
    fn compute_remaining_maker_amount() {
        let bogous_order = OrderRecord {
            order: Order {
                taker_amount: u128::MAX - 1,
                maker_amount: u128::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                // remaining amount bigger than total amount
                remaining_fillable_taker_amount: u128::MAX,
                ..Default::default()
            },
        };
        assert!(bogous_order.remaining_maker_amount().is_err());

        let biggest_unfilled_order = OrderRecord {
            order: Order {
                taker_amount: u128::MAX,
                maker_amount: u128::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                remaining_fillable_taker_amount: u128::MAX,
                ..Default::default()
            },
        };
        assert_eq!(
            u128::MAX,
            // none of the operations overflow with u128::MAX for all values
            biggest_unfilled_order.remaining_maker_amount().unwrap()
        );

        let biggest_partially_filled_order = OrderRecord {
            order: Order {
                taker_amount: u128::MAX,
                maker_amount: u128::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                remaining_fillable_taker_amount: u128::MAX / 2,
                ..Default::default()
            },
        };
        assert_eq!(
            u128::MAX / 2,
            biggest_partially_filled_order
                .remaining_maker_amount()
                .unwrap()
        );
    }
}
