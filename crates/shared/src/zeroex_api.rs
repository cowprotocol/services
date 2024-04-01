//! Ox HTTP API client implementation.
//!
//! For more information on the HTTP API, consult:
//! <https://0x.org/docs/api#request-1>
//! <https://api.0x.org/>

use {
    crate::{
        debug_bytes,
        interaction::{EncodedInteraction, Interaction},
    },
    anyhow::{Context, Result},
    chrono::{DateTime, NaiveDateTime, TimeZone, Utc},
    derivative::Derivative,
    ethcontract::{Bytes, H160, H256, U256},
    ethrpc::current_block::{BlockInfo, CurrentBlockStream},
    number::serialization::HexOrDecimalU256,
    reqwest::{
        header::{HeaderMap, HeaderValue},
        Client,
        ClientBuilder,
        IntoUrl,
        StatusCode,
        Url,
    },
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    std::{
        collections::HashSet,
        fmt::{self, Display, Formatter},
    },
    thiserror::Error,
    tokio::sync::watch,
};

const ORDERS_MAX_PAGE_SIZE: usize = 1_000;

// 0x requires an address as an affiliate.
// Hence we hand over the settlement contract address
const AFFILIATE_ADDRESS: &str = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41";

// The `Display` implementation for `H160` unfortunately does not print
// the full address ad instead uses ellipsis (e.g. "0xeeee…eeee"). This
// helper just works around that.
fn addr2str(addr: H160) -> String {
    format!("{addr:#x}")
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
    pub slippage_percentage: Option<Slippage>,
    /// The taker address to use.
    pub taker_address: Option<H160>,
    /// List of sources to exclude.
    pub excluded_sources: Vec<String>,
    /// Wether or not the taker intends on filling the quote.
    pub intent_on_filling: bool,
    /// Requests trade routes which aim to protect against high slippage and MEV
    /// attacks.
    pub enable_slippage_protection: bool,
}

impl SwapQuery {
    /// Encodes the swap query as a url with get parameters.
    fn format_url(&self, base_url: &Url, endpoint: &str) -> Url {
        let mut url = crate::url::join(base_url, &format!("swap/v1/{endpoint}"));
        url.query_pairs_mut()
            .append_pair("sellToken", &addr2str(self.sell_token))
            .append_pair("buyToken", &addr2str(self.buy_token))
            .append_pair("intentOnFilling", &self.intent_on_filling.to_string())
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
        if let Some(slippage_percentage) = self.slippage_percentage {
            url.query_pairs_mut()
                .append_pair("slippagePercentage", &slippage_percentage.to_string());
        }
        if let Some(taker) = self.taker_address {
            url.query_pairs_mut()
                .append_pair("takerAddress", &addr2str(taker));
        }
        if !self.excluded_sources.is_empty() {
            url.query_pairs_mut()
                .append_pair("excludedSources", &self.excluded_sources.join(","));
        }
        url.query_pairs_mut()
            .append_pair("affiliateAddress", AFFILIATE_ADDRESS);
        // We do not provide a takerAddress so validation does not make sense.
        url.query_pairs_mut().append_pair("skipValidation", "true");
        url
    }
}

/// A 0x slippage amount.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default)]
pub struct Slippage(f64);

impl Slippage {
    pub const ONE_PERCENT: Self = Self(0.01);

    /// Creates a slippage amount from the specified slippage factor.
    pub fn new(factor: f64) -> Self {
        Slippage(factor)
    }
}

