use contracts::WETH9;
use jsonrpc_core::Call as RpcCall;
use serde_json::Value;
use web3::{api::Web3, types::H160, Transport};

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

pub fn dummy_web3() -> Web3<DummyTransport> {
    Web3::new(DummyTransport)
}

pub fn dummy_weth(addr: impl Into<H160>) -> WETH9 {
    WETH9::at(&dummy_web3(), addr.into())
}
