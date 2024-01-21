use {super::eth, crate::infra, anyhow::Result};

pub mod encoded;
pub mod event;
pub mod transaction;
pub use {encoded::Encoded, event::Event, transaction::Transaction};

/// A transaction that settles a settlement. Interacts with the settlement
/// contract `settle` function.
pub struct Settlement {
    pub encoded: Encoded,
    pub transaction: Transaction,
}

impl Settlement {
    pub async fn new(tx: eth::TxId, eth: infra::Ethereum) -> Result<Self> {
        let transaction = eth.transaction(tx).await?.unwrap();
        let domain_separator = eth.contracts().settlement_domain_separator();
        let encoded = Encoded::new(&transaction.input, domain_separator)?;
        Ok(Self {
            encoded,
            transaction,
        })
    }
}