impl Display for Slippage {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
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
        let mut url = crate::url::join(base_url, "/orderbook/v1/orders");

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

#[serde_as]
#[derive(Debug, Derivative, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "camelCase")]
pub struct OrderMetadata {
    #[derivative(Default(value = "DateTime::<Utc>::MIN_UTC"))]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bytes_hex")]
    pub order_hash: Vec<u8>,
    #[serde_as(as = "DisplayFromStr")]
    pub remaining_fillable_taker_amount: u128,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZeroExSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
    pub signature_type: u8,
}

#[serde_as]
#[derive(Debug, Derivative, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// The ID of the Ethereum chain where the `verifying_contract` is located.
    pub chain_id: u64,
    /// Timestamp in seconds of when the order expires. Expired orders cannot be
    /// filled.
    #[derivative(Default(value = "NaiveDateTime::MAX.timestamp() as u64"))]
    #[serde_as(as = "DisplayFromStr")]
    pub expiry: u64,
    /// The address of the entity that will receive any fees stipulated by the
    /// order. This is typically used to incentivize off-chain order relay.
    pub fee_recipient: H160,
    /// The address of the party that creates the order. The maker is also one
    /// of the two parties that will be involved in the trade if the order
    /// gets filled.
    pub maker: H160,
    /// The amount of `maker_token` being sold by the maker.
    #[serde_as(as = "DisplayFromStr")]
    pub maker_amount: u128,
    /// The address of the ERC20 token the maker is selling to the taker.
    pub maker_token: H160,
    /// The staking pool to attribute the 0x protocol fee from this order. Set
    /// to zero to attribute to the default pool, not owned by anyone.
    pub pool: H256,
    /// A value that can be used to guarantee order uniqueness. Typically it is
    /// set to a random number.
    #[serde_as(as = "HexOrDecimalU256")]
    pub salt: U256,
    /// It allows the maker to enforce that the order flow through some
    /// additional logic before it can be filled (e.g., a KYC whitelist).
    pub sender: H160,
    /// The signature of the signed order.
    pub signature: ZeroExSignature,
    /// The address of the party that is allowed to fill the order. If set to a
    /// specific party, the order cannot be filled by anyone else. If left
    /// unspecified, anyone can fill the order.
    pub taker: H160,
    /// The amount of `taker_token` being sold by the taker.
    #[serde_as(as = "DisplayFromStr")]
    pub taker_amount: u128,
    /// The address of the ERC20 token the taker is selling to the maker.
    pub taker_token: H160,
    /// Amount of takerToken paid by the taker to the feeRecipient.
    #[serde_as(as = "DisplayFromStr")]
    pub taker_token_fee_amount: u128,
    /// Address of the contract where the transaction should be sent, usually
    /// this is the 0x exchange proxy contract.
    pub verifying_contract: H160,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct OrderRecord {
    #[serde(rename = "metaData")]
    pub metadata: OrderMetadata,
    pub order: Order,
}

impl OrderRecord {
    /// Scales the `maker_amount` according to how much of the partially
    /// fillable amount was already used.
    pub fn remaining_maker_amount(&self) -> Result<u128> {
        if self.metadata.remaining_fillable_taker_amount > self.order.taker_amount {
            anyhow::bail!("remaining taker amount bigger than total taker amount");
        }

        // all numbers are at most u128::MAX so none of these operations can overflow
        let scaled_maker_amount = U256::from(self.order.maker_amount)
            * U256::from(self.metadata.remaining_fillable_taker_amount)
            / U256::from(self.order.taker_amount);

        // `scaled_maker_amount` is at most as big as `maker_amount` which already fits
        // in an u128
        Ok(scaled_maker_amount.as_u128())
    }
}

/// A Ox API `orders` response.
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrdersResponse {
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub records: Vec<OrderRecord>,
}

/// A Ox API `price` response.
#[serde_as]
#[derive(Clone, Default, Derivative, Deserialize, PartialEq)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct PriceResponse {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    pub allowance_target: H160,
    #[serde_as(as = "DisplayFromStr")]
    pub price: f64,
    #[serde_as(as = "DisplayFromStr")]
    pub estimated_gas: u64,
}

/// A Ox API `swap` response.
#[serde_as]
#[derive(Clone, Default, Derivative, Deserialize, PartialEq)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[serde(flatten)]
    pub price: PriceResponse,
    pub to: H160,
    #[derivative(Debug(format_with = "debug_bytes"))]
    #[serde(with = "bytes_hex")]
    pub data: Vec<u8>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
}

impl Interaction for SwapResponse {
    fn encode(&self) -> EncodedInteraction {
        (self.to, self.value, Bytes(self.data.clone()))
    }
}

/// Abstract 0x API. Provides a mockable implementation.
#[async_trait::async_trait]
#[mockall::automock]
pub trait ZeroExApi: Send + Sync {
    /// Retrieve a swap for the specified parameters from the 0x API.
    ///
    /// See [`/swap/v1/quote`](https://0x.org/docs/api#get-swapv1quote).
    async fn get_swap(
        &self,
        query: SwapQuery,
        set_block_retriever_header: bool,
    ) -> Result<SwapResponse, ZeroExResponseError>;

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
    block_stream: CurrentBlockStream,
}

