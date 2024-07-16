use crate::domain::eth;

/// An on-chain transaction that settled a solution.
#[derive(Debug)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The address of the solver that submitted the transaction.
    pub solver: eth::Address,
    /// The call data of the transaction.
    pub input: eth::Calldata,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The gas used by the transaction.
    pub gas: eth::TokenAmount,
    /// The effective gas price of the transaction.
    pub effective_gas_price: eth::TokenAmount,
}
