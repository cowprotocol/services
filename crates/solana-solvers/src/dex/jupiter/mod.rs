//! Jupiter swap-API adapter.
//!
//! One v2 `/build` request per quote returns both the route's amounts and its
//! instructions. `/build` is ExactIn only, so the adapter serves sell orders
//! only.

mod dto;

use {
    super::{Error, Order, Quote, Side, Swap},
    crate::config::JupiterConfig,
    serde::Deserialize,
    solana_sdk::pubkey::Pubkey,
};

const BUILD_PATH: &str = "swap/v2/build";

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

    /// Price `order` for the settlement signer `taker`: one `/build`
    /// request, which also returns the swap's instructions for that taker.
    pub async fn quote(&self, order: &Order, taker: &Pubkey) -> Result<Quote, Error> {
        if order.side == Side::Buy {
            return Err(Error::OrderNotSupported);
        }
        let raw = self.build(order, taker).await?;
        parse_quote(raw, order.buy_destination)
    }

    /// The swap for `quote`, from the instructions its `/build` response
    /// carries. No request: they were built for the taker the quote was
    /// priced for, so `_taker` must be that same signer.
    pub async fn swap(&self, quote: &Quote, _taker: &Pubkey) -> Result<Swap, Error> {
        dto::BuildInstructions::deserialize(&quote.raw)
            .map_err(|err| Error::BadResponse(format!("build instructions: {err}")))?
            .into_swap(quote.in_amount, quote.out_amount)
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
        let request = match &self.api_key {
            Some(key) => self.client.get(url).header("x-api-key", key),
            None => self.client.get(url),
        };
        self.send(request).await
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

/// Jupiter's `outAmount` is what it expects the route to deliver; its swap
/// instruction enforces only `otherAmountThreshold`, that expectation moved
/// down by `slippageBps`. The settlement program pays the promised amount out
/// of what the swap delivered, so a promise above the enforced bound reverts
/// the payout whenever the pool moves between the quote and the landing.
///
/// The program recomputes that bound on chain from the quoted amount and
/// `slippageBps`; its rounding is not published and may differ from the API's
/// by one atom. Promising one atom below the threshold keeps it within the
/// bound under either rounding.
fn parse_quote(raw: serde_json::Value, destination: Pubkey) -> Result<Quote, Error> {
    let amounts = dto::QuoteAmounts::deserialize(&raw)
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

    fn config() -> JupiterConfig {
        JupiterConfig {
            endpoint: "https://api.jup.ag".parse().unwrap(),
            api_key: std::env::var("JUPITER_API_KEY").ok(),
            slippage_bps: 50,
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

    /// A `/build` response trimmed to the fields we read, plus some we
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

    #[tokio::test]
    async fn buy_orders_are_not_supported() {
        let jupiter = Jupiter::new(&config()).unwrap();
        let result = jupiter
            .quote(&order(Side::Buy), &Pubkey::from_str(WSOL).unwrap())
            .await;
        assert!(matches!(result, Err(Error::OrderNotSupported)));
    }

    #[test]
    fn quote_commits_to_the_output_threshold() {
        let quote = parse_quote(
            build_response(serde_json::Value::Null),
            Pubkey::from_str(WSOL).unwrap(),
        )
        .unwrap();
        assert_eq!(
            (quote.in_amount, quote.out_amount),
            (5_000_000, 480_696_438)
        );
    }

    #[test]
    fn quote_without_amounts_is_a_bad_response() {
        let result = parse_quote(serde_json::json!({}), Pubkey::from_str(WSOL).unwrap());
        assert!(matches!(result, Err(Error::BadResponse(_))));
    }

    #[tokio::test]
    async fn swap_flattens_the_quoted_instructions_in_execution_order() {
        let jupiter = Jupiter::new(&config()).unwrap();
        for (cleanup, programs) in [
            (serde_json::Value::Null, vec![1, 2]),
            (instruction(3, &[3]), vec![1, 2, 3]),
        ] {
            let quote =
                parse_quote(build_response(cleanup), Pubkey::from_str(WSOL).unwrap()).unwrap();
            let swap = jupiter
                .swap(&quote, &Pubkey::from_str(WSOL).unwrap())
                .await
                .unwrap();
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
    async fn swap_without_instructions_is_a_bad_response() {
        let jupiter = Jupiter::new(&config()).unwrap();
        let mut raw = build_response(serde_json::Value::Null);
        raw.as_object_mut().unwrap().remove("swapInstruction");
        let quote = parse_quote(raw, Pubkey::from_str(WSOL).unwrap()).unwrap();
        let result = jupiter.swap(&quote, &Pubkey::from_str(WSOL).unwrap()).await;
        assert!(matches!(result, Err(Error::BadResponse(_))));
    }

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    #[tokio::test]
    #[ignore]
    async fn jupiter_live_sell() {
        let jupiter = Jupiter::new(&config()).unwrap();
        // Any valid pubkey works for building instructions, the swap only runs
        // for real once the driver supplies its settlement signer.
        let taker = Pubkey::from_str(WSOL).unwrap();
        let sell = order(Side::Sell);
        let quote = jupiter.quote(&sell, &taker).await.unwrap();
        let swap = jupiter.swap(&quote, &taker).await.unwrap();
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
}
