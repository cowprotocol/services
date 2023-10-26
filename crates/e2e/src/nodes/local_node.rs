use {
    chrono::{DateTime, Utc},
    ethcontract::{H160, U256},
    std::fmt::Debug,
    web3::{api::Namespace, helpers::CallFuture, Transport},
};

#[derive(Debug, Clone)]
pub struct TestNodeApi<T> {
    transport: T,
}

impl<T: Transport> Namespace<T> for TestNodeApi<T> {
    fn new(transport: T) -> Self
    where
        Self: Sized,
    {
        TestNodeApi { transport }
    }

    fn transport(&self) -> &T {
        &self.transport
    }
}

/// Implements functions that are only available in a testing node.
///
/// Relevant RPC calls for an Anvil node can be found at:
/// https://book.getfoundry.sh/reference/anvil/
impl<T: Transport> TestNodeApi<T> {
    pub fn snapshot(&self) -> CallFuture<U256, T::Out> {
        CallFuture::new(self.transport.execute("evm_snapshot", vec![]))
    }

    pub fn revert(&self, snapshot_id: &U256) -> CallFuture<bool, T::Out> {
        let value_id = serde_json::json!(snapshot_id);
        CallFuture::new(self.transport.execute("evm_revert", vec![value_id]))
    }

    pub fn set_next_block_timestamp(&self, datetime: &DateTime<Utc>) -> CallFuture<(), T::Out> {
        let json_timestamp = serde_json::json!(datetime.timestamp());
        CallFuture::new(
            self.transport
                .execute("evm_setNextBlockTimestamp", vec![json_timestamp]),
        )
    }

    pub fn mine_pending_block(&self) -> CallFuture<String, T::Out> {
        CallFuture::new(self.transport.execute("evm_mine", vec![]))
    }

    pub fn set_balance(&self, address: &H160, balance: &U256) -> CallFuture<(), T::Out> {
        let json_address = serde_json::json!(address);
        let json_balance = serde_json::json!(format!("{:#032x}", balance));
        CallFuture::new(
            self.transport
                .execute("anvil_setBalance", vec![json_address, json_balance]),
        )
    }
}
