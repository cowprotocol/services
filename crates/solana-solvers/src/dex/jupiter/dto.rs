//! DTOs for Jupiter's `/build` response, converting to Solana instructions.
//! Field names follow Jupiter's camelCase JSON.

use {
    super::Error,
    crate::dex::{Swap, dto::ApiInstruction},
    serde::Deserialize,
    serde_with::serde_as,
};

/// The amounts of a `/build` response. The rest stays in the quote until its
/// swap is built.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteAmounts {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub in_amount: u64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub other_amount_threshold: u64,
}

/// The instructions of a `/build` response that make up a [`Swap`].
///
/// The response also carries `computeBudgetInstructions`, but we don't pass
/// them along: the driver sets the settlement's compute budget in its v1
/// message config, and we request v1 transactions so the list comes back
/// empty anyway.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInstructions {
    #[serde(default)]
    setup_instructions: Vec<ApiInstruction>,
    swap_instruction: ApiInstruction,
    #[serde(default)]
    cleanup_instruction: Option<ApiInstruction>,
}

impl BuildInstructions {
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
