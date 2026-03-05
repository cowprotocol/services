use {
    alloy_eips::{BlockId, BlockNumberOrTag},
    alloy_primitives::B256,
    alloy_provider::Provider,
    alloy_rpc_types::Block,
    anyhow::{Context, Result, anyhow, ensure},
    ethrpc::{
        AlloyProvider,
        block_stream::{BlockInfo, BlockNumberHash, get_block_at_id},
    },
    futures::{TryStreamExt, stream::FuturesUnordered},
    std::fmt::Debug,
    tokio::sync::watch,
    tracing::instrument,
};

/// Trait for abstracting the retrieval of the block information such as the
/// latest block number.
#[async_trait::async_trait]
pub trait BlockRetrieving: Debug + Send + Sync + 'static {
    async fn current_block(&self) -> Result<BlockInfo>;
    async fn block(&self, number: u64) -> Result<BlockNumberHash>;
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>>;
}

#[async_trait::async_trait]
impl BlockRetrieving for AlloyProvider {
    #[instrument(skip_all)]
    async fn current_block(&self) -> Result<BlockInfo> {
        get_block_at_id(&self, BlockId::latest()).await
    }

    #[instrument(skip_all)]
    async fn block(&self, number: u64) -> Result<BlockNumberHash> {
        let block = get_block_at_id(&self, BlockId::number(number)).await?;
        Ok((block.number, block.hash))
    }

    /// Gets all blocks requested in the range. For successful results it's
    /// enforced that all the blocks are present, in the correct order and that
    /// there are no reorgs in the block range.
    #[instrument(skip_all)]
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>> {
        let (start, end) = range.into_inner();

        // Uses FuturesUnordered instead of try_join_all, since the latter
        // starts using FuturesOrdered once the number of futures exceeds 30, which
        // doesn't support fail-fast behavior.
        let futures = FuturesUnordered::new();
        for block_num in start..=end {
            let block_id = BlockNumberOrTag::Number(block_num).into();
            let provider = self.clone();
            futures.push(async move {
                provider
                    .get_block(block_id)
                    .await
                    .with_context(|| format!("failed to fetch block {block_num}"))?
                    .with_context(|| format!("missing block {block_num}"))
            });
        }

        let mut blocks: Vec<Block> = futures.try_collect().await?;

        // Sort the same way as the requested range
        blocks.sort_by_key(|block| block.number());

        let mut prev_hash = None;
        let mut result = Vec::with_capacity(blocks.len());

        for block in blocks {
            let current_hash: B256 = block.header.hash;
            if prev_hash.is_some_and(|prev| prev != block.header.parent_hash) {
                tracing::debug!(
                    start,
                    end,
                    ?prev_hash,
                    parent_hash = ?block.header.parent_hash,
                    block_number = ?block.number(),
                    "inconsistent parent in block range"
                );
                return Err(anyhow!("inconsistent block range"));
            }
            prev_hash = Some(current_hash);

            result.push((block.number(), current_hash));
        }

        Ok(result)
    }
}

/// Version of [`BlockRetrieving`] that's optimized for the usage
/// in the event indexing logic.
#[derive(Debug, Clone)]
pub struct BlockRetriever {
    pub provider: AlloyProvider,
    pub block_stream: watch::Receiver<BlockInfo>,
}

#[async_trait::async_trait]
impl BlockRetrieving for BlockRetriever {
    #[instrument(skip_all)]
    async fn current_block(&self) -> Result<BlockInfo> {
        Ok(*self.block_stream.borrow())
    }

    #[instrument(skip_all)]
    async fn block(&self, number: u64) -> Result<BlockNumberHash> {
        self.provider.block(number).await
    }

    #[instrument(skip_all)]
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>> {
        self.provider.blocks(range).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeInclusive<T: Ord> {
    start: T,
    end: T,
}

impl<T: Ord> RangeInclusive<T> {
    pub fn try_new(start: T, end: T) -> Result<Self> {
        ensure!(end >= start, "end has to be bigger or equal to start");
        Ok(Self { start, end })
    }

    pub fn start(&self) -> &T {
        &self.start
    }

    pub fn end(&self) -> &T {
        &self.end
    }

    pub fn into_inner(self) -> (T, T) {
        (self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ethrpc::Web3};

    #[tokio::test]
    #[ignore]
    async fn current_blocks_test() {
        let web3 = Web3::new_from_env();

        // single block
        let range = RangeInclusive::try_new(5, 5).unwrap();
        let blocks = web3.provider.blocks(range).await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks.last().unwrap().0, 5);

        // multiple blocks
        let range = RangeInclusive::try_new(5, 8).unwrap();
        let blocks = web3.provider.blocks(range).await.unwrap();
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks.last().unwrap().0, 8);
        assert_eq!(blocks.first().unwrap().0, 5);

        // shortened blocks
        let current_block_number = 5u64;
        let length = 25u64;
        let range = RangeInclusive::try_new(
            current_block_number.saturating_sub(length),
            current_block_number,
        )
        .unwrap();
        let blocks = web3.provider.blocks(range).await.unwrap();
        assert_eq!(blocks.len(), 6);
        assert_eq!(blocks.last().unwrap().0, 5);
        assert_eq!(blocks.first().unwrap().0, 0);
    }
}
