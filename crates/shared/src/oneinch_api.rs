//! 1Inch HTTP API client implementation.
//!
//! For more information on the HTTP API, consult:
//! <https://docs.1inch.io/docs/aggregation-protocol/api/swagger>
//! Although there is no documentation about API v4.1, it exists and is identical to v4.0 except it
//! uses EIP 1559 gas prices.
use anyhow::{ensure, Result};
use cached::{Cached, TimedCache};
use ethcontract::{Bytes, H160, U256};
use model::{
    interaction::{EncodedInteraction, Interaction},
    u256_decimal,
};
use reqwest::{Client, IntoUrl, Url};
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    fmt::{self, Display, Formatter},
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;

/// Parts to split a swap.
///
/// This type is generic on the maximum number of splits allowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
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

#[derive(Debug, Clone)]
pub struct SellOrderQuoteQuery {
    /// Contract address of a token to sell.
    pub from_token_address: H160,
    /// Contract address of a token to buy.
    pub to_token_address: H160,
    /// Amount of a token to sell, set in atoms.
    pub amount: U256,
    /// Maximum number of token-connectors to be used in a transaction.
    pub complexity_level: Option<Amount<0, 3>>,
    /// List of protocols to use for the swap.
    pub protocols: Option<Vec<String>>,
    /// Maximum amount of gas for a swap. Default: 11500000
    pub gas_limit: Option<Amount<0, 11500000>>,
    /// Limit maximum number of main route parts.
    pub main_route_parts: Option<Amount<1, 50>>,
    /// Limit maximum number of parts each main route part can be split into.
    pub parts: Option<Amount<1, 100>>,
    /// This percentage of from_token_address token amount will be sent
    /// to referrer_address. The rest will be used as input for the swap.
    /// min: 0, max: 3, default: 0
    pub fee: Option<f64>,
    /// Gas price in smallest divisible unit. default: "fast" from network
    pub gas_price: Option<U256>,
    /// Limit the number of virtual split parts. default: 50
    pub virtual_parts: Option<Amount<1, 500>>,
    /// Which tokens should be used for intermediate trading hops.
    pub connector_tokens: Option<Vec<H160>>,
    /// Adress referring this trade which will receive a portion of the swap
    /// fees as a reward.
    pub referrer_address: Option<H160>,
}

// The `Display` implementation for `H160` unfortunately does not print
// the full address and instead uses ellipsis (e.g. "0xeeee…eeee"). This
// helper just works around that.
fn addr2str(addr: H160) -> String {
    format!("{:?}", addr)
}

