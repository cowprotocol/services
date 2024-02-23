use {
    super::{auction, eth},
    crate::infra,
    anyhow::Result,
};

pub mod encoded;
pub mod fees;
pub mod observation;
pub mod surplus;
pub mod transaction;

pub use {
    encoded::Encoded,
    fees::Fees,
    surplus::{NormalizedSurplus, Surplus},
    transaction::Transaction,
};

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

    pub fn auction_id(&self) -> auction::Id {
        self.encoded.auction_id()
    }

    pub fn transaction(&self) -> &transaction::Transaction {
        &self.transaction
    }

    pub fn transaction_receipt(&self) -> &transaction::Receipt {
        &self.receipt
    }

    pub fn encoded(&self) -> &Encoded {
        &self.encoded
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
