//! Instruction JSON shared by the aggregator APIs: `programId`, `accounts` and
//! base64 `data`.

use {
    super::Error,
    base64::prelude::*,
    serde::Deserialize,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    },
    std::str::FromStr,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiInstruction {
    program_id: String,
    accounts: Vec<ApiAccount>,
    data: String,
}

impl ApiInstruction {
    pub fn into_instruction(self) -> Result<Instruction, Error> {
        let program_id = Pubkey::from_str(&self.program_id)
            .map_err(|err| Error::BadResponse(format!("program id: {err}")))?;
        let accounts = self
            .accounts
            .into_iter()
            .map(ApiAccount::into_meta)
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
struct ApiAccount {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

impl ApiAccount {
    fn into_meta(self) -> Result<AccountMeta, Error> {
        Ok(AccountMeta {
            pubkey: Pubkey::from_str(&self.pubkey)
                .map_err(|err| Error::BadResponse(format!("account pubkey: {err}")))?,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        })
    }
}
