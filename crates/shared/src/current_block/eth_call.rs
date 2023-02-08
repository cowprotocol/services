use {
    super::{BlockInfo, BlockNumberHash, BlockRetrieving, RangeInclusive},
    crate::ethrpc::Web3,
    anyhow::{ensure, Context as _, Result},
    contracts::support::FetchBlock,
    web3::types::{BlockNumber, CallRequest, H256, U256},
};

/// An `eth_call`-based block fetcher.
///
/// This can be used for nodes where `eth_getBlockBy*` and `eth_blockNumber`
/// calls return the latest block for which a header is available even if the
/// state isn't. This can be an issue for our services where we will try to
/// update internal state to the new block even if the node we are connected to
/// does not have the block available.
pub struct BlockRetriever(pub Web3);

#[async_trait::async_trait]
impl BlockRetrieving for BlockRetriever {
    async fn current_block(&self) -> Result<BlockInfo> {
        let return_data = self
            .0
            .eth()
            .call(
                CallRequest {
                    data: Some(bytecode!(FetchBlock)),
                    ..Default::default()
                },
                Some(BlockNumber::Pending.into()),
            )
            .await
            .context("failed to execute block fetch call")?
            .0;

        ensure!(
            return_data.len() == 96,
            "failed to decode block fetch result"
        );
        let number = u64::try_from(U256::from_big_endian(&return_data[0..32]))
            .ok()
            .context("block number overflows u64")?;
        let hash = H256::from_slice(&return_data[32..64]);
        let parent_hash = H256::from_slice(&return_data[64..96]);

        Ok(BlockInfo {
            number,
            hash,
            parent_hash,
        })
    }

    async fn block(&self, number: u64) -> Result<BlockNumberHash> {
        self.0.block(number).await
    }

    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>> {
        self.0.blocks(range).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::ethrpc::create_env_test_transport};

    #[ignore]
    #[tokio::test]
    async fn mainnet() {
        let retriever = BlockRetriever(Web3::new(create_env_test_transport()));
        let block = retriever.current_block().await.unwrap();
        println!("current block: {block:#?}");
    }
}
