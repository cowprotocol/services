use crate::{domain::eth, util};

/// An on-chain transaction that settles a settlement.
#[derive(Debug)]
pub struct Transaction {
    pub hash: eth::TxId,
    pub from: eth::Address,
    pub input: CallData,
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
