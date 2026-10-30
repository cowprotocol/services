//! DTOs for Bitget's `/swap` request and response.

use {
    crate::dex::{Error, Order, Quote, Swap, dto::ApiInstruction},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
};

/// Bitget's aggregator. Naming it lets `/swap` skip the `/quote` call.
const MARKET: &str = "bgwaggregator";
/// Bitget's smallest fee. Without a fee its route also needs Bitget's
/// co-signature, and the settlement has a single signer.
const FEE_RATE: f64 = 0.0001;

/// Body of the `/swap` request for a sell order, amounts in atoms.
#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    from_chain: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    from_contract: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    from_amount: u64,
    to_chain: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    to_contract: Pubkey,
    amount_unit: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    from_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    to_address: Pubkey,
    market: &'static str,
    /// Percent.
    slippage: f64,
    fee_rate: f64,
    request_mod: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    sol_max_accounts: u16,
}

impl SwapRequest {
    pub fn new(order: &Order, taker: &Pubkey, slippage_bps: u16, max_accounts: u16) -> Self {
        Self {
            from_chain: "sol",
            from_contract: order.sell_mint,
            from_amount: order.sell_amount,
            to_chain: "sol",
            to_contract: order.buy_mint,
            amount_unit: "wei",
            from_address: *taker,
            to_address: *taker,
            market: MARKET,
            slippage: f64::from(slippage_bps) / 100.0,
            fee_rate: FEE_RATE,
            request_mod: "rich",
            sol_max_accounts: max_accounts,
        }
    }
}

/// Bitget's response wrapper: `status` 0 carries `data`, any other an
/// [`error_code`](https://web3.bitget.com/en/docs/swap-order#error-code-list).
#[derive(Deserialize)]
pub struct Envelope {
    status: i64,
    error_code: Option<i64>,
    msg: Option<String>,
    data: Option<serde_json::Value>,
}

impl Envelope {
    pub fn into_data(self) -> Result<SwapResponse, Error> {
        if self.status != 0 {
            let code = self.error_code.unwrap_or_default();
            return Err(match code {
                // No route, or a Bitget-side failure the next auction retries.
                80000 | 80001 | 80004 | 80005 | 80008..=80012 | 80014 | 80020 => Error::NotFound,
                _ => Error::Dex {
                    code,
                    message: self.msg.unwrap_or_default(),
                },
            });
        }
        let data = self.data.ok_or(Error::NotFound)?;
        serde_json::from_value(data).map_err(|err| Error::BadResponse(format!("swap data: {err}")))
    }
}

/// The parts of a rich-mode `/swap` response we need. Bitget's program checks
/// `minAmount` against what the receiver gets after its fee.
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[serde_as(as = "DisplayFromStr")]
    min_amount: u64,
    instruction_lists: Vec<ApiInstruction>,
}

impl SwapResponse {
    /// The quote for `order`, bidding `minAmount`, keeping the route to build
    /// its swap.
    pub fn into_quote(self, order: &Order) -> Quote {
        Quote {
            in_amount: order.sell_amount,
            out_amount: self.min_amount,
            order: order.clone(),
            raw: serde_json::to_value(self).expect("a quote serializes"),
        }
    }
}

/// The route, its ComputeBudget instructions dropped: the driver sets its
/// own. Bitget pays the taker's buy-mint ATA, as [`crate::dex::Dex::swap`]
/// expects.
pub fn swap(quote: &Quote, taker: &Pubkey) -> Result<Swap, Error> {
    let response = SwapResponse::deserialize(&quote.raw)
        .map_err(|err| Error::BadResponse(format!("quote: {err}")))?;
    let mut instructions = Vec::with_capacity(response.instruction_lists.len());
    for instruction in response.instruction_lists {
        let instruction = instruction.into_instruction()?;
        if instruction.program_id != solana_compute_budget_interface::ID {
            instructions.push(instruction);
        }
    }
    if instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .any(|account| account.is_signer && account.pubkey != *taker)
    {
        return Err(Error::BadResponse(
            "route needs a signer besides the taker".into(),
        ));
    }
    Ok(Swap {
        in_amount: quote.in_amount,
        out_amount: response.min_amount,
        instructions,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::dex::Side,
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
        spl_token_2022_interface::inline_spl_token,
    };

    fn order() -> Order {
        Order {
            sell_mint: Pubkey::new_unique(),
            buy_mint: Pubkey::new_unique(),
            buy_token_program: inline_spl_token::ID,
            buy_decimals: 6,
            buy_destination: Pubkey::new_unique(),
            sell_amount: 1_000,
            buy_amount: 1,
            side: Side::Sell,
        }
    }

    fn response(
        order: &Order,
        taker: &Pubkey,
        token_program: &Pubkey,
        extra_signer: Option<Pubkey>,
    ) -> SwapResponse {
        let output =
            get_associated_token_address_with_program_id(taker, &order.buy_mint, token_program);
        let mut accounts = vec![
            serde_json::json!({"pubkey": taker.to_string(), "isSigner": true, "isWritable": true}),
            serde_json::json!({"pubkey": output.to_string(), "isSigner": false, "isWritable": true}),
        ];
        if let Some(signer) = extra_signer {
            accounts.push(serde_json::json!({"pubkey": signer.to_string(), "isSigner": true, "isWritable": false}));
        }
        serde_json::from_value(serde_json::json!({
            "minAmount": "990",
            "instructionLists": [
                {"programId": solana_compute_budget_interface::ID.to_string(), "accounts": [], "data": "AnBkCAA="},
                {"programId": Pubkey::new_unique().to_string(), "accounts": accounts, "data": "AQ=="},
            ],
        }))
        .unwrap()
    }

    #[test]
    fn keeps_the_route_and_drops_the_compute_budget() {
        let taker = Pubkey::new_unique();
        let order = order();
        let quote = response(&order, &taker, &inline_spl_token::ID, None).into_quote(&order);
        let swap = swap(&quote, &taker).unwrap();

        assert_eq!((quote.in_amount, quote.out_amount), (1_000, 990));
        assert_eq!((swap.in_amount, swap.out_amount), (1_000, 990));
        assert_eq!(swap.instructions.len(), 1);
        assert_ne!(
            swap.instructions[0].program_id,
            solana_compute_budget_interface::ID
        );
    }

    #[test]
    fn rejects_a_route_needing_another_signer() {
        let taker = Pubkey::new_unique();
        let order = order();
        let quote = response(
            &order,
            &taker,
            &inline_spl_token::ID,
            Some(Pubkey::new_unique()),
        )
        .into_quote(&order);
        assert!(matches!(swap(&quote, &taker), Err(Error::BadResponse(_))));
    }
}
