use crate::{domain::eth, util};

/// An on-chain transaction that settles a settlement.
#[derive(Debug)]
pub struct Transaction {
    hash: eth::TxId,
    from: eth::Address,
    input: CallData,
}

impl Transaction {
    /// The hash of the transaction.
    pub fn hash(&self) -> eth::TxId {
        self.hash
    }

    /// The address of the sender.
    pub fn from(&self) -> eth::Address {
        self.from
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
            from: value.from.unwrap().into(),
            input: CallData(value.input.0.into()),
        }
    }
}
