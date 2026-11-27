//! Jupiter swap-API adapter, over either API version (`api-version` in the
//! config):
//!
//! - v1: `/quote` + `/swap-instructions` (ExactIn for sells, ExactOut for
//!   buys). Triton is a base-URL and key swap behind the same adapter.
//! - v2: one `/build` request per quote returns both the route's amounts and
//!   its instructions. `/build` is ExactIn only, so v2 serves sell orders only.

mod dto;

use {
    super::{Error, Order, Quote, Side, Swap},
    crate::config::{JupiterApiVersion, JupiterConfig},
    serde::Deserialize,
    solana_sdk::pubkey::Pubkey,
};

const QUOTE_PATH: &str = "swap/v1/quote";
const SWAP_INSTRUCTIONS_PATH: &str = "swap/v1/swap-instructions";
const BUILD_PATH: &str = "swap/v2/build";

/// Jupiter's quote direction: the `swapMode` query value, echoed back in the
/// response.
#[derive(Clone, Copy, Deserialize)]
pub enum SwapMode {
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
    api_version: JupiterApiVersion,
}

impl Jupiter {
    pub fn new(config: &JupiterConfig) -> Result<Self, Error> {
        if config.api_version == JupiterApiVersion::V2 && config.enable_buy_orders {
            return Err(Error::InvalidConfig(
                "enable-buy-orders needs Jupiter api-version v1: v2 has no ExactOut",
            ));
        }
        Ok(Self {
            client: reqwest::Client::builder().build()?,
            endpoint: config.endpoint.clone(),
            api_key: config.api_key.clone(),
            slippage_bps: config.slippage_bps,
            enable_buy_orders: config.enable_buy_orders,
            api_version: config.api_version,
        })
    }

    /// Price `order` for the settlement signer `taker`.
    ///
    /// v1: one `/quote` request, the swap is built later from it. v2: one
    /// `/build` request, which also returns the swap's instructions for
    /// `taker`.
    pub async fn quote(&self, order: &Order, taker: &Pubkey) -> Result<Quote, Error> {
        match self.api_version {
            JupiterApiVersion::V1 => {
                let swap_mode = match order.side {
                    Side::Sell => SwapMode::ExactIn,
                    Side::Buy if self.enable_buy_orders => SwapMode::ExactOut,
                    Side::Buy => return Err(Error::OrderNotSupported),
                };
                let raw = self.fetch_quote(order, swap_mode).await?;
                parse_v1_quote(raw, order.buy_destination)
            }
            JupiterApiVersion::V2 => {
                if order.side == Side::Buy {
                    return Err(Error::OrderNotSupported);
                }
                let raw = self.build(order, taker).await?;
                parse_v2_quote(raw, order.buy_destination)
            }
        }
    }

