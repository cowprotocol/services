//! Jupiter swap-API adapter.
//!
//! v1 `/quote` + `/swap-instructions` (ExactIn). Triton is a base-URL and key
//! swap behind the same adapter.

mod dto;

use {
    super::{Order, Side, Swap},
    crate::config::JupiterConfig,
    solana_sdk::pubkey::Pubkey,
};

const QUOTE_PATH: &str = "swap/v1/quote";
const SWAP_INSTRUCTIONS_PATH: &str = "swap/v1/swap-instructions";

/// Adapter over the Jupiter swap API.
pub struct Jupiter {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    api_key: Option<String>,
    slippage_bps: u16,
}

impl Jupiter {
    pub fn new(config: &JupiterConfig) -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
            endpoint: config.endpoint.clone(),
            api_key: config.api_key.clone(),
            slippage_bps: config.slippage_bps,
        })
    }

    /// Quote `order` for the settlement signer `taker` and return the swap to
    /// run inside the settlement transaction.
    pub async fn swap(&self, order: &Order, taker: &Pubkey) -> Result<Swap, Error> {
        // Buy orders (ExactOut) aren't served here.
        if order.side == Side::Buy {
            return Err(Error::OrderNotSupported);
        }
        let quote = self.quote(order, "ExactIn").await?;
        let in_amount = amount_field(&quote, "inAmount")?;
        let out_amount = amount_field(&quote, "outAmount")?;
        self.swap_instructions(&quote, taker, &order.buy_destination)
            .await?
            .into_swap(in_amount, out_amount)
    }

    /// `GET /swap/v1/quote`. Kept opaque and passed back verbatim to
    /// `/swap-instructions`, we only read the amounts.
    async fn quote(&self, order: &Order, swap_mode: &str) -> Result<serde_json::Value, Error> {
        let mut url = self
            .endpoint
            .join(QUOTE_PATH)
            .map_err(|_| Error::RequestBuildFailed)?;
        url.query_pairs_mut()
            .append_pair("inputMint", &order.sell_mint.to_string())
            .append_pair("outputMint", &order.buy_mint.to_string())
            .append_pair("amount", &order.amount.to_string())
            .append_pair("swapMode", swap_mode)
            .append_pair("slippageBps", &self.slippage_bps.to_string());
        self.send(self.with_key(self.client.get(url))).await
    }

    /// `POST /swap/v1/swap-instructions` for the given quote.
    async fn swap_instructions(
        &self,
        quote: &serde_json::Value,
        taker: &Pubkey,
        destination: &Pubkey,
    ) -> Result<dto::SwapInstructionsResponse, Error> {
        let url = self
            .endpoint
            .join(SWAP_INSTRUCTIONS_PATH)
            .map_err(|_| Error::RequestBuildFailed)?;
        let payload = dto::SwapInstructionsRequest::new(quote, taker, destination);
        let body = serde_json::to_string(&payload).map_err(|_| Error::RequestBuildFailed)?;
        let request = self.with_key(
            self.client
                .post(url)
                .header("content-type", "application/json")
                .body(body),
        );
        self.send(request).await
    }

    fn with_key(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => request.header("x-api-key", key),
            None => request,
        }
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            return serde_json::from_str(&body)
                .map_err(|err| Error::BadResponse(format!("response body: {err}")));
        }
        match status {
            reqwest::StatusCode::TOO_MANY_REQUESTS => Err(Error::RateLimited),
            // Jupiter returns 400 with an error body when it can't route the order.
            reqwest::StatusCode::BAD_REQUEST => Err(Error::NotFound),
            // Auth or server errors: carry the status and body so the cause shows.
            _ => Err(Error::Api {
                status: status.as_u16(),
                body,
            }),
        }
    }
}

fn amount_field(quote: &serde_json::Value, field: &str) -> Result<u64, Error> {
    quote
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(|amount| amount.parse().ok())
        .ok_or_else(|| Error::BadResponse(format!("missing or non-numeric {field}")))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to build the request")]
    RequestBuildFailed,
    #[error("no route for this order")]
    NotFound,
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("rate limited")]
    RateLimited,
    #[error("jupiter api error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("malformed swap response: {0}")]
    BadResponse(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    // USDC and wrapped SOL mints.
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const WSOL: &str = "So11111111111111111111111111111111111111112";

    fn config() -> JupiterConfig {
        JupiterConfig {
            endpoint: "https://api.jup.ag".parse().unwrap(),
            api_key: std::env::var("JUPITER_API_KEY").ok(),
            slippage_bps: 50,
        }
    }

    fn sell_order() -> Order {
        Order {
            sell_mint: Pubkey::from_str(USDC).unwrap(),
            buy_mint: Pubkey::from_str(WSOL).unwrap(),
            buy_destination: Pubkey::from_str(WSOL).unwrap(),
            amount: 1_000_000,
            side: Side::Sell,
        }
    }

    #[tokio::test]
    async fn buy_unsupported() {
        let jupiter = Jupiter::new(&config()).unwrap();
        let order = Order {
            side: Side::Buy,
            ..sell_order()
        };
        let result = jupiter.swap(&order, &Pubkey::new_unique()).await;
        assert!(matches!(result, Err(Error::OrderNotSupported)));
    }

    #[test]
    fn request_serializes_pubkeys_as_base58() {
        let quote = serde_json::json!({});
        let taker = Pubkey::from_str(WSOL).unwrap();
        let destination = Pubkey::from_str(USDC).unwrap();
        let payload = dto::SwapInstructionsRequest::new(&quote, &taker, &destination);
        let value = serde_json::to_value(payload).unwrap();
        assert_eq!(value["userPublicKey"], WSOL);
        assert_eq!(value["destinationTokenAccount"], USDC);
        assert_eq!(value["wrapAndUnwrapSol"], false);
        assert_eq!(value["skipUserAccountsRpcCalls"], true);
    }

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    #[tokio::test]
    #[ignore]
    async fn jupiter_live_sell() {
        let jupiter = Jupiter::new(&config()).unwrap();
        // Any valid pubkey works for building instructions, the swap only runs
        // for real once the driver supplies its settlement signer.
        let swap = jupiter
            .swap(&sell_order(), &Pubkey::from_str(WSOL).unwrap())
            .await
            .unwrap();
        assert_eq!(swap.in_amount, 1_000_000);
        assert!(swap.out_amount > 0);
        assert!(!swap.instructions.is_empty());
    }
}
