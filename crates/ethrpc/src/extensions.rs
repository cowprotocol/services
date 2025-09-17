//! Module containing Ethereum RPC extension methods.

use {
    ethcontract::state_overrides::{StateOverride, StateOverrides}, serde::Deserialize, std::collections::HashMap, web3::{
        self, api::Namespace, helpers::{self, CallFuture}, types::{BlockId, Bytes, CallRequest, H160, H256}, Transport
    }
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
        overrides: HashMap<H160, StateOverride>,
    ) -> CallFuture<Bytes, T::Out>;
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
    ) -> CallFuture<Bytes, T::Out> {
        let call = helpers::serialize(&call);
        let block = helpers::serialize(&block);
        let overrides = helpers::serialize(&overrides);

        CallFuture::new(
            self.transport()
                .execute("eth_call", vec![call, block, overrides]),
        )
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
