//! DTOs for Jupiter's swap API, converting to and from Solana instructions.
//! Field names follow Jupiter's camelCase JSON.
//!
//! v1: the `/quote` response and the `/swap-instructions` request and
//! response. v2: the `/build` response, which carries both the amounts and
//! the instructions.

use {
    super::{Error, SwapMode},
    crate::dex::{Swap, dto::ApiInstruction},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    solana_sdk::pubkey::Pubkey,
};

/// The parts of a v1 `/quote` response we read; the rest rides back to
/// `/swap-instructions` untouched.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteAmounts {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub in_amount: u64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub out_amount: u64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub other_amount_threshold: u64,
    pub swap_mode: SwapMode,
}

/// The amounts of a v2 `/build` response. The rest stays in the quote until
/// its swap is built.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildAmounts {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub in_amount: u64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub other_amount_threshold: u64,
}

/// Body of the v1 `/swap-instructions` request.
#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionsRequest<'a> {
    quote_response: &'a serde_json::Value,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    user_public_key: Pubkey,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    destination_token_account: Pubkey,
    wrap_and_unwrap_sol: bool,
    skip_user_accounts_rpc_calls: bool,
    use_shared_accounts: bool,
}

impl<'a> SwapInstructionsRequest<'a> {
    /// The swap rides inside the settlement tx, so SOL wrapping is left off and
    /// Jupiter skips its account RPC probes.
    pub fn new(
        quote_response: &'a serde_json::Value,
        taker: &Pubkey,
        destination: &Pubkey,
    ) -> Self {
        Self {
            quote_response,
            user_public_key: *taker,
            destination_token_account: *destination,
            wrap_and_unwrap_sol: false,
            skip_user_accounts_rpc_calls: true,
            // Use direct Route mode while we investigate errors with
            // SharedAccountsRoute. Its instructions ignored the buy-token
            // destination, leaving the buy-buffer PDAs empty and
            // causing settlement transactions to revert.
            use_shared_accounts: false,
        }
    }
}

/// The instructions that make up a [`Swap`]: the v1 `/swap-instructions`
/// response, or the instruction part of a v2 `/build` response. Amounts come
/// from the quote.
///
/// Both responses can carry `computeBudgetInstructions`, but we don't pass
/// them along: the driver sets the settlement's compute budget in its v1
/// message config, where these instructions only cost an address and compute
/// units as no-ops.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructions {
    #[serde(default)]
    setup_instructions: Vec<ApiInstruction>,
    swap_instruction: ApiInstruction,
    #[serde(default)]
    cleanup_instruction: Option<ApiInstruction>,
}

impl SwapInstructions {
    /// Flatten into execution order (setup, swap, cleanup).
    pub fn into_swap(self, in_amount: u64, out_amount: u64) -> Result<Swap, Error> {
        let mut instructions = Vec::with_capacity(self.setup_instructions.len() + 2);
        for instruction in self.setup_instructions {
            instructions.push(instruction.into_instruction()?);
        }
        instructions.push(self.swap_instruction.into_instruction()?);
        if let Some(instruction) = self.cleanup_instruction {
            instructions.push(instruction.into_instruction()?);
        }
        Ok(Swap {
            in_amount,
            out_amount,
            instructions,
        })
    }
}
