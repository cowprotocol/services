//! Module containing Ethereum RPC extension methods.

use {
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    web3::{
        self,
        api::Namespace,
        helpers::{self, CallFuture},
        types::{BlockId, Bytes, CallRequest, H160, H256, U256, U64},
        Transport,
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

pub trait TracesExt<T>
where
    T: Transport,
{
    fn debug_transaction(&self, hash: H256) -> CallFuture<CallFrame, T::Out>;
}

impl<T> TracesExt<T> for web3::api::Traces<T>
where
    T: Transport,
{
    fn debug_transaction(&self, hash: H256) -> CallFuture<CallFrame, T::Out> {
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
    /// The address of the contract that was called.
    #[serde(default)]
    pub to: Option<primitive_types::H160>,
    /// Calldata input.
    pub input: Bytes,
    /// Recorded child calls.
    #[serde(default)]
    pub calls: Vec<CallFrame>,
}

/// State overrides.
pub type StateOverrides = HashMap<H160, StateOverride>;

/// State override object.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateOverride {
    /// Fake balance to set for the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<U256>,

    /// Fake nonce to set for the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U64>,

    /// Fake EVM bytecode to inject into the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,

    /// Fake key-value mapping to override **all** slots in the account storage
    /// before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<HashMap<H256, H256>>,

    /// Fake key-value mapping to override **individual** slots in the account
    /// storage before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_diff: Option<HashMap<H256, H256>>,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{create_env_test_transport, Web3},
        hex_literal::hex,
        maplit::hashmap,
        web3::types::BlockNumber,
    };

    #[ignore]
    #[tokio::test]
    async fn can_call_with_state_override() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);

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
