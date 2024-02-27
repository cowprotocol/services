use {
    super::{auction, eth},
    crate::infra,
    anyhow::Result,
    std::collections::HashMap,
};

pub mod encoded;
pub mod fees;
pub mod observation;
pub mod surplus;
pub mod transaction;

pub use {
    encoded::Encoded,
    fees::{Fees, NormalizedFee},
    observation::Observation,
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

    pub fn solver(&self) -> eth::Address {
        self.transaction.solver()
    }

    pub fn observation(
        &self,
        prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    ) -> observation::Observation {
        observation::Observation {
            gas: self.receipt.gas(),
            effective_gas_price: self.receipt.effective_gas_price(),
            surplus: super::settlement::Surplus::new(self.encoded.trades())
                .normalized_with(prices)
                .unwrap_or_default(),
            fee: super::settlement::Fees::new(self.encoded.trades())
                .normalized_with(prices)
                .unwrap_or_default(),
            order_fees: super::settlement::Fees::new(self.encoded.trades()),
        }
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
