use ethcontract::{jsonrpc::Call as RpcCall, web3::RequestId};
use serde_json::Value;
use web3::{api::Web3, BatchTransport, Transport};

// To create an ethcontract instance we need to provide a web3 even though we never use it. This
// module provides a dummy transport and web3.
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

pub fn web3() -> Web3<DummyTransport> {
    Web3::new(DummyTransport)
}
