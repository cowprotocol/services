use {
    super::{eth, AuctionId},
    crate::infra,
    anyhow::Result,
};

pub mod encoded;
pub mod event;
pub mod observation;
pub mod transaction;

pub use {encoded::Encoded, event::Event, transaction::Transaction};

/// A transaction that settles a settlement. Interacts with the settlement
/// contract `settle` function.
pub struct Settlement {
    encoded: Encoded,
    transaction: transaction::Transaction,
    receipt: transaction::Receipt,
}

impl Settlement {
    pub async fn new(tx: eth::TxId, eth: infra::Ethereum) -> Result<Self, Error> {
        let transaction = eth
            .transaction(tx)
            .await?
            .ok_or(Error::TransactionNotFound)?;
        let receipt = eth
            .transaction_receipt(tx)
            .await?
            .ok_or(Error::TransactionNotFound)?;
        let domain_separator = eth.contracts().settlement_domain_separator();
        let encoded = Encoded::new(transaction.input(), domain_separator)?;
        Ok(Self {
            encoded,
            transaction,
            receipt,
        })
    }

    pub fn auction_id(&self) -> AuctionId {
        self.encoded.auction_id()
    }

    pub fn transaction(&self) -> &transaction::Transaction {
        &self.transaction
    }

    pub fn transaction_receipt(&self) -> &transaction::Receipt {
        &self.receipt
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blockchain(#[from] infra::blockchain::Error),
    #[error("invalid transaction hash, transaction not found")]
    TransactionNotFound,
    #[error(transparent)]
    Encoded(#[from] encoded::Error),
}
