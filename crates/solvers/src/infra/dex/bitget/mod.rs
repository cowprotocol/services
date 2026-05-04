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
    number::conversions::{big_decimal_to_u256, u256_to_big_uint},
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
    BigDecimal::new(u256_to_big_uint(&amount).into(), i64::from(decimals)).normalized()
}

/// Convert a decimal amount (from API response) to U256 wei.
/// e.g., "1964.365496" with 6 decimals → 1964365496
fn decimal_to_wei(amount: &BigDecimal, decimals: u8) -> Result<U256, Error> {
    let scaled = amount * BigDecimal::new(1.into(), -i64::from(decimals));
    big_decimal_to_u256(&scaled).ok_or(Error::AmountConversionFailed)
}

mod dto;

/// Default Bitget swap API base endpoint.
pub const DEFAULT_ENDPOINT: &str = "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/";

/// Default API path used for signature computation. When a proxy endpoint is
/// configured, the request URL differs from the canonical path that Bitget
/// expects in the signature.
const SIGNATURE_API_PATH: &str = "/bgw-pro/swapx/pro/";

/// Bitget API path for getting swap calldata (sell orders, exact-in).
const SWAP_PATH: &str = "swap";

/// Bitget API path for the reverse-quote swap endpoint (buy orders,
/// `requestMode = "minAmountOut"`).
const SWAP_REVERSE_PATH: &str = "swapr";

/// Bindings to the Bitget swap API.
pub struct Bitget {
    client: super::Client,
    endpoint: reqwest::Url,
    api_key: String,
    api_secret: String,
    partner_code: String,
    chain_name: dto::ChainName,
    settlement_contract: Address,
    enable_buy_orders: bool,
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

    /// Whether buy orders should be served via the reverse-quote endpoint.
    /// Disabled by default since the on-chain overshoot accrues to the
    /// settlement buffer rather than to the user, costing up to the
    /// configured slippage in user surplus.
    pub enable_buy_orders: bool,
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
            enable_buy_orders: config.enable_buy_orders,
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
        if order.side == order::Side::Buy && !self.enable_buy_orders {
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
        let span = tracing::trace_span!("swap", id = %id);

        match order.side {
            order::Side::Sell => {
                self.handle_sell_order(order, slippage, sell_decimals, buy_decimals)
                    .instrument(span)
                    .await
            }
            order::Side::Buy => {
                self.handle_buy_order(order, slippage, sell_decimals, buy_decimals)
                    .instrument(span)
                    .await
            }
        }
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
        buy_decimals: u8,
    ) -> Result<dex::Swap, Error> {
        let swap_request = dto::SwapRequest::from_order(
            order,
            self.chain_name,
            self.settlement_contract,
            slippage,
            sell_decimals,
        );
        let response: dto::SwapResponse = self.send_post_request(SWAP_PATH, &swap_request).await?;

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

    /// Handle buy orders via the reverse-quote endpoint.
    ///
    /// Bitget computes the required input amount server-side using a recursive
    /// search on top of sell quotes. The response calldata enforces
    /// `minAmountOut` on chain, so the swap reverts if it underdelivers.
    async fn handle_buy_order(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        sell_decimals: u8,
        buy_decimals: u8,
    ) -> Result<dex::Swap, Error> {
        let swap_request = dto::ReverseSwapRequest::from_order(
            order,
            self.chain_name,
            self.settlement_contract,
            slippage,
            buy_decimals,
        );
        let response: dto::ReverseSwapResponse = self
            .send_post_request(SWAP_REVERSE_PATH, &swap_request)
            .await?;

        let tx = response.txs.first().ok_or(Error::NotFound)?;
        let calldata = tx.decode_calldata().map_err(|_| Error::InvalidCalldata)?;
        let contract = tx.to;

        // Increase gas estimate by 50% for safety margin, similar to OKX.
        let gas_limit = U256::from(tx.gas_limit);
        let gas = gas_limit
            .checked_add(gas_limit / U256::from(2))
            .ok_or(Error::GasCalculationFailed)?;

        let input_amount = decimal_to_wei(&response.amount_in, sell_decimals)?;
        let expected_out = decimal_to_wei(&response.expected_amount_out, buy_decimals)?;

        // CoW buy orders require the executed buy amount to equal the order's
        // buy amount exactly. BitGet's reverse-quote enforces a *minimum*
        // output on chain, so the swap may deliver slightly more, but we
        // report the exact buy amount here so `Fulfillment::new` accepts the
        // solution. The slight on-chain overshoot accrues to the settlement
        // contract buffer as positive surplus.
        if expected_out < order.amount.get() {
            return Err(Error::NotFound);
        }
        let output_amount = order.amount.get();

        Ok(dex::Swap {
            calls: vec![dex::Call {
                to: contract,
                calldata,
            }],
            input: eth::Asset {
                token: order.sell,
                amount: input_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: output_amount,
            },
            allowance: dex::Allowance {
                spender: contract,
                amount: dex::Amount::new(input_amount),
            },
            gas: eth::Gas(gas),
        })
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

    /// Bitget error handling based on `error_code`.
    ///
    /// See <https://web3.bitget.com/en/docs/swap-order#error-code-list>
    fn handle_api_error(
        status: i64,
        error_code: Option<i64>,
        message: String,
    ) -> Result<(), Error> {
        if status == 0 {
            return Ok(());
        }

        Err(match error_code.unwrap_or(80000) {
            80001 // Insufficient token balance
            | 80004 // Order expired
            | 80005 // Insufficient liquidity
            | 80008 // Reverse quote did not converge
            | 80009 // Token info not found
            | 80010 // Price/gas price not found
            | 80011 // Failed to generate calldata
            | 80012 // Quote failed
            | 80014 // Order not found
            => Error::NotFound,
            80002 // Amount below minimum
            | 80003 // Amount above maximum
            | 80006 // Illegal request
            | 80013 // Unsupported chain
            => Error::BadRequest,
            code => Error::Api { code, message },
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
        let signature_path = format!("{SIGNATURE_API_PATH}{endpoint}");
        let signature = self.generate_signature(&signature_path, &body_str, &timestamp)?;

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

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }
        if !status.is_success() {
            return Err(util::http::Error::Status(status, body).into());
        }

        let response: dto::Response<U> =
            serde_json::from_str(&body).map_err(util::http::Error::from)?;

        Self::handle_api_error(
            response.status,
            response.error_code,
            response.message.unwrap_or_default(),
        )?;
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
    #[error("bad request")]
    BadRequest,
    #[error("api error code {code}: {message}")]
    Api { code: i64, message: String },
    #[error(transparent)]
    Http(#[from] util::http::Error),
}
