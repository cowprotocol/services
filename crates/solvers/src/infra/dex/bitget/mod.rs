use {
    crate::{
        domain::{auction, dex, eth, order},
        util,
    },
    alloy::primitives::{Address, U256},
    base64::prelude::*,
    bigdecimal::BigDecimal,
    ethrpc::block_stream::CurrentBlockWatcher,
    hmac::{Hmac, Mac},
    sha2::Sha256,
    std::{
        collections::BTreeMap,
        sync::atomic::{self, AtomicU64},
    },
    tracing::Instrument,
};

/// Convert a U256 wei amount to a decimal string using the token's decimals.
/// e.g., 1000000000000000000 with 18 decimals → "1"
fn wei_to_decimal(amount: U256, decimals: u8) -> BigDecimal {
    BigDecimal::new(
        util::conv::u256_to_biguint(&amount).into(),
        i64::from(decimals),
    )
    .normalized()
}

/// Convert a decimal amount (from API response) to U256 wei.
/// e.g., "1964.365496" with 6 decimals → 1964365496
fn decimal_to_wei(amount: &BigDecimal, decimals: u8) -> Result<U256, Error> {
    let scaled = amount * BigDecimal::new(1.into(), -i64::from(decimals));
    util::conv::bigdecimal_to_u256(&scaled).ok_or(Error::AmountConversionFailed)
}

mod dto;

/// Default Bitget swap API base endpoint.
pub const DEFAULT_ENDPOINT: &str = "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/";

/// Bitget API path for getting swap calldata.
const SWAP_PATH: &str = "swap";

/// Bindings to the Bitget swap API.
pub struct Bitget {
    client: super::Client,
    endpoint: reqwest::Url,
    api_key: String,
    api_secret: String,
    partner_code: String,
    chain_name: dto::ChainName,
    settlement_contract: Address,
}

pub struct Config {
    /// The base URL for the Bitget swap API.
    pub endpoint: reqwest::Url,

    pub chain_id: eth::ChainId,

    pub settlement_contract: Address,

    /// Credentials used to access Bitget API.
    pub credentials: BitgetCredentialsConfig,

    /// Partner code sent in the `Partner-Code` header.
    pub partner_code: String,

    /// The stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,
}

pub struct BitgetCredentialsConfig {
    /// Bitget API key.
    pub api_key: String,

    /// Bitget API secret for signing requests.
    pub api_secret: String,
}

