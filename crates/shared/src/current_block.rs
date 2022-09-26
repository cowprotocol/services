use crate::Web3;
use anyhow::{anyhow, ensure, Context as _, Result};
use primitive_types::H256;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use web3::{
    helpers,
    types::{BlockId, BlockNumber},
    BatchTransport, Transport,
};

pub type Block = web3::types::Block<H256>;
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

/// Creates a cloneable stream that yields the current block whenever it changes.
///
/// The stream is not guaranteed to yield *every* block individually without gaps but it does yield
/// the newest block whenever it detects a block number increase. In practice this means that if
/// the node changes the current block in quick succession we might only observe the last block,
/// skipping some blocks in between.
///
/// The stream is cloneable so that we only have to poll the node once while being able to share the
/// result with several consumers. Calling this function again would create a new poller so it is
/// preferable to clone an existing stream instead.
pub async fn current_block_stream(
    web3: Web3,
    poll_interval: Duration,
) -> Result<watch::Receiver<Block>> {
    let first_block = web3.current_block().await?;
    let first_hash = first_block.hash.ok_or_else(|| anyhow!("missing hash"))?;
    let first_number = first_block
        .number
        .ok_or_else(|| anyhow!("missing number"))?;

    let current_block_number = Arc::new(AtomicU64::new(first_number.as_u64()));

    let (sender, receiver) = watch::channel(first_block);

    let update_future = async move {
        let mut previous_hash = first_hash;
        loop {
            tokio::time::sleep(poll_interval).await;
            let block = match web3.current_block().await {
                Ok(block) => block,
                Err(err) => {
                    tracing::warn!("failed to get current block: {:?}", err);
                    continue;
                }
            };
            let hash = match block.hash {
                Some(hash) => hash,
                None => {
                    tracing::warn!("missing hash");
                    continue;
                }
            };
            if hash == previous_hash {
                continue;
            }
            let number = match block.number {
                Some(number) => number.as_u64(),
                None => {
                    tracing::warn!("missing block number");
                    continue;
                }
            };

            if !block_number_increased(current_block_number.as_ref(), number) {
                continue;
            }

            tracing::debug!(%number, %hash, "new block");
            if sender.send(block.clone()).is_err() {
                break;
            }
            previous_hash = hash;
        }
    };

    tokio::task::spawn(update_future);
    Ok(receiver)
}

/// A method for creating a block stream with an initial value that never observes any new blocks.
/// This is useful for testing and creating "mock" components.
pub fn mock_single_block(block: Block) -> CurrentBlockStream {
    let (sender, receiver) = watch::channel(block);
    // Make sure the `sender` never drops so the `receiver` stays open.
    std::mem::forget(sender);
    receiver
}

pub type CurrentBlockStream = watch::Receiver<Block>;

pub fn into_stream(receiver: watch::Receiver<Block>) -> WatchStream<Block> {
    WatchStream::new(receiver)
}

pub fn block_number(block: &Block) -> Result<u64> {
    block
        .number
        .map(|number| number.as_u64())
        .ok_or_else(|| anyhow!("no block number"))
}

/// Trait for abstracting the retrieval of the block information such as the
/// latest block number.
#[async_trait::async_trait]
pub trait BlockRetrieving {
    async fn current_block(&self) -> Result<Block>;
    async fn current_block_number(&self) -> Result<u64>;
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>>;
}

#[async_trait::async_trait]
impl BlockRetrieving for Web3 {
    async fn current_block(&self) -> Result<Block> {
        self.eth()
            .block(BlockId::Number(BlockNumber::Latest))
            .await
            .context("failed to get current block")?
            .ok_or_else(|| anyhow!("no current block"))
    }

    async fn current_block_number(&self) -> Result<u64> {
        Ok(self
            .eth()
            .block_number()
            .await
            .context("failed to get current block number")?
            .as_u64())
    }

    /// get blocks defined by the range (inclusive)
    /// if successful, function guarantees full range of blocks in Result (does not return partial results)
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

        // send_batch guarantees the size and order of the responses to match the requests
        self.transport()
            .send_batch(batch_request.iter().cloned())
            .await?
            .into_iter()
            .map(|response| match response {
                Ok(response) => serde_json::from_value::<web3::types::Block<H256>>(response)
                    .context("unexpected response format")
                    .and_then(|response| {
                        Ok((
                            response.number.context("missing block number")?.as_u64(),
                            response.hash.context("missing hash")?,
                        ))
                    }),
                Err(err) => Err(anyhow!("web3 error: {}", err)),
            })
            .collect()
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// How much a new block number differs from the current block number.
    #[metric(buckets(0., 1., 2., 4., 8., 25.), labels("sign"))]
    block_stream_update_delta: prometheus::HistogramVec,
}

/// Only updates the current block if the new block number is strictly bigger than the current one.
/// Updates metrics about the difference of the new block number compared to the current block.
fn block_number_increased(current_block: &AtomicU64, new_block: u64) -> bool {
    let current_block = current_block.fetch_max(new_block, Ordering::SeqCst);
    let metric = &Metrics::instance(global_metrics::get_metric_storage_registry())
        .unwrap()
        .block_stream_update_delta;

    let delta = (i128::from(new_block) - i128::from(current_block)) as f64;
    if delta <= 0. {
        tracing::debug!(delta, new_block, "ignored new block number");
        metric.with_label_values(&["negative"]).observe(delta.abs());
    } else {
        tracing::debug!(delta, new_block, "increased current block number");
        metric.with_label_values(&["positive"]).observe(delta.abs());
    }

    new_block > current_block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{create_env_test_transport, create_test_transport};
    use futures::StreamExt;
    use num::Saturating;

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        let node = std::env::var("NODE_URL").unwrap();
        let transport = create_test_transport(&node);
        let web3 = Web3::new(transport);
        let receiver = current_block_stream(web3, Duration::from_secs(1))
            .await
            .unwrap();
        let mut stream = into_stream(receiver);
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number.unwrap().as_u64());
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

    #[test]
    fn more_recent_block_gets_propagated() {
        let current_block = AtomicU64::new(100);
        assert!(block_number_increased(&current_block, 101));
        assert_eq!(current_block.load(Ordering::SeqCst), 101);
    }

    #[test]
    fn outdated_block_does_not_get_propagated() {
        let current_block = AtomicU64::new(100);
        assert!(!block_number_increased(&current_block, 100));
        assert_eq!(current_block.load(Ordering::SeqCst), 100);

        assert!(!block_number_increased(&current_block, 99));
        assert_eq!(current_block.load(Ordering::SeqCst), 100);
    }
}
