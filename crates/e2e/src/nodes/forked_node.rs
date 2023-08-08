use {
    super::TestNode,
    ethcontract::H160,
    serde_json::json,
    std::fmt::Debug,
    web3::{api::Namespace, helpers::CallFuture, Transport},
};

pub struct Forker<T> {
    forked_node_api: ForkedNodeApi<T>,
    fork_url: String,
}

impl<T: Transport> Forker<T> {
    pub async fn new(web3: &web3::Web3<T>, solver_address: H160, fork_url: String) -> Self {
        let forked_node_api = web3.api::<ForkedNodeApi<_>>();
        forked_node_api
            .fork(&fork_url)
            .await
            .expect("Test network must support hardhat_reset");

        forked_node_api
            .impersonate(&solver_address)
            .await
            .expect("Test network must support hardhat_impersonateAccount");

        Self {
            forked_node_api,
            fork_url,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<T: Transport> TestNode for Forker<T> {
    async fn reset(&self) {
        self.forked_node_api
            .fork(&self.fork_url)
            .await
            .expect("Test network must support hardhat_reset");
    }
}

#[derive(Debug, Clone)]
pub struct ForkedNodeApi<T> {
    transport: T,
}

impl<T: Transport> Namespace<T> for ForkedNodeApi<T> {
    fn new(transport: T) -> Self
    where
        Self: Sized,
    {
        ForkedNodeApi { transport }
    }

    fn transport(&self) -> &T {
        &self.transport
    }
}

/// Implements functions that are only available in a forked node.
///
/// Relevant RPC calls for the Hardhat forked network can be found at:
/// https://hardhat.org/hardhat-network/docs/reference#hardhat-network-methods
impl<T: Transport> ForkedNodeApi<T> {
    pub fn fork(&self, fork_url: &String) -> CallFuture<bool, T::Out> {
        CallFuture::new(self.transport.execute(
            "hardhat_reset",
            vec![json!({ "forking": {"jsonRpcUrl": fork_url} })],
        ))
    }

    pub fn impersonate(&self, address: &H160) -> CallFuture<bool, T::Out> {
        let json_address = serde_json::json!(address);
        CallFuture::new(
            self.transport
                .execute("hardhat_impersonateAccount", vec![json_address]),
        )
    }
}