impl SellOrderQuoteQuery {
    fn into_url(self, base_url: &Url, chain_id: u64) -> Url {
        let endpoint = format!("v4.1/{}/quote", chain_id);
        let mut url = base_url
            .join(&endpoint)
            .expect("unexpectedly invalid URL segment");

        url.query_pairs_mut()
            .append_pair("fromTokenAddress", &addr2str(self.from_token_address))
            .append_pair("toTokenAddress", &addr2str(self.to_token_address))
            .append_pair("amount", &self.amount.to_string());

        if let Some(protocols) = self.protocols {
            url.query_pairs_mut()
                .append_pair("protocols", &protocols.join(","));
        }
        if let Some(fee) = self.fee {
            url.query_pairs_mut().append_pair("fee", &fee.to_string());
        }
        if let Some(gas_limit) = self.gas_limit {
            url.query_pairs_mut()
                .append_pair("gasLimit", &gas_limit.to_string());
        }
        if let Some(connector_tokens) = self.connector_tokens {
            url.query_pairs_mut().append_pair(
                "connectorTokens",
                &connector_tokens
                    .into_iter()
                    .map(addr2str)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        if let Some(complexity_level) = self.complexity_level {
            url.query_pairs_mut()
                .append_pair("complexityLevel", &complexity_level.to_string());
        }
        if let Some(main_route_parts) = self.main_route_parts {
            url.query_pairs_mut()
                .append_pair("mainRouteParts", &main_route_parts.to_string());
        }
        if let Some(virtual_parts) = self.virtual_parts {
            url.query_pairs_mut()
                .append_pair("virtualParts", &virtual_parts.to_string());
        }
        if let Some(parts) = self.parts {
            url.query_pairs_mut()
                .append_pair("parts", &parts.to_string());
        }
        if let Some(gas_price) = self.gas_price {
            url.query_pairs_mut()
                .append_pair("gasPrice", &gas_price.to_string());
        }
        if let Some(referrer_address) = self.referrer_address {
            url.query_pairs_mut()
                .append_pair("referrerAddress", &addr2str(referrer_address));
        }

        url
    }

    pub fn with_default_options(
        sell_token: H160,
        buy_token: H160,
        protocols: Option<Vec<String>>,
        amount: U256,
        referrer_address: Option<H160>,
    ) -> Self {
        Self {
            from_token_address: sell_token,
            to_token_address: buy_token,
            amount,
            protocols,
            complexity_level: None,
            gas_limit: None,
            main_route_parts: None,
            parts: None,
            fee: None,
            gas_price: None,
            virtual_parts: None,
            connector_tokens: None,
            referrer_address,
        }
    }
}

/// A sell order quote from 1Inch.
#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SellOrderQuote {
    pub from_token: Token,
    pub to_token: Token,
    #[serde(with = "u256_decimal")]
    pub from_token_amount: U256,
    #[serde(with = "u256_decimal")]
    pub to_token_amount: U256,
    pub protocols: Vec<Vec<Vec<ProtocolRouteSegment>>>,
    pub estimated_gas: u64,
}

/// A 1Inch API quote query parameters.
///
/// These parameters are currently incomplete, and missing parameters can be
/// added incrementally as needed.
#[derive(Clone, Debug)]
pub struct SwapQuery {
    /// Address of a seller.
    ///
    /// Make sure that this address has approved to spend `from_token_address`
    /// in needed amount.
    pub from_address: H160,
    /// Limit of price slippage you are willing to accept.
    pub slippage: Slippage,
    /// Flag to disable checks of the required quantities.
    pub disable_estimate: Option<bool>,
    /// Receiver of destination currency. default: from_address
    pub dest_receiver: Option<H160>,
    /// Should Chi of from_token_address be burnt to compensate for gas.
    /// default: false
    pub burn_chi: Option<bool>,
    /// If true, the algorithm can cancel part of the route, if the rate has become
    /// less attractive. Unswapped tokens will return to the from_address
    /// default: true
    pub allow_partial_fill: Option<bool>,
    pub quote: SellOrderQuoteQuery,
}

/// A slippage amount.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default)]
pub struct Slippage(f64);

impl Slippage {
    pub const ONE_PERCENT: Self = Self(1.);

    /// Creates a slippage amount from the specified percentage.
    pub fn percentage(amount: f64) -> Result<Self> {
        // 1Inch API only accepts a slippage from 0 to 50.
        ensure!(
            (0. ..=50.).contains(&amount),
            "slippage outside of [0%, 50%] range"
        );

        Ok(Slippage(amount))
    }
}

impl Display for Slippage {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Note that we use a rounded slippage percentage. This is because the
        // 1Inch API will repsond with server errors if the slippage paramter
        // has too much precision.
        write!(f, "{:.4}", self.0)
    }
}

