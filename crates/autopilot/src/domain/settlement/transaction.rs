//! Domain types for on-chain transactions that settle settlements.

use {
    super::{coded, Settlement},
    crate::{
        domain::{auction, eth},
        infra,
        util,
    },
    std::collections::HashMap,
};

/// A transaction that settles a settlement. Interacts with the settlement
/// contract `settle` function.
pub struct SettlementTx {
    settlement: Settlement,
    transaction: Transaction,
    receipt: Receipt,
}

impl SettlementTx {
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
        let settlement = Settlement::new(&transaction.input, domain_separator)?;
        Ok(Self {
            settlement,
            transaction,
            receipt,
        })
    }

    pub fn auction_id(&self) -> auction::Id {
        self.settlement.auction_id()
    }

    pub fn solver(&self) -> eth::Address {
        self.transaction.solver
    }

    pub fn observation(
        &self,
        prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    ) -> super::Observation {
        super::Observation {
            gas: self.receipt.gas,
            effective_gas_price: self.receipt.effective_gas_price,
            surplus: super::Surplus::new(self.settlement.trades())
                .normalized_with(prices)
                .unwrap_or_default(),
            fee: super::Fees::new(self.settlement.trades())
                .normalized_with(prices)
                .unwrap_or_default(),
            order_fees: super::Fees::new(self.settlement.trades()),
        }
    }
}

/// An on-chain transaction that settles a settlement.
#[derive(Debug)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The address of the solver that submitted the transaction.
    pub solver: eth::Address,
    /// The call data of the transaction.
    pub input: CallData,
}

/// Call data in a format expected by the settlement contract.
#[derive(Debug)]
pub struct CallData(pub util::Bytes<Vec<u8>>);

/// A receipt of a transaction that settles a settlement.
#[derive(Debug)]
pub struct Receipt {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The gas used by the transaction.
    pub gas: eth::U256,
    /// The effective gas price of the transaction.
    pub effective_gas_price: eth::U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blockchain(#[from] infra::blockchain::Error),
    #[error("invalid transaction hash, transaction not found")]
    TransactionNotFound,
    #[error(transparent)]
    Encoded(#[from] coded::Error),
}
