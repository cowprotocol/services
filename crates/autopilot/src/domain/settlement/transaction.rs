use {
    crate::domain::{self, eth},
    ethcontract::common::FunctionExt,
};

/// An on-chain transaction that settled a solution, with calldata in a valid
/// format.
#[derive(Debug, Clone)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The associated auction id.
    pub auction_id: domain::auction::Id,
    /// The address of the solver that submitted the transaction.
    pub solver: eth::Address,
    /// The call data of the transaction.
    pub input: eth::Calldata,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The gas used by the transaction.
    pub gas: eth::Gas,
    /// The effective gas price of the transaction.
    pub effective_gas_price: eth::EffectiveGasPrice,
}

/// Number of bytes that may be appended to the calldata to store an auction
/// id.
const META_DATA_LEN: usize = 8;

impl TryFrom<eth::Transaction> for Transaction {
    type Error = Error;

    fn try_from(transaction: eth::Transaction) -> Result<Self, Self::Error> {
        let function = contracts::GPv2Settlement::raw_contract()
            .interface
            .abi
            .function("settle")
            .unwrap();
        let data = transaction
            .input
            .0
            .strip_prefix(&function.selector())
            .ok_or(Error::InvalidSelector)?;

        let (calldata, metadata) = data.split_at(data.len() - META_DATA_LEN);
        let metadata: Option<[u8; META_DATA_LEN]> = metadata.try_into().ok();
        let auction_id = metadata
            .map(crate::domain::auction::Id::from_be_bytes)
            .ok_or(Error::MissingAuctionId)?;

        Ok(Self {
            hash: transaction.hash,
            auction_id,
            solver: transaction.solver,
            input: crate::util::Bytes(calldata.to_vec()),
            block: transaction.block,
            gas: transaction.gas,
            effective_gas_price: transaction.effective_gas_price,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transaction calldata is not a settlement")]
    InvalidSelector,
    #[error("no auction id found in calldata")]
    MissingAuctionId,
}