impl SwapQuery {
    /// Encodes the swap query as
    fn into_url(self, base_url: &Url, chain_id: u64) -> Url {
        let endpoint = format!("v4.1/{}/swap", chain_id);
        let mut url = base_url
            .join(&endpoint)
            .expect("unexpectedly invalid URL segment");
        url.query_pairs_mut()
            .append_pair("fromTokenAddress", &addr2str(self.quote.from_token_address))
            .append_pair("toTokenAddress", &addr2str(self.quote.to_token_address))
            .append_pair("amount", &self.quote.amount.to_string())
            .append_pair("fromAddress", &addr2str(self.from_address))
            .append_pair("slippage", &self.slippage.to_string());

        if let Some(protocols) = self.quote.protocols {
            url.query_pairs_mut()
                .append_pair("protocols", &protocols.join(","));
        }
        if let Some(disable_estimate) = self.disable_estimate {
            url.query_pairs_mut()
                .append_pair("disableEstimate", &disable_estimate.to_string());
        }
        if let Some(complexity_level) = self.quote.complexity_level {
            url.query_pairs_mut()
                .append_pair("complexityLevel", &complexity_level.to_string());
        }
        if let Some(gas_limit) = self.quote.gas_limit {
            url.query_pairs_mut()
                .append_pair("gasLimit", &gas_limit.to_string());
        }
        if let Some(main_route_parts) = self.quote.main_route_parts {
            url.query_pairs_mut()
                .append_pair("mainRouteParts", &main_route_parts.to_string());
        }
        if let Some(parts) = self.quote.parts {
            url.query_pairs_mut()
                .append_pair("parts", &parts.to_string());
        }
        if let Some(dest_receiver) = self.dest_receiver {
            url.query_pairs_mut()
                .append_pair("destReceiver", &addr2str(dest_receiver));
        }
        if let Some(referrer_address) = self.quote.referrer_address {
            url.query_pairs_mut()
                .append_pair("referrerAddress", &addr2str(referrer_address));
        }
        if let Some(fee) = self.quote.fee {
            url.query_pairs_mut().append_pair("fee", &fee.to_string());
        }
        if let Some(gas_price) = self.quote.gas_price {
            url.query_pairs_mut()
                .append_pair("gasPrice", &gas_price.to_string());
        }
        if let Some(burn_chi) = self.burn_chi {
            url.query_pairs_mut()
                .append_pair("burnChi", &burn_chi.to_string());
        }
        if let Some(allow_partial_fill) = self.allow_partial_fill {
            url.query_pairs_mut()
                .append_pair("allowPartialFill", &allow_partial_fill.to_string());
        }
        if let Some(virtual_parts) = self.quote.virtual_parts {
            url.query_pairs_mut()
                .append_pair("virtualParts", &virtual_parts.to_string());
        }
        if let Some(connector_tokens) = self.quote.connector_tokens {
            url.query_pairs_mut().append_pair(
                "connectorTokens",
                &connector_tokens
                    .into_iter()
                    .map(addr2str)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }

        url
    }

    pub fn with_default_options(
        sell_token: H160,
        buy_token: H160,
        in_amount: U256,
        from_address: H160,
        protocols: Option<Vec<String>>,
        slippage: Slippage,
        referrer_address: Option<H160>,
    ) -> Self {
        Self {
            from_address,
            slippage,
            // Disable balance/allowance checks, as the settlement contract
            // does not hold balances to traded tokens.
            disable_estimate: Some(true),
            quote: SellOrderQuoteQuery::with_default_options(
                sell_token,
                buy_token,
                protocols,
                in_amount,
                referrer_address,
            ),
            dest_receiver: None,
            burn_chi: None,
            allow_partial_fill: Some(false),
        }
    }
}

/// A 1Inch API response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum RestResponse<T> {
    Ok(T),
    Err(RestError),
}

#[derive(Debug, Error)]
pub enum OneInchError {
    #[error("1Inch API error: {0}")]
    Api(#[from] RestError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl OneInchError {
    pub fn is_insuffucient_liquidity(&self) -> bool {
        matches!(self, Self::Api(err) if err.description == "insufficient liquidity")
    }
}

impl From<reqwest::Error> for OneInchError {
    fn from(err: reqwest::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<serde_json::Error> for OneInchError {
    fn from(err: serde_json::Error) -> Self {
        Self::Other(err.into())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Default, Error)]
#[error("1Inch API error ({status_code}): {description}")]
#[serde(rename_all = "camelCase")]
pub struct RestError {
    pub status_code: u32,
    pub description: String,
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
    pub protocols: Vec<Vec<Vec<ProtocolRouteSegment>>>,
    pub tx: Transaction,
}

impl Interaction for Swap {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.tx.to, self.tx.value, Bytes(self.tx.data.clone()))]
    }
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
pub struct ProtocolRouteSegment {
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
    #[serde(with = "model::bytes_hex")]
    pub data: Vec<u8>,
    #[serde(with = "u256_decimal")]
    pub value: U256,
    #[serde(with = "u256_decimal")]
    pub max_fee_per_gas: U256,
    #[serde(with = "u256_decimal")]
    pub max_priority_fee_per_gas: U256,
    pub gas: u64,
}

impl std::fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transaction")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("data", &format_args!("0x{}", hex::encode(&self.data)))
            .field("value", &self.value)
            .field("max_fee_per_gas", &self.max_fee_per_gas)
            .field("max_priority_fee_per_gas", &self.max_priority_fee_per_gas)
            .field("gas", &self.gas)
            .finish()
    }
}
/// Approve spender response.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Spender {
    pub address: H160,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ProtocolInfo {
    pub id: String,
}

impl From<&str> for ProtocolInfo {
    fn from(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

/// Protocols query response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Protocols {
    pub protocols: Vec<ProtocolInfo>,
}

// Mockable version of API Client
#[async_trait::async_trait]
#[mockall::automock]
pub trait OneInchClient: Send + Sync + 'static {
    /// Retrieves a swap for the specified parameters from the 1Inch API.
    async fn get_swap(&self, query: SwapQuery) -> Result<Swap, OneInchError>;

