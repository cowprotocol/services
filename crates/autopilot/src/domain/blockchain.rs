use eth_domain_types as eth;

/// Originated from the blockchain transaction input data.
pub type Calldata = alloy::primitives::Bytes;

/// Call frames of a transaction.
#[derive(Clone, Debug, Default)]
pub struct CallFrame {
    /// The address of the call initiator.
    pub from: eth::Address,
    /// The address of the contract that was called.
    pub to: Option<eth::Address>,
    /// Calldata input.
    pub input: Calldata,
    /// Recorded child calls.
    pub calls: Vec<CallFrame>,
}

impl From<alloy::rpc::types::trace::geth::CallFrame> for CallFrame {
    fn from(value: alloy::rpc::types::trace::geth::CallFrame) -> Self {
        Self {
            from: value.from,
            to: value.to,
            input: value.input,
            calls: value.calls.into_iter().map(Into::into).collect(),
        }
    }
}

/// Any type of on-chain transaction.
#[derive(Debug, Clone, Default)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The address of the sender of the transaction.
    pub from: eth::Address,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The timestamp of the block that contains the transaction.
    pub timestamp: u32,
    /// The gas used by the transaction.
    pub gas: eth::Gas,
    /// The effective gas price of the transaction.
    pub gas_price: eth::EffectiveGasPrice,
    /// Traces of all Calls contained in the transaction.
    pub trace_calls: CallFrame,
}
