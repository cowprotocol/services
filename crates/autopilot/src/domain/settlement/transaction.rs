//! Domain types for on-chain transactions that settle settlements.

use crate::{domain::eth, util};

/// An on-chain transaction that settles a settlement.
#[derive(Debug)]
pub struct Transaction {
    hash: eth::TxId,
    input: CallData,
}

impl Transaction {
    /// The hash of the transaction.
    pub fn hash(&self) -> eth::TxId {
        self.hash
    }

    /// The call data of the transaction.
    pub fn input(&self) -> &CallData {
        &self.input
    }
}

/// Call data in a format expected by the settlement contract.
#[derive(Debug)]
pub struct CallData(pub util::Bytes<Vec<u8>>);

impl From<web3::types::Transaction> for Transaction {
    fn from(value: web3::types::Transaction) -> Self {
        Self {
            hash: value.hash.into(),
            input: CallData(value.input.0.into()),
        }
    }
}

/// A receipt of a transaction that settles a settlement.
#[derive(Debug)]
pub struct Receipt {
    hash: eth::TxId,
    block: eth::BlockNo,
    gas: eth::U256,
    effective_gas_price: eth::U256,
}

impl Receipt {
    /// The hash of the transaction.
    pub fn hash(&self) -> eth::TxId {
        self.hash
    }

    /// The block number of the block that contains the transaction.
    pub fn block(&self) -> eth::BlockNo {
        self.block
    }

    /// The gas used by the transaction.
    pub fn gas(&self) -> eth::U256 {
        self.gas
    }

    /// The effective gas price of the transaction.
    pub fn effective_gas_price(&self) -> eth::U256 {
        self.effective_gas_price
    }
}

impl From<web3::types::TransactionReceipt> for Receipt {
    fn from(value: web3::types::TransactionReceipt) -> Self {
        Self {
            hash: value.transaction_hash.into(),
            block: value.block_number.unwrap().0[0].into(),
            gas: value.gas_used.unwrap(),
            effective_gas_price: value.effective_gas_price.unwrap(),
        }
    }
}
