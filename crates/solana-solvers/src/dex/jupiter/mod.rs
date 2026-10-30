//! Jupiter swap-API adapter.
//!
//! v1 `/quote` + `/swap-instructions` (ExactIn for sells, ExactOut for buys).
//! Triton is a base-URL and key swap behind the same adapter.

mod dto;

use {
    super::{Error, Order, Quote, Side, Swap},
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

    /// Price `order`: one `/quote` request, the swap is built later from it.
    pub async fn quote(&self, order: &Order) -> Result<Quote, Error> {
        let swap_mode = match order.side {
            Side::Sell => SwapMode::ExactIn,
            Side::Buy if self.enable_buy_orders => SwapMode::ExactOut,
            Side::Buy => return Err(Error::OrderNotSupported),
        };
        let raw = self.fetch_quote(order, swap_mode).await?;
        parse_quote(raw, swap_mode, order.buy_destination)
    }

    /// Build the swap for `quote` that the settlement signer `taker`
    /// executes inside the settlement transaction: one `/swap-instructions`
    /// request, at the quote's amounts.
    pub async fn swap(&self, quote: &Quote, taker: &Pubkey) -> Result<Swap, Error> {
        self.swap_instructions(&quote.raw, taker, &quote.destination)
            .await?
            .into_swap(quote.in_amount, quote.out_amount)
    }

    /// `GET /swap/v1/quote`. Kept opaque and passed back verbatim to
    /// `/swap-instructions`, we only read the amounts.
    async fn fetch_quote(
        &self,
        order: &Order,
        swap_mode: SwapMode,
    ) -> Result<serde_json::Value, Error> {
        let mut url = self
            .endpoint
            .join(QUOTE_PATH)
            .map_err(|_| Error::RequestBuildFailed)?;
        url.query_pairs_mut()
            .append_pair("inputMint", &order.sell_mint.to_string())
            .append_pair("outputMint", &order.buy_mint.to_string())
            .append_pair("amount", &order.amount().to_string())
            .append_pair("swapMode", swap_mode.as_str())
            // Jupiter bakes the resulting bounds into the returned instruction
            // data; nothing downstream re-applies slippage.
            .append_pair("slippageBps", &self.slippage_bps.to_string())
            // The swap pays into the settlement's buffer, which the taker does
            // not own, and GoonFi V2 fails that with `InvalidAccountOwner`.
            .append_pair("excludeDexes", "GoonFi V2");
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
            // Jupiter returns 400 with an error body when it can't route the
            // order.
            reqwest::StatusCode::BAD_REQUEST => Err(Error::NotFound),
            // Auth or server errors: carry the status and body so the cause
            // shows.
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

/// Jupiter's `inAmount` and `outAmount` are what it expects the route to
/// move; its swap instruction enforces only `otherAmountThreshold`, that
/// expectation moved by `slippageBps`. The settlement program pays the
/// promised amount out of what the swap delivered, so a promise above the
/// enforced bound reverts the payout whenever the pool moves between the
/// quote and the landing.
fn parse_quote(
    raw: serde_json::Value,
    swap_mode: SwapMode,
    destination: Pubkey,
) -> Result<Quote, Error> {
    let threshold = amount_field(&raw, "otherAmountThreshold")?;
    let (in_amount, out_amount) = match swap_mode {
        SwapMode::ExactIn => (amount_field(&raw, "inAmount")?, threshold),
        SwapMode::ExactOut => (threshold, amount_field(&raw, "outAmount")?),
    };
    Ok(Quote {
        in_amount,
        out_amount,
        destination,
        raw,
    })
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
        // Open limit legs: the adapter never checks them.
        let (sell_amount, buy_amount) = match side {
            Side::Sell => (1_000_000, 0),
            Side::Buy => (u64::MAX, 1_000_000),
        };
        Order {
            sell_mint: Pubkey::from_str(USDC).unwrap(),
            buy_mint: Pubkey::from_str(WSOL).unwrap(),
            buy_destination: Pubkey::from_str(WSOL).unwrap(),
            sell_amount,
            buy_amount,
            side,
        }
    }

    #[tokio::test]
    async fn buy_disabled() {
        let jupiter = Jupiter::new(&config(false)).unwrap();
        let result = jupiter.quote(&order(Side::Buy)).await;
        assert!(matches!(result, Err(Error::OrderNotSupported)));
    }

    #[test]
    fn sell_quote_commits_to_the_output_threshold() {
        let raw = serde_json::json!({
            "inAmount": "5000000",
            "outAmount": "483111998",
            "otherAmountThreshold": "480696439",
        });
        let quote = parse_quote(raw, SwapMode::ExactIn, Pubkey::from_str(WSOL).unwrap()).unwrap();
        assert_eq!(
            (quote.in_amount, quote.out_amount),
            (5_000_000, 480_696_439)
        );
    }

    #[test]
    fn buy_quote_commits_to_the_input_threshold() {
        let raw = serde_json::json!({
            "inAmount": "1000",
            "outAmount": "1000000",
            "otherAmountThreshold": "1005",
        });
        let quote = parse_quote(raw, SwapMode::ExactOut, Pubkey::from_str(WSOL).unwrap()).unwrap();
        assert_eq!((quote.in_amount, quote.out_amount), (1_005, 1_000_000));
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
        let quote = jupiter.quote(&sell).await.unwrap();
        let swap = jupiter
            .swap(&quote, &Pubkey::from_str(WSOL).unwrap())
            .await
            .unwrap();
        assert_eq!(
            (swap.in_amount, swap.out_amount),
            (quote.in_amount, quote.out_amount)
        );
        assert_eq!(swap.in_amount, 1_000_000);
        assert!(swap.out_amount > 0);
        assert!(!swap.instructions.is_empty());

        // End to end: the live swap assembles into a valid solution that
        // serializes.
        let solution = crate::dto::solution::Solution::new(
            0,
            crate::dto::order::OrderUid([1; 32]),
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
        let quote = jupiter.quote(&order(Side::Buy)).await.unwrap();
        let swap = jupiter
            .swap(&quote, &Pubkey::from_str(WSOL).unwrap())
            .await
            .unwrap();
        assert_eq!(swap.out_amount, 1_000_000);
        assert!(swap.in_amount > 0);
        assert!(!swap.instructions.is_empty());
    }
}
