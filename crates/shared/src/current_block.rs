mod arguments;
mod retriever;

use {
    crate::ethrpc::Web3,
    anyhow::{anyhow, ensure, Context as _, Result},
    primitive_types::H256,
    std::{sync::Arc, time::Duration},
    tokio::sync::watch,
    tokio_stream::wrappers::WatchStream,
    tracing::Instrument,
    web3::{
        helpers,
        types::{Block, BlockId, BlockNumber, U64},
        BatchTransport,
        Transport,
    },
};

pub use self::arguments::Arguments;

pub type BlockNumberHash = (u64, H256);

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

/// Block information.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: H256,
    pub parent_hash: H256,
}

impl TryFrom<Block<H256>> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(value: Block<H256>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            number: value.number.context("block missing number")?.as_u64(),
            hash: value.hash.context("block missing hash")?,
            parent_hash: value.parent_hash,
        })
    }
}

/// Creates a cloneable stream that yields the current block whenever it
/// changes.
///
/// The stream is not guaranteed to yield *every* block individually without
/// gaps but it does yield the newest block whenever it detects a block number
/// increase. In practice this means that if the node changes the current block
/// in quick succession we might only observe the last block, skipping some
/// blocks in between.
///
/// The stream is cloneable so that we only have to poll the node once while
/// being able to share the result with several consumers. Calling this function
/// again would create a new poller so it is preferable to clone an existing
/// stream instead.
pub async fn current_block_stream(
    retriever: Arc<dyn BlockRetrieving>,
    poll_interval: Duration,
) -> Result<CurrentBlockStream> {
    let first_block = retriever.current_block().await?;
    tracing::debug!(number=%first_block.number, hash=?first_block.hash, "polled block");

    let (sender, receiver) = watch::channel(first_block);
    let update_future = async move {
        let mut previous_block = first_block;
        loop {
            tokio::time::sleep(poll_interval).await;
            let block = match retriever.current_block().await {
                Ok(block) => block,
                Err(err) => {
                    tracing::warn!("failed to get current block: {:?}", err);
                    continue;
                }
            };

            // If the block is exactly the same, ignore it.
            if previous_block.hash == block.hash {
                continue;
            }

            // The new block is different but might still have the same number.

            tracing::debug!(number=%block.number, hash=?block.hash, "polled block");
            update_block_metrics(previous_block.number, block.number);

            // Only update the stream if the number has increased.
            if block.number <= previous_block.number {
                continue;
            }

            if sender.send(block).is_err() {
                tracing::debug!("exiting polling loop");
                break;
            }

            previous_block = block;
        }
    };

    tokio::task::spawn(update_future.instrument(tracing::info_span!("current_block_stream")));
    Ok(receiver)
}

/// A method for creating a block stream with an initial value that never
/// observes any new blocks. This is useful for testing and creating "mock"
/// components.
pub fn mock_single_block(block: BlockInfo) -> CurrentBlockStream {
    let (sender, receiver) = watch::channel(block);
    // Make sure the `sender` never drops so the `receiver` stays open.
    std::mem::forget(sender);
    receiver
}

pub type CurrentBlockStream = watch::Receiver<BlockInfo>;

pub fn into_stream(receiver: CurrentBlockStream) -> WatchStream<BlockInfo> {
    WatchStream::new(receiver)
}

/// Trait for abstracting the retrieval of the block information such as the
/// latest block number.
#[async_trait::async_trait]
pub trait BlockRetrieving: Send + Sync + 'static {
    async fn current_block(&self) -> Result<BlockInfo>;
    async fn block(&self, number: u64) -> Result<BlockNumberHash>;
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>>;
}

#[async_trait::async_trait]
impl BlockRetrieving for Web3 {
    async fn current_block(&self) -> Result<BlockInfo> {
        get_block_info_at_id(self, BlockNumber::Latest.into()).await
    }

    async fn block(&self, number: u64) -> Result<BlockNumberHash> {
        let block = get_block_info_at_id(self, U64::from(number).into()).await?;
        Ok((block.number, block.hash))
    }

