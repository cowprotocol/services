use {
    crate::domain::{self, eth},
    anyhow::anyhow,
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
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The gas used by the transaction.
    pub gas: eth::Gas,
    /// The effective gas price of the transaction.
    pub effective_gas_price: eth::EffectiveGasPrice,
    /// The solution that was settled.
    pub solution: domain::settlement::Solution,
}

impl Transaction {
    pub fn new(
        transaction: &eth::Transaction,
        domain_separator: &eth::DomainSeparator,
    ) -> anyhow::Result<Self> {
        let (_, metadata) = transaction
            .input
            .0
            .split_at(transaction.input.0.len() - META_DATA_LEN);
        let metadata: Option<[u8; META_DATA_LEN]> = metadata.try_into().ok();
        let auction_id = metadata
            .map(crate::domain::auction::Id::from_be_bytes)
            .ok_or(anyhow!("missing auction id"))?;
        Ok(Self {
            hash: transaction.hash,
            auction_id,
            solver: transaction.solver,
            block: transaction.block,
            gas: transaction.gas,
            effective_gas_price: transaction.effective_gas_price,
            solution: domain::settlement::Solution::new(&transaction.input, domain_separator)
                .map_err(|err| anyhow!("solution build {}", err))?,
        })
    }
}

/// Number of bytes that may be appended to the calldata to store an auction
/// id.
const META_DATA_LEN: usize = 8;
