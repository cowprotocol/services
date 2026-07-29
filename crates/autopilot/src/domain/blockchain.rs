use {eth_domain_types as eth, serde::Deserialize};

/// Originated from the blockchain transaction input data.
pub type Calldata = alloy::primitives::Bytes;

// Note: normally we would first deserialize into a DTO and then
// convert that DTO into the domain type to decouple the wire
// format from the domain representation. However, in this case
// we are dealing with an object that's potentially very deeply
// nested. To not run into stack overflows we therefore directly
// deserialize into the domain type.
/// Call frames of a transaction.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CallFrame {
    /// The address of the call initiator.
    pub from: eth::Address,
    /// The address of the contract that was called.
    pub to: Option<eth::Address>,
    /// Calldata input.
    pub input: Calldata,
    /// Recorded child calls.
    #[serde(default)]
    pub calls: Vec<CallFrame>,
}

// custom drop implementation to avoid stack overflows on very
// deeply nested [`CallFrame`]s
impl Drop for CallFrame {
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.calls);
        while let Some(mut frame) = stack.pop() {
            stack.append(&mut frame.calls);
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
