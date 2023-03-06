//! This module provides a "dummy" implementation of a [`web3::Transport`]. The
//! reason is that [`ethcontract`] generated bindings require a `web3` instance
//! in order to construct contract instances. This is annoying when trying to
//! use the generated bindings for just encoding contract function calls, where
//! connection to a node is not needed at all.

use ethcontract::{
    futures,
    json::Value,
    jsonrpc::Call as RpcCall,
    web3::{self, BatchTransport, RequestId, Transport, Web3},
};

/// A dummy [`web3::Transport`] implementation that always panics.
#[derive(Clone, Debug)]
pub struct DummyTransport;

impl Transport for DummyTransport {
    type Out = futures::future::Pending<web3::Result<Value>>;

    fn prepare(&self, _method: &str, _params: Vec<Value>) -> (web3::RequestId, RpcCall) {
        unimplemented!()
    }

    fn send(&self, _id: web3::RequestId, _request: RpcCall) -> Self::Out {
        unimplemented!()
    }
}

impl BatchTransport for DummyTransport {
    type Batch = futures::future::Pending<web3::Result<Vec<web3::Result<Value>>>>;

    fn send_batch<T>(&self, _requests: T) -> Self::Batch
    where
        T: IntoIterator<Item = (RequestId, RpcCall)>,
    {
        unimplemented!()
    }
}

/// Creates a [`web3::Web3`] instance with a [`DummyTransport`].
pub fn dummy() -> Web3<DummyTransport> {
    Web3::new(DummyTransport)
}
