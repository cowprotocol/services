use {
    ethcontract::H160,
    reqwest::Url,
    serde_json::json,
    std::fmt::Debug,
    web3::{api::Namespace, helpers::CallFuture, Transport},
};

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
