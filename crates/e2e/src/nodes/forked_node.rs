use {
    super::TestNode,
    crate::setup::to_wei,
    ethcontract::{H160, U256},
    ethrpc::{create_test_transport, Web3},
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

        let http = create_test_transport(fork_url.as_str());
        let remote_web3 = Web3::new(http);

        let chain_id = remote_web3
            .eth()
            .chain_id()
            .await
            .expect("Error getting chain ID")
            .as_u64();

        forked_node_api
            .set_chain_id(chain_id)
            .await
            .expect("Test network must support anvil_setChainId");

        forked_node_api
            .fork(&fork_url)
            .await
            .expect("Test network must support anvil_reset");

        // fund default accounts, as tests expect them to have a balance
        let default_accounts = web3.eth().accounts().await.expect("Error getting accounts");
        for account in default_accounts {
            forked_node_api
                .set_balance(&account, to_wei(10000))
                .await
                .expect("Test network must support anvil_setBalance");
        }

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

    pub fn set_chain_id(&self, chain_id: u64) -> CallFuture<(), T::Out> {
        let json_chain_id = serde_json::json!(chain_id);
        CallFuture::new(
            self.transport
                .execute("anvil_setChainId", vec![json_chain_id]),
        )
    }

    pub fn set_balance(&self, address: &H160, balance: U256) -> CallFuture<(), T::Out> {
        let json_address = serde_json::json!(address);
        let json_balance = serde_json::json!(balance);
        CallFuture::new(
            self.transport
                .execute("anvil_setBalance", vec![json_address, json_balance]),
        )
    }

    pub fn set_storage_at(
        &self,
        address: &H160,
        slot: &str,
        value: &str,
    ) -> CallFuture<bool, T::Out> {
        let json_address = serde_json::json!(address);
        let json_slot = serde_json::json!(slot);
        let json_value = serde_json::json!(value);
        CallFuture::new(self.transport.execute(
            "anvil_setStorageAt",
            vec![json_address, json_slot, json_value],
        ))
    }
}