    /// Build the swap for `quote` that the settlement signer `taker`
    /// executes inside the settlement transaction, at the quote's amounts.
    ///
    /// v1: one `/swap-instructions` request. v2: no request, the instructions
    /// come from the quote's `/build` response. They were built for the taker
    /// the quote was priced for, so `taker` must be that same signer.
    pub async fn swap(&self, quote: &Quote, taker: &Pubkey) -> Result<Swap, Error> {
        let instructions = match self.api_version {
            JupiterApiVersion::V1 => {
                self.swap_instructions(&quote.raw, taker, &quote.destination)
                    .await?
            }
            JupiterApiVersion::V2 => dto::SwapInstructions::deserialize(&quote.raw)
                .map_err(|err| Error::BadResponse(format!("build instructions: {err}")))?,
        };
        instructions.into_swap(quote.in_amount, quote.out_amount)
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
    ) -> Result<dto::SwapInstructions, Error> {
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

    /// `GET /swap/v2/build`. Kept opaque in the [`Quote`]: we read the amounts
    /// now and the instructions once the quote clears.
    async fn build(&self, order: &Order, taker: &Pubkey) -> Result<serde_json::Value, Error> {
        let mut url = self
            .endpoint
            .join(BUILD_PATH)
            .map_err(|_| Error::RequestBuildFailed)?;
        url.query_pairs_mut()
            .append_pair("inputMint", &order.sell_mint.to_string())
            .append_pair("outputMint", &order.buy_mint.to_string())
            .append_pair("amount", &order.amount().to_string())
            .append_pair("taker", &taker.to_string())
            .append_pair(
                "destinationTokenAccount",
                &order.buy_destination.to_string(),
            )
            // Jupiter bakes the resulting bounds into the returned instruction
            // data; nothing downstream re-applies slippage.
            .append_pair("slippageBps", &self.slippage_bps.to_string())
            // The swap rides inside the settlement transaction, which moves
            // only SPL tokens.
            .append_pair("wrapAndUnwrapSol", "false")
            // The driver settles with v1 transactions, which have no address
            // lookup tables, and sets their compute budget itself.
            .append_pair("transactionVersion", "1")
            // The swap pays into the settlement's buffer, which the taker does
            // not own, and GoonFi V2 fails that with `InvalidAccountOwner`.
            .append_pair("excludeDexes", "GoonFi V2");
        self.send(self.with_key(self.client.get(url))).await
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

/// Jupiter's `inAmount` and `outAmount` are what it expects the route to
/// move. Its swap instruction enforces only `otherAmountThreshold`, that
/// expectation moved by `slippageBps`. The settlement program pays the
/// promised amount out of the buffer the swap delivers into, so a promise
/// above the enforced bound reverts the payout whenever the pool moves
/// between the quote and the landing and the buffer held less than the gap.
/// The quote promises the bound and keeps `outAmount` as the expected output.
///
/// The program recomputes that bound on chain from the quoted amount and
/// `slippageBps`; its rounding is not published and may differ from the API's
/// by one atom. Stepping the promise one atom past the threshold on each side
/// keeps it within the bound under either rounding.
fn parse_v1_quote(raw: serde_json::Value, destination: Pubkey) -> Result<Quote, Error> {
    let amounts = dto::QuoteAmounts::deserialize(&raw)
        .map_err(|err| Error::BadResponse(format!("quote: {err}")))?;
    let (in_amount, out_amount) = match amounts.swap_mode {
        SwapMode::ExactIn => (
            amounts.in_amount,
            amounts.other_amount_threshold.saturating_sub(1),
        ),
        SwapMode::ExactOut => (
            amounts.other_amount_threshold.saturating_add(1),
            amounts.out_amount,
        ),
    };
    Ok(Quote {
        in_amount,
        out_amount,
        expected_out_amount: amounts.out_amount,
        destination,
        raw,
    })
}

/// The v2 counterpart of [`parse_v1_quote`]: `/build` is always ExactIn, so
/// the promise is one atom below `otherAmountThreshold`, for the same
/// reasons.
fn parse_v2_quote(raw: serde_json::Value, destination: Pubkey) -> Result<Quote, Error> {
    let amounts = dto::BuildAmounts::deserialize(&raw)
        .map_err(|err| Error::BadResponse(format!("quote: {err}")))?;
    Ok(Quote {
        in_amount: amounts.in_amount,
        out_amount: amounts.other_amount_threshold.saturating_sub(1),
        destination,
        raw,
    })
}

#[cfg(test)]
mod tests {
    use {super::*, base64::prelude::*, std::str::FromStr};

    // USDC and wrapped SOL mints.
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const WSOL: &str = "So11111111111111111111111111111111111111112";

    fn config(api_version: JupiterApiVersion, enable_buy_orders: bool) -> JupiterConfig {
        JupiterConfig {
            endpoint: "https://api.jup.ag".parse().unwrap(),
            api_key: std::env::var("JUPITER_API_KEY").ok(),
            slippage_bps: 50,
            enable_buy_orders,
            api_version,
        }
    }

    fn jupiter(api_version: JupiterApiVersion, enable_buy_orders: bool) -> Jupiter {
        Jupiter::new(&config(api_version, enable_buy_orders)).unwrap()
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

    fn taker() -> Pubkey {
        Pubkey::from_str(WSOL).unwrap()
    }

    fn instruction(program: u8, data: &[u8]) -> serde_json::Value {
        serde_json::json!({
            "programId": Pubkey::new_from_array([program; 32]).to_string(),
            "accounts": [{
                "pubkey": Pubkey::new_from_array([9; 32]).to_string(),
                "isSigner": false,
                "isWritable": true,
            }],
            "data": BASE64_STANDARD.encode(data),
        })
    }

    /// A v2 `/build` response trimmed to the fields we read, plus some we
    /// ignore.
    fn build_response(cleanup: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "inAmount": "5000000",
            "outAmount": "483111998",
            "otherAmountThreshold": "480696439",
            "swapMode": "ExactIn",
            "computeBudgetInstructions": [],
            "setupInstructions": [instruction(1, &[1])],
            "swapInstruction": instruction(2, &[2, 2]),
            "cleanupInstruction": cleanup,
            "otherInstructions": [],
            "tipInstruction": null,
            "addressesByLookupTableAddress": {},
        })
    }

    #[test]
    fn v2_rejects_buy_orders_in_the_config() {
        let result = Jupiter::new(&config(JupiterApiVersion::V2, true));
        assert!(matches!(result, Err(Error::InvalidConfig(_))));
        // v1 takes the same config.
        Jupiter::new(&config(JupiterApiVersion::V1, true)).unwrap();
    }

    #[tokio::test]
    async fn buy_orders_are_not_supported_unless_enabled_on_v1() {
        for api_version in [JupiterApiVersion::V1, JupiterApiVersion::V2] {
            let result = jupiter(api_version, false)
                .quote(&order(Side::Buy), &taker())
                .await;
            assert!(matches!(result, Err(Error::OrderNotSupported)));
        }
    }

    #[test]
    fn v1_sell_quote_commits_to_the_output_threshold() {
        let raw = serde_json::json!({
            "inAmount": "5000000",
            "outAmount": "483111998",
            "otherAmountThreshold": "480696439",
            "swapMode": "ExactIn",
        });
        let quote = parse_v1_quote(raw, taker()).unwrap();
        assert_eq!(
            (quote.in_amount, quote.out_amount),
            (5_000_000, 480_696_438)
        );
        assert_eq!(quote.expected_out_amount, 483_111_998);
    }

    #[test]
    fn v1_buy_quote_commits_to_the_input_threshold() {
        let raw = serde_json::json!({
            "inAmount": "1000",
            "outAmount": "1000000",
            "otherAmountThreshold": "1005",
            "swapMode": "ExactOut",
        });
        let quote = parse_v1_quote(raw, taker()).unwrap();
        assert_eq!((quote.in_amount, quote.out_amount), (1_006, 1_000_000));
        assert_eq!(quote.expected_out_amount, 1_000_000);
    }

    #[test]
    fn v1_quote_without_amounts_is_a_bad_response() {
        let result = parse_v1_quote(serde_json::json!({}), taker());
        assert!(matches!(result, Err(Error::BadResponse(_))));
    }

    #[test]
    fn v2_quote_commits_to_the_output_threshold() {
        let quote = parse_v2_quote(build_response(serde_json::Value::Null), taker()).unwrap();
        assert_eq!(
            (quote.in_amount, quote.out_amount),
            (5_000_000, 480_696_438)
        );
    }

    #[test]
    fn v2_quote_without_amounts_is_a_bad_response() {
        let result = parse_v2_quote(serde_json::json!({}), taker());
        assert!(matches!(result, Err(Error::BadResponse(_))));
    }

    #[tokio::test]
    async fn v2_swap_flattens_the_quoted_instructions_in_execution_order() {
        let jupiter = jupiter(JupiterApiVersion::V2, false);
        for (cleanup, programs) in [
            (serde_json::Value::Null, vec![1, 2]),
            (instruction(3, &[3]), vec![1, 2, 3]),
        ] {
            let quote = parse_v2_quote(build_response(cleanup), taker()).unwrap();
            let swap = jupiter.swap(&quote, &taker()).await.unwrap();
            assert_eq!(
                (swap.in_amount, swap.out_amount),
                (quote.in_amount, quote.out_amount)
            );
            assert_eq!(
                swap.instructions
                    .iter()
                    .map(|instruction| instruction.program_id)
                    .collect::<Vec<_>>(),
                programs
                    .into_iter()
                    .map(|program| Pubkey::new_from_array([program; 32]))
                    .collect::<Vec<_>>()
            );
            assert_eq!(swap.instructions[1].data, vec![2, 2]);
        }
    }

    #[tokio::test]
    async fn v2_swap_without_instructions_is_a_bad_response() {
        let jupiter = jupiter(JupiterApiVersion::V2, false);
        let mut raw = build_response(serde_json::Value::Null);
        raw.as_object_mut().unwrap().remove("swapInstruction");
        let quote = parse_v2_quote(raw, taker()).unwrap();
        let result = jupiter.swap(&quote, &taker()).await;
        assert!(matches!(result, Err(Error::BadResponse(_))));
    }

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    async fn live_sell(api_version: JupiterApiVersion) {
        let jupiter = jupiter(api_version, false);
        // Any valid pubkey works for building instructions, the swap only runs
        // for real once the driver supplies its settlement signer.
        let sell = order(Side::Sell);
        let quote = jupiter.quote(&sell, &taker()).await.unwrap();
        let swap = jupiter.swap(&quote, &taker()).await.unwrap();
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

    #[tokio::test]
    #[ignore]
    async fn jupiter_live_sell_v1() {
        live_sell(JupiterApiVersion::V1).await;
    }

    #[tokio::test]
    #[ignore]
    async fn jupiter_live_sell_v2() {
        live_sell(JupiterApiVersion::V2).await;
    }

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    #[tokio::test]
    #[ignore]
    async fn jupiter_live_buy_v1() {
        let jupiter = jupiter(JupiterApiVersion::V1, true);
        // ExactOut: the swap delivers exactly the order's `amount` of buy mint.
        let quote = jupiter.quote(&order(Side::Buy), &taker()).await.unwrap();
        let swap = jupiter.swap(&quote, &taker()).await.unwrap();
        assert_eq!(swap.out_amount, 1_000_000);
        assert!(swap.in_amount > 0);
        assert!(!swap.instructions.is_empty());
    }
}
