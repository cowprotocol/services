//! DTOs for Jupiter's `/swap-instructions` request and response, converting to
//! and from Solana instructions. Field names follow Jupiter's camelCase JSON.

use {
    super::Error,
    crate::dex::{Swap, dto::ApiInstruction},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    solana_sdk::pubkey::Pubkey,
};

/// Body of the `/swap-instructions` request.
#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionsRequest<'a> {
    quote_response: &'a serde_json::Value,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    user_public_key: Pubkey,
    wrap_and_unwrap_sol: bool,
    skip_user_accounts_rpc_calls: bool,
    use_shared_accounts: bool,
}

impl<'a> SwapInstructionsRequest<'a> {
    /// The swap rides inside the settlement tx, so SOL wrapping is left off and
    /// Jupiter skips its account RPC probes. With no destination named, the
    /// swap pays the taker's own ATA of the output mint.
    pub fn new(quote_response: &'a serde_json::Value, taker: &Pubkey) -> Self {
        Self {
            quote_response,
            user_public_key: *taker,
            wrap_and_unwrap_sol: false,
            skip_user_accounts_rpc_calls: true,
            // Direct Route mode: every token account in the route is the
            // taker's own.
            use_shared_accounts: false,
        }
    }
}

/// The parts of the `/swap-instructions` response we need to build a [`Swap`].
/// Amounts come from the `/quote` response.
///
/// Jupiter's `/swap-instructions` response can contain
/// `computeBudgetInstructions`, but we don't pass them along: the driver sets
/// the settlement's compute budget in its v1 message config, where these
/// instructions only cost an address and compute units as no-ops.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionsResponse {
    #[serde(default)]
    setup_instructions: Vec<ApiInstruction>,
    swap_instruction: ApiInstruction,
    #[serde(default)]
    cleanup_instruction: Option<ApiInstruction>,
}

impl SwapInstructionsResponse {
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
