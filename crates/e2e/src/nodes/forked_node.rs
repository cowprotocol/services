use {
    super::TestNode,
    ethcontract::H160,
    reqwest::{IntoUrl, Url},
    serde_json::json,
    std::fmt::Debug,
    web3::{api::Namespace, helpers::CallFuture, Transport},
};

pub struct Forker<T> {
    forked_node_api: ForkedNodeApi<T>,
    fork_url: Url,
}

impl<T: Transport> Forker<T> {
    pub async fn new(web3: &web3::Web3<T>, solver_address: H160, fork_url: impl IntoUrl) -> Self {
        let fork_url = fork_url.into_url().expect("Invalid fork URL");

        let forked_node_api = web3.api::<ForkedNodeApi<_>>();
        forked_node_api
            .fork(&fork_url)
            .await
            .expect("Test network must support anvil_reset");

        forked_node_api
            .impersonate(&solver_address)
            .await
            .expect("Test network must support anvil_impersonateAccount");

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
            .expect("Test network must support anvil_reset");
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
/// Relevant RPC calls for the Anvil network can be found at:
/// https://book.getfoundry.sh/reference/anvil/
impl<T: Transport> ForkedNodeApi<T> {
    pub fn fork(&self, fork_url: &Url) -> CallFuture<(), T::Out> {
        CallFuture::new(self.transport.execute(
            "anvil_reset",
            vec![json!({ "forking": {"jsonRpcUrl": fork_url.to_string()} })],
        ))
    }

    pub fn impersonate(&self, address: &H160) -> CallFuture<(), T::Out> {
        let json_address = serde_json::json!(address);
        CallFuture::new(
            self.transport
                .execute("anvil_impersonateAccount", vec![json_address]),
        )
    }
}
