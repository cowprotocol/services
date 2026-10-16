//! The `/cosign` request and response: a transaction as base64 bincode.

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
        let bytes = bincode::serialize(transaction).expect("a transaction serializes");
        Self {
            transaction: BASE64_STANDARD.encode(bytes),
        }
    }

    pub fn into_transaction(self) -> Result<VersionedTransaction, String> {
        let bytes = BASE64_STANDARD
            .decode(&self.transaction)
            .map_err(|err| format!("transaction base64: {err}"))?;
        bincode::deserialize(&bytes).map_err(|err| format!("transaction bincode: {err}"))
    }
}