    /// get blocks defined by the range (inclusive)
    /// if successful, function guarantees full range of blocks in Result (does
    /// not return partial results)
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>> {
        let include_txs = helpers::serialize(&false);
        let (start, end) = range.into_inner();
        let mut batch_request = Vec::with_capacity((end - start + 1) as usize);
        for i in start..=end {
            let num = helpers::serialize(&BlockNumber::Number(i.into()));
            let request = self
                .transport()
                .prepare("eth_getBlockByNumber", vec![num, include_txs.clone()]);
            batch_request.push(request);
        }

        // send_batch guarantees the size and order of the responses to match the
        // requests
        self.transport()
            .send_batch(batch_request.iter().cloned())
            .await?
            .into_iter()
            .map(|response| match response {
                Ok(response) => {
                    serde_json::from_value::<web3::types::Block<H256>>(response.clone())
                        .with_context(|| format!("unexpected response format: {response:?}"))
                        .and_then(|response| {
                            Ok((
                                response.number.context("missing block number")?.as_u64(),
                                response.hash.context("missing hash")?,
                            ))
                        })
                }
                Err(err) => Err(anyhow!("web3 error: {}", err)),
            })
            .collect()
    }
}

async fn get_block_info_at_id(web3: &Web3, id: BlockId) -> Result<BlockInfo> {
    web3.eth()
        .block(id)
        .await
        .with_context(|| format!("failed to get block for {id:?}"))?
        .with_context(|| format!("no block for {id:?}"))?
        .try_into()
}

pub async fn timestamp_of_block_in_seconds(web3: &Web3, block_number: BlockNumber) -> Result<u32> {
    Ok(web3
        .eth()
        .block(block_number.into())
        .await
        .context("failed to get latest block")?
        .context("block should exists")?
        .timestamp
        .as_u32())
}

pub async fn timestamp_of_current_block_in_seconds(web3: &Web3) -> Result<u32> {
    timestamp_of_block_in_seconds(web3, BlockNumber::Latest).await
}

pub async fn block_number_to_block_number_hash(
    web3: &Web3,
    block_number: BlockNumber,
) -> Option<BlockNumberHash> {
    web3.eth()
        .block(BlockId::Number(block_number))
        .await
        .ok()
        .flatten()
        .map(|block| {
            (
                block.number.expect("number must exist").as_u64(),
                block.hash.expect("hash must exist"),
            )
        })
}

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// How much a new block number differs from the current block number.
    #[metric(buckets(0., 1., 2., 4., 8., 25.), labels("sign"))]
    block_stream_update_delta: prometheus::HistogramVec,
}

/// Updates metrics about the difference of the new block number compared to the
/// current block.
fn update_block_metrics(current_block: u64, new_block: u64) {
    let metric = &Metrics::instance(observe::metrics::get_storage_registry())
        .unwrap()
        .block_stream_update_delta;

    let delta = (i128::from(new_block) - i128::from(current_block)) as f64;
    if delta <= 0. {
        metric.with_label_values(&["negative"]).observe(delta.abs());
    } else {
        metric.with_label_values(&["positive"]).observe(delta.abs());
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::ethrpc::{create_env_test_transport, create_test_transport},
        futures::StreamExt,
        num::Saturating,
    };

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        observe::tracing::initialize_reentrant("shared=debug");
        let node = std::env::var("NODE_URL").unwrap();
        let transport = create_test_transport(&node);
        let web3 = Web3::new(transport);
        let receiver = current_block_stream(Arc::new(web3), Duration::from_secs(1))
            .await
            .unwrap();
        let mut stream = into_stream(receiver);
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn current_blocks_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);

        // single block
        let range = RangeInclusive::try_new(5, 5).unwrap();
        let blocks = web3.blocks(range).await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks.last().unwrap().0, 5);

        // multiple blocks
        let range = RangeInclusive::try_new(5, 8).unwrap();
        let blocks = web3.blocks(range).await.unwrap();
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks.last().unwrap().0, 8);
        assert_eq!(blocks.first().unwrap().0, 5);

        // shortened blocks
        let current_block_number = 5;
        let length = 25;
        let range = RangeInclusive::try_new(
            current_block_number.saturating_sub(length),
            current_block_number,
        )
        .unwrap();
        let blocks = web3.blocks(range).await.unwrap();
        assert_eq!(blocks.len(), 6);
        assert_eq!(blocks.last().unwrap().0, 5);
        assert_eq!(blocks.first().unwrap().0, 0);
    }
}
