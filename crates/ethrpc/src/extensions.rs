//! Module containing Ethereum RPC extension methods.

use {
    ethcontract::state_overrides::StateOverrides,
    serde::Deserialize,
    tracing::{Instrument, instrument::Instrumented},
    web3::{
        self,
        Transport,
        api::Namespace,
        helpers::{self, CallFuture},
        types::{BlockId, Bytes, CallRequest, H256},
    },
};

/// Web3 convenience extension trait.
pub trait EthExt<T>
where
    T: Transport,
{
    fn call_with_state_overrides(
        &self,
        call: CallRequest,
        block: BlockId,
        overrides: StateOverrides,
    ) -> Instrumented<CallFuture<Bytes, T::Out>>;
}

impl<T> EthExt<T> for web3::api::Eth<T>
where
    T: Transport,
{
    fn call_with_state_overrides(
        &self,
        call: CallRequest,
        block: BlockId,
        overrides: StateOverrides,
    ) -> Instrumented<CallFuture<Bytes, T::Out>> {
        let call = helpers::serialize(&call);
        let block = helpers::serialize(&block);
        let overrides = helpers::serialize(&overrides);

        CallFuture::new(
            self.transport()
                .execute("eth_call", vec![call, block, overrides]),
        )
        .instrument(tracing::info_span!("eth_call"))
    }
}

/// Debug namespace extension trait.
pub trait DebugNamespace<T>
where
    T: Transport,
{
    fn debug(&self) -> Debug<T>;
}

impl<T: Transport> DebugNamespace<T> for web3::Web3<T> {
    fn debug(&self) -> Debug<T> {
        self.api()
    }
}

/// `Debug` namespace
#[derive(Debug, Clone)]
pub struct Debug<T> {
    transport: T,
}

impl<T: Transport> Namespace<T> for Debug<T> {
    fn new(transport: T) -> Self
    where
        Self: Sized,
    {
        Debug { transport }
    }

    fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: Transport> Debug<T> {
    /// Returns all debug traces for callTracer type of tracer.
    pub fn transaction(&self, hash: H256) -> CallFuture<CallFrame, T::Out> {
        let hash = helpers::serialize(&hash);
        let tracing_options = serde_json::json!({ "tracer": "callTracer" });
        CallFuture::new(
            self.transport()
                .execute("debug_traceTransaction", vec![hash, tracing_options]),
        )
    }

    /// Traces a call with struct logs to capture opcode-level execution.
    /// This is useful for detecting which storage slots are accessed during a
    /// call.
    pub fn trace_call(
        &self,
        call: CallRequest,
        block: BlockId,
    ) -> CallFuture<StructLogTrace, T::Out> {
        let call = helpers::serialize(&call);
        let block = helpers::serialize(&block);
        let tracing_options = serde_json::json!({
            "enableMemory": false,
            "disableStack": false,
            "disableStorage": false,
            "enableReturnData": false
        });
        CallFuture::new(
            self.transport()
                .execute("debug_traceCall", vec![call, block, tracing_options]),
        )
    }
}

/// Taken from alloy::rpc::types::trace::geth::CallFrame
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct CallFrame {
    /// The address of that initiated the call.
    pub from: primitive_types::H160,
    /// The address of the contract that was called.
    #[serde(default)]
    pub to: Option<primitive_types::H160>,
    /// Calldata input.
    pub input: Bytes,
    /// Recorded child calls.
    #[serde(default)]
    pub calls: Vec<CallFrame>,
}

/// Struct log trace response from debug_traceCall
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructLogTrace {
    /// Gas used by the call
    #[serde(default)]
    pub gas: u64,
    /// Whether the call failed
    #[serde(default)]
    pub failed: bool,
    /// Return value
    #[serde(default)]
    pub return_value: String,
    /// Struct logs containing opcode-level execution trace
    #[serde(default)]
    pub struct_logs: Vec<StructLog>,
}

/// Individual struct log entry representing one opcode execution
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructLog {
    /// Program counter
    pub pc: u64,
    /// Opcode name
    pub op: String,
    /// Gas remaining
    pub gas: u64,
    /// Gas cost
    pub gas_cost: u64,
    /// Depth of call stack
    pub depth: u64,
    /// Stack values (top of stack is last element)
    /// Stack values can be variable length hex strings, so we deserialize as
    /// strings
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_hex_stack")]
    pub stack: Vec<primitive_types::H256>,
    /// Storage changes at this step
    #[serde(default)]
    pub storage: std::collections::HashMap<primitive_types::H256, primitive_types::H256>,
}

/// Custom deserializer for stack values that handles variable-length hex
/// strings (side note: I don't know why this has to be so complicated...)
fn deserialize_hex_stack<'de, D>(deserializer: D) -> Result<Vec<primitive_types::H256>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let strings: Vec<String> = Vec::deserialize(deserializer)?;

    strings
        .into_iter()
        .map(|s| {
            // Remove 0x prefix if present
            let mut hex_str = s.strip_prefix("0x").unwrap_or(&s).to_string();

            // Hex decoder requires even number of digits, prepend 0 if odd
            if hex_str.len() % 2 != 0 {
                hex_str.insert(0, '0');
            }

            // Decode hex to bytes
            let bytes = const_hex::decode(&hex_str)
                .map_err(|e| serde::de::Error::custom(format!("invalid hex: {}", e)))?;

            // Left-pad to 32 bytes
            let mut padded = [0u8; 32];
            if bytes.len() <= 32 {
                padded[32 - bytes.len()..].copy_from_slice(&bytes);
            } else {
                return Err(serde::de::Error::custom(format!(
                    "hex value too long: {} bytes",
                    bytes.len()
                )));
            }

            Ok(primitive_types::H256(padded))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::Web3,
        ethcontract::{H160, state_overrides::StateOverride},
        hex_literal::hex,
        maplit::hashmap,
        web3::types::BlockNumber,
    };

    #[ignore]
    #[tokio::test]
    async fn can_call_with_state_override() {
        let web3 = Web3::new_from_env();

        let address = H160(hex!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
        let output = web3
            .eth()
            .call_with_state_overrides(
                CallRequest {
                    to: Some(address),
                    ..Default::default()
                },
                BlockNumber::Latest.into(),
                hashmap! {
                    address => StateOverride {
                        // EVM program to just return 32 bytes from 0 to 31
                        code: Some(hex!(
                            "7f 000102030405060708090a0b0c0d0e0f
                                101112131415161718191a1b1c1d1e1f
                             60 00
                             52
                             60 20
                             60 00
                             f3"
                        ).into()),
                        ..Default::default()
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(output.0, (0..32).collect::<Vec<_>>());
    }
}
