//! DTOs for Jupiter's `/swap-instructions` request and response, converting to
//! and from Solana instructions. Field names follow Jupiter's camelCase JSON.

use {
    super::Error,
    crate::dex::Swap,
    base64::prelude::*,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    },
    std::str::FromStr,
};

/// Body of the `/swap-instructions` request.
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
        }
    }
}

/// The parts of the `/swap-instructions` response we need to build a [`Swap`].
/// Amounts come from the `/quote` response.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionsResponse {
    #[serde(default)]
    setup_instructions: Vec<JupInstruction>,
    swap_instruction: JupInstruction,
    #[serde(default)]
    cleanup_instruction: Option<JupInstruction>,
    #[serde(default)]
    address_lookup_table_addresses: Vec<String>,
}

impl SwapInstructionsResponse {
    /// Flatten into execution order (setup, swap, cleanup) and resolve the
    /// lookup-table addresses.
    pub fn into_swap(self, in_amount: u64, out_amount: u64) -> Result<Swap, Error> {
        let mut instructions = Vec::with_capacity(self.setup_instructions.len() + 2);
        for instruction in self.setup_instructions {
            instructions.push(instruction.into_instruction()?);
        }
        instructions.push(self.swap_instruction.into_instruction()?);
        if let Some(instruction) = self.cleanup_instruction {
            instructions.push(instruction.into_instruction()?);
        }
        let address_lookup_tables = self
            .address_lookup_table_addresses
            .iter()
            .map(|address| {
                Pubkey::from_str(address)
                    .map_err(|err| Error::BadResponse(format!("lookup table address: {err}")))
            })
            .collect::<Result<_, _>>()?;
        Ok(Swap {
            in_amount,
            out_amount,
            instructions,
            address_lookup_tables,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JupInstruction {
    program_id: String,
    accounts: Vec<JupAccount>,
    data: String,
}

impl JupInstruction {
    fn into_instruction(self) -> Result<Instruction, Error> {
        let program_id = Pubkey::from_str(&self.program_id)
            .map_err(|err| Error::BadResponse(format!("program id: {err}")))?;
        let accounts = self
            .accounts
            .into_iter()
            .map(JupAccount::into_meta)
            .collect::<Result<_, _>>()?;
        let data = BASE64_STANDARD
            .decode(&self.data)
            .map_err(|err| Error::BadResponse(format!("instruction data: {err}")))?;
        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JupAccount {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

impl JupAccount {
    fn into_meta(self) -> Result<AccountMeta, Error> {
        Ok(AccountMeta {
            pubkey: Pubkey::from_str(&self.pubkey)
                .map_err(|err| Error::BadResponse(format!("account pubkey: {err}")))?,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        })
    }
}