impl Bitget {
    pub fn try_new(config: Config) -> Result<Self, CreationError> {
        let client = {
            let client = reqwest::Client::builder().build()?;
            super::Client::new(client, config.block_stream)
        };

        let chain_name = dto::ChainName::new(config.chain_id);

        Ok(Self {
            client,
            endpoint: config.endpoint,
            api_key: config.credentials.api_key,
            api_secret: config.credentials.api_secret,
            partner_code: config.partner_code,
            chain_name,
            settlement_contract: config.settlement_contract,
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
        // Bitget only supports sell orders (exactIn).
        if order.side == order::Side::Buy {
            return Err(Error::OrderNotSupported);
        }

        let sell_decimals = tokens
            .get(&order.sell)
            .and_then(|t| t.decimals)
            .ok_or(Error::MissingDecimals)?;
        let buy_decimals = tokens
            .get(&order.buy)
            .and_then(|t| t.decimals)
            .ok_or(Error::MissingDecimals)?;

        // Set up a tracing span to make debugging of API requests easier.
        static ID: AtomicU64 = AtomicU64::new(0);
        let id = ID.fetch_add(1, atomic::Ordering::Relaxed);

        let response = self
            .handle_sell_order(order, slippage, sell_decimals)
            .instrument(tracing::trace_span!("swap", id = %id))
            .await?;

        let calldata = response
            .swap_transaction
            .decode_calldata()
            .map_err(|_| Error::InvalidCalldata)?;

        let contract = response.swap_transaction.to;

        // Increase gas estimate by 50% for safety margin, similar to OKX.
        let gas_limit = U256::from(response.gas_fee.gas_limit);
        let gas = gas_limit
            .checked_add(gas_limit / U256::from(2))
            .ok_or(Error::GasCalculationFailed)?;

        let output_amount = decimal_to_wei(&response.out_amount, buy_decimals)?;

        Ok(dex::Swap {
            calls: vec![dex::Call {
                to: contract,
                calldata,
            }],
            input: eth::Asset {
                token: order.sell,
                amount: order.amount.get(),
            },
            output: eth::Asset {
                token: order.buy,
                amount: output_amount,
            },
            allowance: dex::Allowance {
                spender: contract,
                amount: dex::Amount::new(order.amount.get()),
            },
            gas: eth::Gas(gas),
        })
    }

    /// Handle sell orders with a single enriched swap API call.
    ///
    /// Uses `requestMod = "rich"` so the swap endpoint returns both quote
    /// data (output amount, gas) and calldata in a single response,
    /// eliminating the race condition window of the previous two-call flow.
    async fn handle_sell_order(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        sell_decimals: u8,
    ) -> Result<dto::SwapResponse, Error> {
        let swap_request = dto::SwapRequest::from_order(
            order,
            self.chain_name,
            self.settlement_contract,
            slippage,
            sell_decimals,
        );

        self.send_post_request(SWAP_PATH, &swap_request).await
    }

    /// Generate HMAC-SHA256 signature for the Bitget API.
    ///
    /// The signature is computed over a JSON object with alphabetically sorted
    /// keys containing: the API path, body, API key, and timestamp.
    pub(crate) fn generate_signature(
        &self,
        api_path: &str,
        body: &str,
        timestamp: &str,
    ) -> Result<String, Error> {
        // Use `BTreeMap` for alphabetical ordering required by API (see <https://web3.bitget.com/en/docs/authentication/#signature-algorithm>).
        let mut content = BTreeMap::new();
        content.insert("apiPath", api_path);
        content.insert("body", body);
        content.insert("x-api-key", &self.api_key);
        content.insert("x-api-timestamp", timestamp);

        let content_str = serde_json::to_string(&content).map_err(|_| Error::SignRequestFailed)?;

        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes())
            .map_err(|_| Error::SignRequestFailed)?;
        mac.update(content_str.as_bytes());
        let signature = mac.finalize().into_bytes();

        Ok(BASE64_STANDARD.encode(signature))
    }

    /// Bitget error handling based on status codes.
    fn handle_api_error(status: i64, body: String) -> Result<(), Error> {
        Err(match status {
            0 => return Ok(()),
            429 => Error::RateLimited,
            404 => Error::NotFound,
            _ => Error::Api { status, body },
        })
    }

    async fn send_post_request<T, U>(&self, endpoint: &str, body: &T) -> Result<U, Error>
    where
        T: serde::Serialize,
        U: serde::de::DeserializeOwned,
    {
        let url = self
            .endpoint
            .join(endpoint)
            .map_err(|_| Error::RequestBuildFailed)?;

        let body_str = serde_json::to_string(body).map_err(|_| Error::RequestBuildFailed)?;
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let api_path = url.path();
        let signature = self.generate_signature(api_path, &body_str, &timestamp)?;

        let request_builder = self
            .client
            .request(reqwest::Method::POST, url.clone())
            .header("Content-Type", "application/json")
            .header("Partner-Code", &self.partner_code)
            .header("x-api-key", &self.api_key)
            .header("x-api-timestamp", &timestamp)
            .header("x-api-signature", &signature)
            .body(body_str);

        let response = request_builder
            .send()
            .await
            .map_err(util::http::Error::from)?;
        let status = response.status();
        let body = response.text().await.map_err(util::http::Error::from)?;

        if !status.is_success() {
            return Err(util::http::Error::Status(status, body).into());
        }

        let response: dto::Response<U> =
            serde_json::from_str(&body).map_err(util::http::Error::from)?;

        Self::handle_api_error(response.status, body)?;
        response.data.ok_or(Error::NotFound)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    #[error(transparent)]
    Client(#[from] reqwest::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to build the request")]
    RequestBuildFailed,
    #[error("failed to sign the request")]
    SignRequestFailed,
    #[error("calculating output gas failed")]
    GasCalculationFailed,
    #[error("unable to find a quote")]
    NotFound,
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("rate limited")]
    RateLimited,
    #[error("invalid calldata in response")]
    InvalidCalldata,
    #[error("failed to convert amount between decimal and U256")]
    AmountConversionFailed,
    #[error("decimals are missing for the swapped tokens")]
    MissingDecimals,
    #[error("api error status {status}: {body}")]
    Api { status: i64, body: String },
    #[error(transparent)]
    Http(#[from] util::http::Error),
}
