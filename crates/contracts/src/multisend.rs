//! Module for encoding transactions for the Safe periphery `MultiSend` and
//! `MultiSendCallOnly` contracts.
//!
//! Details about the encoding can be found:
//! - <https://github.com/safe-global/safe-contracts/blob/v1.3.0/contracts/libraries/MultiSend.sol#L17-L23>
//! - <https://github.com/safe-global/safe-contracts/blob/v1.3.0/contracts/libraries/MultiSendCallOnly.sol#L10-L16>

use ethcontract::{Bytes, H160, U256};

#[derive(Clone, Debug, Default)]
pub struct Transaction {
    pub op: Operation,
    pub to: H160,
    pub value: U256,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(u8)]
pub enum Operation {
    #[default]
    Call = 0,
    DelegateCall = 1,
}

pub fn encode(transactions: &[Transaction]) -> Bytes<Vec<u8>> {
    let len = transactions.iter().map(Transaction::encoded_len).sum();
    let mut buffer = Vec::with_capacity(len);

    for transaction in transactions {
        buffer.push(transaction.op as _);
        buffer.extend_from_slice(&transaction.to.0);
        buffer.extend_from_slice(&{
            let mut word = [0; 32];
            transaction.value.to_big_endian(&mut word);
            word
        });
        buffer.extend_from_slice(&{
            let mut word = [0; 32];
            U256::from(transaction.data.len()).to_big_endian(&mut word);
            word
        });
        buffer.extend_from_slice(&transaction.data);
    }

    Bytes(buffer)
}

impl Transaction {
    fn encoded_len(&self) -> usize {
        1 /* op */ + 20 /* to */ + 32 /* value */ + 32 /* data.len() */ + self.data.len()
    }
}
