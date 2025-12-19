//! Module containing Ethereum RPC extension methods.

use {
    alloy::primitives::Address,
    serde::Deserialize,
    web3::{
        self,
        Transport,
        api::Namespace,
        helpers::{self, CallFuture},
        types::H256,
    },
};

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
    pub from: Address,
    /// The address of the contract that was called.
    #[serde(default)]
    pub to: Option<Address>,
    /// Calldata input.
    pub input: alloy::primitives::Bytes,
    /// Recorded child calls.
    #[serde(default)]
    pub calls: Vec<CallFrame>,
}