impl DefaultZeroExApi {
    /// Default 0x API URL.
    pub const DEFAULT_URL: &'static str = "https://api.0x.org/";
    /// Default 0x verifying contract.
    /// The currently latest 0x v4 contract.
    pub const DEFAULT_VERIFICATION_CONTRACT: H160 =
        addr!("Def1C0ded9bec7F1a1670819833240f027b25EfF");

    /// Create a new 0x HTTP API client with the specified base URL.
    pub fn new(
        client_builder: ClientBuilder,
        base_url: impl IntoUrl,
        api_key: Option<String>,
        block_stream: CurrentBlockStream,
    ) -> Result<Self> {
        let client_builder = if let Some(api_key) = api_key {
            let mut key = HeaderValue::from_str(&api_key)?;
            key.set_sensitive(true);

            let mut headers = HeaderMap::new();
            headers.insert("0x-api-key", key);

            client_builder.default_headers(headers)
        } else {
            client_builder
        };

        Ok(Self {
            client: client_builder.build().unwrap(),
            base_url: base_url.into_url().context("zeroex api url")?,
            block_stream,
        })
    }

    /// Create a 0x HTTP API client for testing using the default HTTP client.
    ///
    /// This method will attempt to read the `ZEROEX_URL` (falling back to the
    /// default URL) and `ZEROEX_API_KEY` (falling back to no API key) from the
    /// local environment when creating the API client.
    pub fn test() -> Self {
        let (_, block_stream) = watch::channel(BlockInfo::default());
        Self::new(
            Client::builder(),
            std::env::var("ZEROEX_URL").unwrap_or_else(|_| Self::DEFAULT_URL.to_string()),
            std::env::var("ZEROEX_API_KEY").ok(),
            block_stream,
        )
        .unwrap()
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
        self.request(url, false).await
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawResponse<Ok> {
    ResponseOk(Ok),
    ResponseErr { reason: String, code: u32 },
}

#[derive(Error, Debug)]
pub enum ZeroExResponseError {
    #[error("ServerError from query {0}")]
    ServerError(String),

    #[error("uncatalogued error message: {0}")]
    UnknownZeroExError(String),

    #[error("insufficient liquidity")]
    InsufficientLiquidity,

    #[error("Error({0}) for response {1}")]
    DeserializeError(serde_json::Error, String),

    // Recovered Response but failed on async call of response.text()
    #[error(transparent)]
    TextFetch(reqwest::Error),

    // Connectivity or non-response error
    #[error("Failed on send")]
    Send(reqwest::Error),

    #[error("Rate limited")]
    RateLimited,
}

#[async_trait::async_trait]
impl ZeroExApi for DefaultZeroExApi {
    async fn get_swap(
        &self,
        query: SwapQuery,
        set_current_block_header: bool,
    ) -> Result<SwapResponse, ZeroExResponseError> {
        self.request(
            query.format_url(&self.base_url, "quote"),
            set_current_block_header,
        )
        .await
    }

    async fn get_price(&self, query: SwapQuery) -> Result<PriceResponse, ZeroExResponseError> {
        self.request(query.format_url(&self.base_url, "price"), false)
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

/// Append data of response to results and return whether another page should be
/// fetched.
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
        let expiry = Utc.timestamp_opt(order.order.expiry as i64, 0).unwrap();

        // only keep orders which are still valid and unique
        expiry > now && included_orders.insert(order.metadata.order_hash.clone())
    });
}

impl DefaultZeroExApi {
    async fn request<T: for<'a> serde::Deserialize<'a>>(
        &self,
        url: Url,
        set_current_block_header: bool,
    ) -> Result<T, ZeroExResponseError> {
        tracing::trace!("Querying 0x API: {}", url);

        let path = url.path().to_owned();
        let result = async move {
            let mut request = self.client.get(url.clone());
            if set_current_block_header {
                request = request.header(
                    "X-Current-Block-Hash",
                    self.block_stream.borrow().hash.to_string(),
                );
            };
            if let Some(id) = observe::request_id::get_task_local_storage() {
                request = request.header("X-REQUEST-ID", id);
            }

            let response = request.send().await.map_err(ZeroExResponseError::Send)?;

            let status = response.status();
            let response_text = response
                .text()
                .await
                .map_err(ZeroExResponseError::TextFetch)?;
            tracing::trace!("Response from 0x API: {}", response_text);

            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(ZeroExResponseError::RateLimited);
            }

            match serde_json::from_str::<RawResponse<T>>(&response_text) {
                Ok(RawResponse::ResponseOk(response)) => Ok(response),
                Ok(RawResponse::ResponseErr { reason, code }) => match code {
                    // Validation Error
                    100 => Err(ZeroExResponseError::InsufficientLiquidity),
                    500..=599 => Err(ZeroExResponseError::ServerError(format!("{url:?}"))),
                    _ => Err(ZeroExResponseError::UnknownZeroExError(reason)),
                },
                Err(err) => Err(ZeroExResponseError::DeserializeError(
                    err,
                    response_text.parse().unwrap(),
                )),
            }
        }
        .await;

