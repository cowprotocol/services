use {
    ethcontract::{Account, H160, U256},
    reqwest::Url,
    serde_json::json,
    std::fmt::Debug,
    web3::{api::Namespace, helpers::CallFuture, Transport},
};

/// The block number from which we will fetch state for forked e2e tests.
pub const FORK_BLOCK: u64 = 18477910;
/// USDC whale address as per [FORK_BLOCK].
pub const USDC_WHALE: H160 = H160(hex_literal::hex!(
    "28c6c06298d514db089934071355e5743bf21d60"
));

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

    pub async fn impersonate(&self, address: &H160) -> Result<Account, web3::Error> {
        let json_address = serde_json::json!(address);
        self.transport
            .execute("anvil_impersonateAccount", vec![json_address])
            .await?;

        Ok(Account::Local(*address, None))
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
        let json_balance = serde_json::json!(format!("{:#032x}", balance));
        CallFuture::new(
            self.transport
                .execute("anvil_setBalance", vec![json_address, json_balance]),
        )
    }
}