    /// Quotes a sell order with the 1Inch API.
    async fn get_sell_order_quote(
        &self,
        query: SellOrderQuoteQuery,
    ) -> Result<SellOrderQuote, OneInchError>;

    /// Retrieves the address of the spender to use for token approvals.
    async fn get_spender(&self) -> Result<Spender, OneInchError>;

    /// Retrieves a list of the on-chain protocols supported by 1Inch.
    async fn get_liquidity_sources(&self) -> Result<Protocols, OneInchError>;
}

/// 1Inch API Client implementation.
#[derive(Debug)]
pub struct OneInchClientImpl {
    client: Client,
    base_url: Url,
    chain_id: u64,
}

impl OneInchClientImpl {
    pub const DEFAULT_URL: &'static str = "https://api.1inch.exchange/";

    // 1: mainnet, 100: gnosis chain
    pub const SUPPORTED_CHAINS: &'static [u64] = &[1, 100];

    /// Create a new 1Inch HTTP API client with the specified base URL.
    pub fn new(base_url: impl IntoUrl, client: Client, chain_id: u64) -> Result<Self> {
        ensure!(
            Self::SUPPORTED_CHAINS.contains(&chain_id),
            "1Inch is not supported on this chain"
        );

        Ok(Self {
            client,
            base_url: base_url.into_url()?,
            chain_id,
        })
    }
}

#[async_trait::async_trait]
impl OneInchClient for OneInchClientImpl {
    async fn get_swap(&self, query: SwapQuery) -> Result<Swap, OneInchError> {
        logged_query(&self.client, query.into_url(&self.base_url, self.chain_id)).await
    }

    async fn get_sell_order_quote(
        &self,
        query: SellOrderQuoteQuery,
    ) -> Result<SellOrderQuote, OneInchError> {
        logged_query(&self.client, query.into_url(&self.base_url, self.chain_id)).await
    }

    async fn get_spender(&self) -> Result<Spender, OneInchError> {
        let endpoint = format!("v4.1/{}/approve/spender", self.chain_id);
        let url = self
            .base_url
            .join(&endpoint)
            .expect("unexpectedly invalid URL");
        logged_query(&self.client, url).await
    }

    async fn get_liquidity_sources(&self) -> Result<Protocols, OneInchError> {
        let endpoint = format!("v4.1/{}/liquidity-sources", self.chain_id);
        let url = self
            .base_url
            .join(&endpoint)
            .expect("unexpectedly invalid URL");
        logged_query(&self.client, url).await
    }
}

async fn logged_query<D>(client: &Client, url: Url) -> Result<D, OneInchError>
where
    D: DeserializeOwned,
{
    tracing::debug!("Query 1inch API for url {}", url);
    let response = client.get(url).send().await?.text().await;
    tracing::debug!("Response from 1inch API: {:?}", response);
    match serde_json::from_str::<RestResponse<D>>(&response?)? {
        RestResponse::Ok(result) => Ok(result),
        RestResponse::Err(err) => Err(err.into()),
    }
}

#[derive(Debug, Clone)]
pub struct ProtocolCache(Arc<Mutex<TimedCache<(), Vec<ProtocolInfo>>>>);

impl ProtocolCache {
    pub fn new(cache_validity_in_seconds: Duration) -> Self {
        Self(Arc::new(Mutex::new(TimedCache::with_lifespan_and_refresh(
            cache_validity_in_seconds.as_secs(),
            false,
        ))))
    }

    pub async fn get_all_protocols(&self, api: &dyn OneInchClient) -> Result<Vec<ProtocolInfo>> {
        if let Some(cached) = self.0.lock().unwrap().cache_get(&()) {
            return Ok(cached.clone());
        }

        let all_protocols = api.get_liquidity_sources().await?.protocols;
        // In the mean time the cache could have already been populated with new protocols,
        // which we would now overwrite. This is fine.
        self.0.lock().unwrap().cache_set((), all_protocols.clone());

        Ok(all_protocols)
    }

    pub async fn get_allowed_protocols(
        &self,
        disabled_protocols: &[String],
        api: &dyn OneInchClient,
    ) -> Result<Option<Vec<String>>> {
        if disabled_protocols.is_empty() {
            return Ok(None);
        }

        let allowed_protocols = self
            .get_all_protocols(api)
            .await?
            .into_iter()
            // linear search through the slice is okay because it's very small
            .filter(|protocol| !disabled_protocols.contains(&protocol.id))
            .map(|protocol| protocol.id)
            .collect();

        Ok(Some(allowed_protocols))
    }
}