        let metrics = Metrics::get();
        let status = if result.is_ok() { "success" } else { "failure" };
        metrics
            .zeroex_api_requests
            .with_label_values(&[path.as_str(), status])
            .inc();

        result
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Counter for 0x API requests by URI path and result.
    #[metric(labels("path", "result"))]
    zeroex_api_requests: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::addr,
        chrono::{DateTime, NaiveDate},
    };

    #[tokio::test]
    #[ignore]
    async fn zeroex_swap() {
        let zeroex_client = DefaultZeroExApi::test();
        let swap_query = SwapQuery {
            sell_token: testlib::tokens::WETH,
            buy_token: testlib::tokens::USDC,
            sell_amount: Some(U256::from_f64_lossy(1e18)),
            slippage_percentage: Some(Slippage::new(0.012345678)),
            ..Default::default()
        };

        let price_response = zeroex_client.get_swap(swap_query, false).await;
        dbg!(&price_response);
        assert!(price_response.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_api_e2e_private() {
        let zeroex_client = DefaultZeroExApi::test();
        let swap_query = SwapQuery {
            sell_token: testlib::tokens::WETH,
            buy_token: testlib::tokens::USDC,
            sell_amount: Some(U256::from_f64_lossy(1e18)),
            slippage_percentage: Some(Slippage::ONE_PERCENT),
            ..Default::default()
        };

        let price_response = zeroex_client.get_price(swap_query.clone()).await;
        dbg!(&price_response);
        assert!(price_response.is_ok());
        let swap_response = zeroex_client.get_swap(swap_query, false).await;
        dbg!(&swap_response);
        assert!(swap_response.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn excluded_sources() {
        let zeroex = DefaultZeroExApi::test();
        let query = SwapQuery {
            sell_token: testlib::tokens::WETH,
            buy_token: addr!("c011a73ee8576fb46f5e1c5751ca3b9fe0af2a6f"), // SNX
            sell_amount: Some(U256::from_f64_lossy(1000e18)),
            slippage_percentage: Some(Slippage::ONE_PERCENT),
            ..Default::default()
        };

        let swap = zeroex.get_swap(query.clone(), false).await;
        dbg!(&swap);
        assert!(swap.is_ok());

        let swap = zeroex
            .get_swap(
                SwapQuery {
                    excluded_sources: vec!["Balancer_V2".to_string()],
                    ..query
                },
                false,
            )
            .await;
        dbg!(&swap);
        assert!(swap.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_orders() {
        let api = DefaultZeroExApi::test();
        let result = api.get_orders(&OrdersQuery::default()).await;
        dbg!(&result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_orders_paginated_with_empty_result() {
        let api = DefaultZeroExApi::test();
        // `get_orders()` relies on `get_orders_with_pagination()` not producing and
        // error instead of an response with 0 records. To test that we request
        // a page which should never have a any records and check that it
        // doesn't throw an error.
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
                        created_at: DateTime::from_utc(NaiveDate::from_ymd_opt(2022, 2, 26).unwrap().and_hms_nano_opt(6, 59, 0, 440_000_000).unwrap(), Utc),
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
