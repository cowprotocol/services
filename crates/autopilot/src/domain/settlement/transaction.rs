//! Domain types for on-chain transactions that settle settlements.

use crate::{domain::eth, util};

/// An on-chain transaction that settles a settlement.
#[derive(Debug)]
pub struct Transaction {
    hash: eth::TxId,
    solver: eth::Address,
    input: CallData,
}

impl Transaction {
    /// The hash of the transaction.
    pub fn hash(&self) -> eth::TxId {
        self.hash
    }

    /// The address of the solver that submitted the transaction.
    pub fn solver(&self) -> eth::Address {
        self.solver
    }

    /// The call data of the transaction.
    pub fn input(&self) -> &CallData {
        &self.input
    }
}

/// Call data in a format expected by the settlement contract.
#[derive(Debug)]
pub struct CallData(pub util::Bytes<Vec<u8>>);

impl TryFrom<web3::types::Transaction> for Transaction {
    type Error = &'static str;

    fn try_from(value: web3::types::Transaction) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: value.hash.into(),
            solver: value.from.ok_or("from")?.into(),
            input: CallData(value.input.0.into()),
        })
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

impl TryFrom<web3::types::TransactionReceipt> for Receipt {
    type Error = &'static str;

    fn try_from(value: web3::types::TransactionReceipt) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: value.transaction_hash.into(),
            block: value.block_number.ok_or("block_number")?.0[0].into(),
            gas: value.gas_used.ok_or("gas_used")?,
            effective_gas_price: value.effective_gas_price.ok_or("effective_gas_price")?,
        })
    }
}