impl Default for ProtocolCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr;
    use futures::FutureExt as _;

    #[test]
    fn slippage_rounds_percentage() {
        assert_eq!(Slippage(1.2345678).to_string(), "1.2346");
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
            from_address: addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
            slippage: Slippage::percentage(0.5).unwrap(),
            disable_estimate: None,
            quote: SellOrderQuoteQuery {
                from_token_address: addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                to_token_address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
                amount: 1_000_000_000_000_000_000u128.into(),
                protocols: None,
                complexity_level: None,
                gas_limit: None,
                main_route_parts: None,
                parts: None,
                fee: None,
                gas_price: None,
                virtual_parts: None,
                connector_tokens: None,
                referrer_address: None,
            },
            dest_receiver: None,
            burn_chi: None,
            allow_partial_fill: None,
        }
        .into_url(&base_url, 1);

        assert_eq!(
            url.as_str(),
            "https://api.1inch.exchange/v4.1/1/swap\
                ?fromTokenAddress=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\
                &toTokenAddress=0x111111111117dc0aa78b770fa6a738034120c302\
                &amount=1000000000000000000\
                &fromAddress=0x00000000219ab540356cbb839cbe05303d7705fa\
                &slippage=0.5000",
        );
    }

    #[test]
    fn swap_query_serialization_options_parameters() {
        let base_url = Url::parse("https://api.1inch.exchange/").unwrap();
        let url = SwapQuery {
            from_address: addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
            slippage: Slippage::percentage(0.5).unwrap(),
            disable_estimate: Some(true),
            quote: SellOrderQuoteQuery {
                from_token_address: addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                to_token_address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
                protocols: Some(vec!["WETH".to_string(), "UNISWAP_V3".to_string()]),
                amount: 1_000_000_000_000_000_000u128.into(),
                complexity_level: Some(Amount::new(2).unwrap()),
                gas_limit: Some(Amount::new(133700).unwrap()),
                main_route_parts: Some(Amount::new(28).unwrap()),
                parts: Some(Amount::new(42).unwrap()),
                fee: Some(1.5),
                gas_price: Some(100_000.into()),
                virtual_parts: Some(Amount::new(10).unwrap()),
                connector_tokens: Some(vec![
                    addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                    addr!("6810e776880c02933d47db1b9fc05908e5386b96"),
                ]),
                referrer_address: Some(addr!("41111a111217dc0aa78b774fa6a738024120c302")),
            },
            burn_chi: Some(false),
            allow_partial_fill: Some(false),
            dest_receiver: Some(addr!("41111a111217dc0aa78b774fa6a738024120c302")),
        }
        .into_url(&base_url, 1);

        assert_eq!(
            url.as_str(),
            "https://api.1inch.exchange/v4.1/1/swap\
                ?fromTokenAddress=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\
                &toTokenAddress=0x111111111117dc0aa78b770fa6a738034120c302\
                &amount=1000000000000000000\
                &fromAddress=0x00000000219ab540356cbb839cbe05303d7705fa\
                &slippage=0.5000\
                &protocols=WETH%2CUNISWAP_V3\
                &disableEstimate=true\
                &complexityLevel=2\
                &gasLimit=133700\
                &mainRouteParts=28\
                &parts=42\
                &destReceiver=0x41111a111217dc0aa78b774fa6a738024120c302\
                &referrerAddress=0x41111a111217dc0aa78b774fa6a738024120c302\
                &fee=1.5\
                &gasPrice=100000\
                &burnChi=false\
                &allowPartialFill=false\
                &virtualParts=10\
                &connectorTokens=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2%2C\
                    0x6810e776880c02933d47db1b9fc05908e5386b96"
        );
    }

    #[test]
    fn deserialize_swap_response() {
        let swap = serde_json::from_str::<RestResponse<Swap>>(
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
                "maxFeePerGas": "123865303708",
                "maxPriorityFeePerGas": "2000000000",
                "gas": 143297
              }
            }"#,
        )
        .unwrap();

        assert_eq!(
            swap,
            RestResponse::Ok(Swap {
                from_token: Token {
                    address: addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
                },
                to_token: Token {
                    address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
                },
                from_token_amount: 1_000_000_000_000_000_000u128.into(),
                to_token_amount: 501_739_725_821_378_713_485u128.into(),
                protocols: vec![vec![
                    vec![ProtocolRouteSegment {
                        name: "WETH".to_owned(),
                        part: 100.,
                        from_token_address: addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
                        to_token_address: testlib::tokens::WETH,
                    }],
                    vec![ProtocolRouteSegment {
                        name: "UNISWAP_V2".to_owned(),
                        part: 100.,
                        from_token_address: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                        to_token_address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
                    }],
                ]],
                tx: Transaction {
                    from: addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
                    to: addr!("11111112542d85b3ef69ae05771c2dccff4faa26"),
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
                    max_fee_per_gas: 123_865_303_708u128.into(),
                    max_priority_fee_per_gas: 2_000_000_000u128.into(),
                    gas: 143297,
                },
            })
        );

        let swap_error = serde_json::from_str::<RestResponse<Swap>>(
            r#"{
            "statusCode":500,
            "description":"Internal server error"
        }"#,
        )
        .unwrap();

        assert_eq!(
            swap_error,
            RestResponse::Err(RestError {
                status_code: 500,
                description: "Internal server error".into()
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
                address: addr!("11111112542d85b3ef69ae05771c2dccff4faa26"),
            }
        )
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_swap() {
        let swap = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1)
            .unwrap()
            .get_swap(SwapQuery {
                from_address: addr!("00000000219ab540356cBB839Cbe05303d7705Fa"),
                slippage: Slippage::ONE_PERCENT,
                disable_estimate: None,
                quote: SellOrderQuoteQuery::with_default_options(
                    addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                    addr!("111111111117dc0aa78b770fa6a738034120c302"),
                    None,
                    1_000_000_000_000_000_000u128.into(),
                    None,
                ),
                burn_chi: None,
                allow_partial_fill: None,
                dest_receiver: None,
            })
            .await
            .unwrap();
        println!("{:#?}", swap);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_swap_fully_parameterized() {
        let swap = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1)
            .unwrap()
            .get_swap(SwapQuery {
                from_address: addr!("4e608b7da83f8e9213f554bdaa77c72e125529d0"),
                slippage: Slippage::percentage(1.2345678).unwrap(),
                disable_estimate: Some(true),
                quote: SellOrderQuoteQuery {
                    from_token_address: addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                    to_token_address: addr!("a3BeD4E1c75D00fa6f4E5E6922DB7261B5E9AcD2"),
                    protocols: Some(vec!["WETH".to_string(), "UNISWAP_V2".to_string()]),
                    amount: 100_000_000_000_000_000_000u128.into(),
                    complexity_level: Some(Amount::new(2).unwrap()),
                    gas_limit: Some(Amount::new(750_000).unwrap()),
                    main_route_parts: Some(Amount::new(3).unwrap()),
                    parts: Some(Amount::new(3).unwrap()),
                    fee: Some(1.5),
                    // setting `gas_price` will produce a response with different fields
                    gas_price: None,
                    connector_tokens: Some(vec![
                        addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                        addr!("6810e776880c02933d47db1b9fc05908e5386b96"),
                    ]),
                    virtual_parts: Some(Amount::new(10).unwrap()),
                    referrer_address: Some(addr!("9008D19f58AAbD9eD0D60971565AA8510560ab41")),
                },
                dest_receiver: Some(addr!("9008D19f58AAbD9eD0D60971565AA8510560ab41")),
                burn_chi: Some(false),
                allow_partial_fill: Some(false),
            })
            .await
            .unwrap();
        println!("{:#?}", swap);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_liquidity_sources() {
        let protocols = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1)
            .unwrap()
            .get_liquidity_sources()
            .await
            .unwrap();
        println!("{:#?}", protocols);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_spender_address() {
        let spender = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1)
            .unwrap()
            .get_spender()
            .await
            .unwrap();
        println!("{:#?}", spender);
    }

    #[test]
    fn sell_order_quote_query_serialization() {
        let base_url = Url::parse("https://api.1inch.exchange/").unwrap();
        let url = SellOrderQuoteQuery {
            from_token_address: addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
            to_token_address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
            amount: 1_000_000_000_000_000_000u128.into(),
            protocols: None,
            complexity_level: None,
            gas_limit: None,
            main_route_parts: None,
            parts: None,
            fee: None,
            gas_price: None,
            virtual_parts: None,
            connector_tokens: None,
            referrer_address: None,
        }
        .into_url(&base_url, 1);

        assert_eq!(
            url.as_str(),
            "https://api.1inch.exchange/v4.1/1/quote\
                ?fromTokenAddress=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\
                &toTokenAddress=0x111111111117dc0aa78b770fa6a738034120c302\
                &amount=1000000000000000000"
        );
    }

    #[test]
    fn sell_order_quote_query_serialization_optional_parameters() {
        let base_url = Url::parse("https://api.1inch.exchange/").unwrap();
        let url = SellOrderQuoteQuery {
            from_token_address: addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
            to_token_address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
            protocols: Some(vec!["WETH".to_string(), "UNISWAP_V3".to_string()]),
            amount: 1_000_000_000_000_000_000u128.into(),
            fee: Some(0.5),
            connector_tokens: Some(vec![
                addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                addr!("6810e776880c02933d47db1b9fc05908e5386b96"),
            ]),
            virtual_parts: Some(Amount::new(42).unwrap()),
            gas_price: Some(200_000.into()),
            complexity_level: Some(Amount::new(2).unwrap()),
            gas_limit: Some(Amount::new(750_000).unwrap()),
            main_route_parts: Some(Amount::new(3).unwrap()),
            parts: Some(Amount::new(3).unwrap()),
            referrer_address: Some(addr!("9008D19f58AAbD9eD0D60971565AA8510560ab41")),
        }
        .into_url(&base_url, 1);

        assert_eq!(
            url.as_str(),
            "https://api.1inch.exchange/v4.1/1/quote\
                ?fromTokenAddress=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\
                &toTokenAddress=0x111111111117dc0aa78b770fa6a738034120c302\
                &amount=1000000000000000000\
                &protocols=WETH%2CUNISWAP_V3\
                &fee=0.5\
                &gasLimit=750000\
                &connectorTokens=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2%2C0x6810e776880c02933d47db1b9fc05908e5386b96\
                &complexityLevel=2\
                &mainRouteParts=3\
                &virtualParts=42\
                &parts=3\
                &gasPrice=200000\
                &referrerAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41"
        );
    }

    #[test]
    fn deserialize_sell_order_quote_response() {
        let swap = serde_json::from_str::<RestResponse<SellOrderQuote>>(
            r#"{
                "fromToken": {
                    "symbol": "USDC",
                    "name": "USD Coin",
                    "address": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "decimals": 6,
                    "logoURI": "https://tokens.1inch.io/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.png"
                },
                "toToken": {
                    "symbol": "USDT",
                    "name": "Tether USD",
                    "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
                    "decimals": 6,
                    "logoURI": "https://tokens.1inch.io/0xdac17f958d2ee523a2206206994597c13d831ec7.png"
                },
                "toTokenAmount": "8387323826205172",
                "fromTokenAmount": "10000000000000000",
                "protocols": [
                    [
                        [
                            {
                                "name": "CURVE_V2_EURT_2_ASSET",
                                "part": 20,
                                "fromTokenAddress": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                                "toTokenAddress": "0xdac17f958d2ee523a2206206994597c13d831ec7"
                            },
                            {
                                "name": "CURVE_V2_XAUT_2_ASSET",
                                "part": 20,
                                "fromTokenAddress": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                                "toTokenAddress": "0xdac17f958d2ee523a2206206994597c13d831ec7"
                            },
                            {
                                "name": "CURVE",
                                "part": 20,
                                "fromTokenAddress": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                                "toTokenAddress": "0xdac17f958d2ee523a2206206994597c13d831ec7"
                            },
                            {
                                "name": "SHELL",
                                "part": 40,
                                "fromTokenAddress": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                                "toTokenAddress": "0xdac17f958d2ee523a2206206994597c13d831ec7"
                            }
                        ]
                    ]
                ],
                "estimatedGas": 1456155
            }"#,
        )
        .unwrap();

        assert_eq!(
            swap,
            RestResponse::Ok(SellOrderQuote {
                from_token: Token {
                    address: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                },
                to_token: Token {
                    address: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                },
                from_token_amount: 10_000_000_000_000_000u128.into(),
                to_token_amount: 8_387_323_826_205_172u128.into(),
                protocols: vec![vec![vec![
                    ProtocolRouteSegment {
                        name: "CURVE_V2_EURT_2_ASSET".to_owned(),
                        part: 20.,
                        from_token_address: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                        to_token_address: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                    },
                    ProtocolRouteSegment {
                        name: "CURVE_V2_XAUT_2_ASSET".to_owned(),
                        part: 20.,
                        from_token_address: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                        to_token_address: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                    },
                    ProtocolRouteSegment {
                        name: "CURVE".to_owned(),
                        part: 20.,
                        from_token_address: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                        to_token_address: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                    },
                    ProtocolRouteSegment {
                        name: "SHELL".to_owned(),
                        part: 40.,
                        from_token_address: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                        to_token_address: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                    }
                ]]],
                estimated_gas: 1_456_155
            })
        );

        let swap_error = serde_json::from_str::<RestResponse<SellOrderQuote>>(
            r#"{
                "statusCode":500,
                "description":"Internal server error"
            }"#,
        )
        .unwrap();

        assert_eq!(
            swap_error,
            RestResponse::Err(RestError {
                status_code: 500,
                description: "Internal server error".into()
            })
        );
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_sell_order_quote() {
        let swap = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1)
            .unwrap()
            .get_sell_order_quote(SellOrderQuoteQuery::with_default_options(
                addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                addr!("111111111117dc0aa78b770fa6a738034120c302"),
                None,
                1_000_000_000_000_000_000u128.into(),
                None,
            ))
            .await
            .unwrap();
        println!("{:#?}", swap);
    }

    #[tokio::test]
    #[ignore]
    async fn oneinch_sell_order_quote_fully_parameterized() {
        let swap = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 1)
            .unwrap()
            .get_sell_order_quote(SellOrderQuoteQuery {
                from_token_address: addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
                to_token_address: addr!("111111111117dc0aa78b770fa6a738034120c302"),
                protocols: Some(vec!["WETH".to_string(), "UNISWAP_V3".to_string()]),
                amount: 1_000_000_000_000_000_000u128.into(),
                fee: Some(0.5),
                connector_tokens: Some(vec![
                    addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                    addr!("6810e776880c02933d47db1b9fc05908e5386b96"),
                ]),
                virtual_parts: Some(Amount::new(42).unwrap()),
                // setting `gas_price` will produce a response with different fields
                gas_price: None,
                complexity_level: Some(Amount::new(3).unwrap()),
                gas_limit: Some(Amount::new(750_000).unwrap()),
                main_route_parts: Some(Amount::new(2).unwrap()),
                parts: Some(Amount::new(2).unwrap()),
                referrer_address: Some(addr!("6C642caFCbd9d8383250bb25F67aE409147f78b2")),
            })
            .await
            .unwrap();
        println!("{:#?}", swap);
    }

    #[tokio::test]
    async fn allowing_all_protocols_will_not_use_api() {
        let mut api = MockOneInchClient::new();
        api.expect_get_liquidity_sources().times(0);
        let allowed_protocols = ProtocolCache::default()
            .get_allowed_protocols(&Vec::default(), &api)
            .await;
        matches!(allowed_protocols, Ok(None));
    }

    #[tokio::test]
    async fn allowed_protocols_get_cached() {
        let mut api = MockOneInchClient::new();
        // only 1 API call when calling get_allowed_protocols 2 times
        api.expect_get_liquidity_sources().times(1).returning(|| {
            async {
                Ok(Protocols {
                    protocols: vec!["PMM1".into(), "UNISWAP_V3".into()],
                })
            }
            .boxed()
        });

        let cache = ProtocolCache::default();
        let disabled_protocols = vec!["PMM1".to_string()];

        for _ in 0..2 {
            let allowed_protocols = cache
                .get_allowed_protocols(&disabled_protocols, &api)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(1, allowed_protocols.len());
            assert_eq!("UNISWAP_V3", allowed_protocols[0]);
        }
    }

    #[test]
    fn creation_fails_on_unsupported_chain() {
        let api = OneInchClientImpl::new(OneInchClientImpl::DEFAULT_URL, Client::new(), 2);
        assert!(api.is_err());
    }

    #[test]
    fn deserialize_liquidity_sources_response() {
        let swap = serde_json::from_str::<Protocols>(
            r#"{
                "protocols": [
                    {
                      "id": "PMMX",
                      "title": "LiqPool X",
                      "img": "https://api.1inch.io/pmm.png"
                    },
                    {
                      "id": "UNIFI",
                      "title": "Unifi",
                      "img": "https://api.1inch.io/unifi.png"
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(
            swap,
            Protocols {
                protocols: vec!["PMMX".into(), "UNIFI".into()]
            }
        );

        let swap_error = serde_json::from_str::<Protocols>(
            r#"{
                "statusCode":500,
                "description":"Internal server error"
            }"#,
        );

        assert!(swap_error.is_err());
    }
}
