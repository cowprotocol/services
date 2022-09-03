//! Module containing Ethereum RPC extension methods.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web3::{
    self,
    api::Namespace,
    helpers::{self, CallFuture},
    types::{BlockId, Bytes, CallRequest, H160, H256, U256, U64},
    Transport,
};

/// Ethereum RPC extension methods that are not part of the JSON RPC standard
/// but commonly implemented by nodes.
#[derive(Debug, Clone)]
pub struct EthExt<T> {
    transport: T,
}

impl<T: Transport> EthExt<T> {
    pub async fn call(
        &self,
        call: CallRequest,
        block: BlockId,
        overrides: HashMap<H160, StateOverride>,
    ) -> Result<Bytes, web3::Error> {
        let call = helpers::serialize(&call);
        let block = helpers::serialize(&block);
        let overrides = helpers::serialize(&overrides);

        CallFuture::new(
            self.transport
                .execute("eth_call", vec![call, block, overrides]),
        )
        .await
    }
}

impl<T: Transport> Namespace<T> for EthExt<T> {
    fn new(transport: T) -> Self
    where
        Self: Sized,
    {
        EthExt { transport }
    }

    fn transport(&self) -> &T {
        &self.transport
    }
}

/// State override object.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
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
    pub state: Option<HashMap<H256, U256>>,

    /// Fake key-value mapping to override **individual** slots in the account
    /// storage before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_diff: Option<HashMap<H256, U256>>,
}

/// Web3 convenience extension trait.
pub trait Web3EthExt<T> {
    fn eth_ext(&self) -> EthExt<T>;
}

impl<T> Web3EthExt<T> for web3::Web3<T>
where
    T: Transport,
{
    fn eth_ext(&self) -> EthExt<T> {
        EthExt {
            transport: self.transport().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{transport::create_env_test_transport, Web3};
    use hex_literal::hex;
    use maplit::hashmap;
    use web3::types::BlockNumber;

    #[ignore]
    #[tokio::test]
    async fn can_call_with_state_override() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);

        let address = addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");
        let output = web3
            .eth_ext()
            .call(
                CallRequest {
                    to: Some(address),
                    ..Default::default()
                },
                BlockNumber::Latest.into(),
                hashmap! {
                    address => StateOverride {
                        // EVM program to just return 32 bytes from 0 to 31
                        code: Some(Bytes(
                            hex!(
                                "7f 000102030405060708090a0b0c0d0e0f
                                    101112131415161718191a1b1c1d1e1f
                                 60 00
                                 52
                                 60 20
                                 60 00
                                 f3"
                            )
                            .to_vec(),
                        )),
                        ..Default::default()
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(output.0, (0..32).collect::<Vec<_>>());
    }
}
