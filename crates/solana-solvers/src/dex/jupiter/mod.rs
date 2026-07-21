//! Jupiter swap-API adapter.
//!
//! v1 `/quote` + `/swap-instructions` (ExactIn for sells, ExactOut for buys).
//! Triton is a base-URL and key swap behind the same adapter.

mod dto;

use {
    super::{Order, Side, Swap},
    crate::config::JupiterConfig,
    solana_sdk::pubkey::Pubkey,
};

const QUOTE_PATH: &str = "swap/v1/quote";
const SWAP_INSTRUCTIONS_PATH: &str = "swap/v1/swap-instructions";

/// Jupiter's quote direction, rendered as the `swapMode` query value.
#[derive(Clone, Copy)]
enum SwapMode {
    ExactIn,
    ExactOut,
}

impl SwapMode {
    fn as_str(self) -> &'static str {
        match self {
            SwapMode::ExactIn => "ExactIn",
            SwapMode::ExactOut => "ExactOut",
        }
    }
}

/// Adapter over the Jupiter swap API.
pub struct Jupiter {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    api_key: Option<String>,
    slippage_bps: u16,
    enable_buy_orders: bool,
}

impl Jupiter {
    pub fn new(config: &JupiterConfig) -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
            endpoint: config.endpoint.clone(),
            api_key: config.api_key.clone(),
            slippage_bps: config.slippage_bps,
            enable_buy_orders: config.enable_buy_orders,
        })
    }

    /// Quote `order` for the settlement signer `taker` and return the swap to
    /// run inside the settlement transaction.
    pub async fn swap(&self, order: &Order, taker: &Pubkey) -> Result<Swap, Error> {
        let swap_mode = match order.side {
            Side::Sell => SwapMode::ExactIn,
            Side::Buy if self.enable_buy_orders => SwapMode::ExactOut,
            Side::Buy => return Err(Error::OrderNotSupported),
        };
        let quote = self.quote(order, swap_mode).await?;
        let in_amount = amount_field(&quote, "inAmount")?;
        let out_amount = amount_field(&quote, "outAmount")?;
        self.swap_instructions(&quote, taker, &order.buy_destination)
            .await?
            .into_swap(in_amount, out_amount)
    }

    /// `GET /swap/v1/quote`. Kept opaque and passed back verbatim to
    /// `/swap-instructions`, we only read the amounts.
    async fn quote(&self, order: &Order, swap_mode: SwapMode) -> Result<serde_json::Value, Error> {
        let mut url = self
            .endpoint
            .join(QUOTE_PATH)
            .map_err(|_| Error::RequestBuildFailed)?;
        url.query_pairs_mut()
            .append_pair("inputMint", &order.sell_mint.to_string())
            .append_pair("outputMint", &order.buy_mint.to_string())
            .append_pair("amount", &order.amount.to_string())
            .append_pair("swapMode", swap_mode.as_str())
            // Jupiter bakes the resulting bounds into the returned instruction
            // data; nothing downstream re-applies slippage.
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

    fn config(enable_buy_orders: bool) -> JupiterConfig {
        JupiterConfig {
            endpoint: "https://api.jup.ag".parse().unwrap(),
            api_key: std::env::var("JUPITER_API_KEY").ok(),
            slippage_bps: 50,
            enable_buy_orders,
        }
    }

    fn order(side: Side) -> Order {
        Order {
            sell_mint: Pubkey::from_str(USDC).unwrap(),
            buy_mint: Pubkey::from_str(WSOL).unwrap(),
            buy_destination: Pubkey::from_str(WSOL).unwrap(),
            amount: 1_000_000,
            side,
        }
    }

    #[tokio::test]
    async fn buy_disabled() {
        let jupiter = Jupiter::new(&config(false)).unwrap();
        let result = jupiter.swap(&order(Side::Buy), &Pubkey::new_unique()).await;
        assert!(matches!(result, Err(Error::OrderNotSupported)));
    }

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    #[tokio::test]
    #[ignore]
    async fn jupiter_live_sell() {
        let jupiter = Jupiter::new(&config(false)).unwrap();
        // Any valid pubkey works for building instructions, the swap only runs
        // for real once the driver supplies its settlement signer.
        let sell = order(Side::Sell);
        let swap = jupiter
            .swap(&sell, &Pubkey::from_str(WSOL).unwrap())
            .await
            .unwrap();
        assert_eq!(swap.in_amount, 1_000_000);
        assert!(swap.out_amount > 0);
        assert!(!swap.instructions.is_empty());

        // End to end: the live swap assembles into a valid solution that
        // serializes.
        let solution = crate::domain::solution::Solution::single(
            0,
            crate::domain::order::OrderUid([1; 32]),
            &sell,
            swap,
        )
        .unwrap();
        assert_eq!(solution.trades.len(), 1);
        assert!(!solution.interactions.is_empty());
        serde_json::to_string(&solution).unwrap();
    }

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    #[tokio::test]
    #[ignore]
    async fn jupiter_live_buy() {
        let jupiter = Jupiter::new(&config(true)).unwrap();
        // ExactOut: the swap delivers exactly the order's `amount` of buy mint.
        let swap = jupiter
            .swap(&order(Side::Buy), &Pubkey::from_str(WSOL).unwrap())
            .await
            .unwrap();
        assert_eq!(swap.out_amount, 1_000_000);
        assert!(swap.in_amount > 0);
        assert!(!swap.instructions.is_empty());
    }
}
