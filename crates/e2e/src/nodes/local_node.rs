use {
    super::TestNode,
    chrono::{DateTime, Utc},
    ethcontract::U256,
    std::fmt::Debug,
    web3::{api::Namespace, helpers::CallFuture, Transport},
};

pub struct Resetter<T> {
    test_node_api: TestNodeApi<T>,
    snapshot_id: U256,
}

impl<T: Transport> Resetter<T> {
    pub async fn new(web3: &web3::Web3<T>) -> Self {
        let test_node_api = web3.api::<TestNodeApi<_>>();
        let snapshot_id = test_node_api
            .snapshot()
            .await
            .expect("Test network must support evm_snapshot");
        Self {
            test_node_api,
            snapshot_id,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<T: Transport> TestNode for Resetter<T> {
    async fn reset(&self) {
        self.test_node_api
            .revert(&self.snapshot_id)
            .await
            .expect("Test network must support evm_revert");
    }
}

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
/// Relevant RPC calls for the Hardhat network can be found at:
/// https://hardhat.org/hardhat-network/docs/reference#special-testing/debugging-methods
impl<T: Transport> TestNodeApi<T> {
    pub fn snapshot(&self) -> CallFuture<U256, T::Out> {
        CallFuture::new(self.transport.execute("evm_snapshot", vec![]))
    }

    pub fn revert(&self, snapshot_id: &U256) -> CallFuture<bool, T::Out> {
        let value_id = serde_json::json!(snapshot_id);
        CallFuture::new(self.transport.execute("evm_revert", vec![value_id]))
    }

    pub fn set_next_block_timestamp(&self, datetime: &DateTime<Utc>) -> CallFuture<String, T::Out> {
        let json_timestamp = serde_json::json!(datetime.timestamp());
        CallFuture::new(
            self.transport
                .execute("evm_setNextBlockTimestamp", vec![json_timestamp]),
        )
    }

    pub fn mine_pending_block(&self) -> CallFuture<String, T::Out> {
        CallFuture::new(self.transport.execute("evm_mine", vec![]))
    }
}
