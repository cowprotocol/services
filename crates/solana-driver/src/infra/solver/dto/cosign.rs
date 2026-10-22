//! The `/cosign` request and response: a transaction as base64 of its wire
//! bytes.

use {
    base64::prelude::*,
    serde::{Deserialize, Serialize},
    solana_sdk::transaction::VersionedTransaction,
};

#[derive(Serialize, Deserialize)]
pub struct Transaction {
    transaction: String,
}

impl Transaction {
    pub fn new(transaction: &VersionedTransaction) -> Self {
        let bytes = wincode::serialize(transaction).expect("a transaction serializes");
        Self {
            transaction: BASE64_STANDARD.encode(bytes),
        }
    }

    pub fn into_transaction(self) -> Result<VersionedTransaction, String> {
        let bytes = BASE64_STANDARD
            .decode(&self.transaction)
            .map_err(|err| format!("transaction base64: {err}"))?;
        wincode::deserialize(&bytes).map_err(|err| format!("transaction bytes: {err}"))
    }
}
