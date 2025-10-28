use {
    crate::{
        Web3,
        alloy::conversions::{IntoAlloy, IntoLegacy},
    },
    alloy::{
        providers::{Provider, ProviderBuilder},
        transports::ws::WsConnect,
    },
    anyhow::{Context as _, Result, anyhow, ensure},
    futures::{StreamExt, future},
    primitive_types::{H256, U256},
    std::{
        fmt::Debug,
        num::NonZeroU64,
        time::{Duration, Instant},
    },
    tokio::sync::watch,
    tokio_stream::wrappers::WatchStream,
    tracing::{Instrument, instrument},
    url::Url,
    web3::types::{Block, BlockId, BlockNumber, U64},
};

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
#[derive(Clone, Copy, Debug, Eq)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: H256,
    pub parent_hash: H256,
    pub timestamp: u64,
    pub gas_limit: U256,
    pub gas_price: U256,
    /// When the system noticed the new block.
    pub observed_at: Instant,
}

impl Default for BlockInfo {
    fn default() -> Self {
        Self {
            number: Default::default(),
            hash: Default::default(),
            parent_hash: Default::default(),
            timestamp: Default::default(),
            gas_limit: Default::default(),
            gas_price: Default::default(),
            observed_at: Instant::now(),
        }
    }
}

impl PartialEq<Self> for BlockInfo {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
            && self.hash == other.hash
            && self.parent_hash == other.parent_hash
            && self.timestamp == other.timestamp
            && self.gas_limit == other.gas_limit
            && self.gas_price == other.gas_price
    }
}

impl TryFrom<alloy::rpc::types::Block> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(value: alloy::rpc::types::Block) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            number: value.header.number,
            hash: value.header.hash.into_legacy(),
            parent_hash: value.header.parent_hash.into_legacy(),
            timestamp: value.header.timestamp,
            gas_limit: primitive_types::U256::from(value.header.gas_limit),
            gas_price: value
                .header
                .base_fee_per_gas
                .map(primitive_types::U256::from)
                .context("no gas price")?,
            observed_at: Instant::now(),
        })
    }
}

impl TryFrom<alloy::rpc::types::Header> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(value: alloy::rpc::types::Header) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            number: value.number,
            hash: value.hash.into_legacy(),
            parent_hash: value.parent_hash.into_legacy(),
            timestamp: value.timestamp,
            gas_limit: primitive_types::U256::from(value.gas_limit),
            gas_price: value
                .base_fee_per_gas
                .map(primitive_types::U256::from)
                .context("no gas price")?,
            observed_at: Instant::now(),
        })
    }
}

/// Creates a cloneable stream that yields the current block whenever it
/// changes.
///
/// Uses websocket subscriptions for real-time block updates. The stream is not
/// guaranteed to yield *every* block individually without gaps but it does
/// yield the newest block whenever it detects a block number increase.
///
/// The stream is cloneable so that we only have to subscribe once while being
/// able to share the result with several consumers. Calling this function
/// again would create a new subscription so it is preferable to clone an
/// existing stream instead.
///
/// The websocket reconnection is handled by the alloy lib.
pub async fn current_block_stream(ws_url: Url) -> Result<CurrentBlockWatcher> {
    tracing::info!(?ws_url, "connecting to websocket for block stream");

    // Create a WS transport, which implements an automatic reconnection mechanism
    let ws_connect = WsConnect::new(ws_url.as_str());
    let provider = ProviderBuilder::new()
        .connect_ws(ws_connect)
        .await
        .context("failed to connect to websocket")?;

    let mut stream = provider
        .subscribe_blocks()
        .await
        .context("failed to subscribe to blocks")?
        .into_stream();

    tracing::info!("waiting for first block from websocket");
    let first_block = match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
        Ok(Some(block)) => BlockInfo::try_from(block.clone())
            .with_context(|| format!("failed to parse first block {:?}", block))?,
        Ok(None) => {
            anyhow::bail!("websocket stream ended before receiving first block");
        }
        Err(_) => {
            anyhow::bail!("timeout waiting for first block from websocket");
        }
    };

    let (sender, receiver) = watch::channel(first_block);
    let update_future = async move {
        let mut previous_block = first_block;

        // Process incoming blocks. WsConnect handles reconnection automatically,
        // so we don't need manual reconnection logic here.
        while let Some(block) = stream.next().await {
            let block_info = match BlockInfo::try_from(block.clone()) {
                Ok(info) => info,
                Err(err) => {
                    tracing::error!(?err, ?block, "failed to parse block, skipping");
                    continue;
                }
            };

            update_current_block_metrics(block_info.number);

            // If the block is exactly the same as the previous one, ignore it.
            if previous_block.hash == block_info.hash {
                continue;
            }

            tracing::debug!(number=%block_info.number, hash=?block_info.hash, "received block");
            update_block_metrics(previous_block.number, block_info.number);

            // Only update the stream if the number has increased.
            if block_info.number <= previous_block.number {
                continue;
            }

            tracing::info!(number=%block_info.number, hash=?block_info.hash, "noticed a new block");
            if let Err(err) = sender.send(block_info) {
                tracing::error!(
                    ?err,
                    "failed to send block to stream, aborting subscription loop"
                );
                panic!("subscription loop terminated due to sender failure");
            }

            previous_block = block_info;
        }

        // If we reach here, the stream ended permanently
        tracing::error!("block stream ended after max reconnection attempts");
    };

    tokio::task::spawn(update_future.instrument(tracing::info_span!("current_block_stream")));
    Ok(receiver)
}

/// Returns a stream that is synchronized to the passed in stream by only yields
/// every nth update of the original stream.
pub fn throttle(blocks: CurrentBlockWatcher, updates_to_skip: NonZeroU64) -> CurrentBlockWatcher {
    let first_block = *blocks.borrow();

    // `receiver` yields `first_block` immediately.
    let (sender, receiver) = watch::channel(first_block);

    let update_future = async move {
        let mut skipped_updates = 0;

        // The `block_stream` would yield `first_block` immediately and since `receiver`
        // is already guaranteed to yield that block by construction we skip 1
        // update right away to avoid yielding `first_block` twice from the
        // throttled stream.
        let mut block_stream = into_stream(blocks).skip(1);

        while let Some(block) = block_stream.next().await {
            if skipped_updates == updates_to_skip.get() {
                // reset counter
                skipped_updates = 0;
            } else {
                // Don't update the throttled stream because we didn't skip enough updates yet.
                skipped_updates += 1;
                continue;
            }

            if let Err(err) = sender.send(block) {
                tracing::error!(
                    ?err,
                    "failed to send block to stream, aborting polling loop"
                );
                panic!("polling loop terminated due to sender failure");
            }
        }
    };
    tokio::task::spawn(
        update_future.instrument(tracing::info_span!("current_block_stream_throttled")),
    );
    receiver
}

/// A method for creating a block stream with an initial value that never
/// observes any new blocks. This is useful for testing and creating "mock"
/// components.
pub fn mock_single_block(block: BlockInfo) -> CurrentBlockWatcher {
    let (sender, receiver) = watch::channel(block);
    // Make sure the `sender` never drops so the `receiver` stays open.
    std::mem::forget(sender);
    receiver
}

pub type CurrentBlockWatcher = watch::Receiver<BlockInfo>;

pub fn into_stream(receiver: CurrentBlockWatcher) -> WatchStream<BlockInfo> {
    WatchStream::new(receiver)
}

/// Trait for abstracting the retrieval of the block information such as the
/// latest block number.
#[async_trait::async_trait]
pub trait BlockRetrieving: Debug + Send + Sync + 'static {
    async fn current_block(&self) -> Result<BlockInfo>;
    async fn block(&self, number: u64) -> Result<BlockNumberHash>;
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>>;
}

#[async_trait::async_trait]
impl BlockRetrieving for crate::Web3 {
    async fn current_block(&self) -> Result<BlockInfo> {
        get_block_at_id(self, BlockNumber::Latest.into())
            .await?
            .try_into()
    }

    async fn block(&self, number: u64) -> Result<BlockNumberHash> {
        let block = get_block_at_id(self, U64::from(number).into()).await?;
        Ok((block.header.number, block.header.hash.into_legacy()))
    }

    /// Gets all blocks requested in the range. For successful results it's
    /// enforced that all the blocks are present, in the correct order and that
    /// there are not reorgs in the block range.
    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>> {
        let (start, end) = range.clone().into_inner();

        let alloy = self.alloy.clone();
        let block_futures: Vec<_> = (start..=end)
            .map(|block_num| {
                let block_id = alloy::eips::BlockNumberOrTag::Number(block_num).into();
                let alloy = alloy.clone();
                async move {
                    alloy
                        .get_block(block_id)
                        .await
                        .with_context(|| format!("failed to fetch block {block_num}"))
                }
            })
            .collect();

        let blocks: Vec<_> = future::try_join_all(block_futures).await?;

        let mut prev_hash = None;
        let mut result = Vec::with_capacity(blocks.len());

        for (block_num, block_opt) in (start..=end).zip(blocks) {
            let block = block_opt.with_context(|| format!("missing block {block_num}"))?;

            let current_hash: H256 = block.header.hash.into_legacy();
            let current_block = block.header.number;

            if prev_hash.is_some_and(|prev| prev != block.header.parent_hash.into_legacy()) {
                tracing::debug!(
                    ?range,
                    ?prev_hash,
                    parent_hash = ?block.header.parent_hash,
                    "inconsistent parent in block range"
                );
                return Err(anyhow!("inconsistent block range"));
            }
            prev_hash = Some(current_hash);

            result.push((current_block, current_hash));
        }

        Ok(result)
    }
}

async fn get_block_at_id(web3: &Web3, id: BlockId) -> Result<alloy::rpc::types::Block> {
    let block_id: alloy::eips::BlockId = id.into_alloy();
    let block = web3
        .alloy
        .get_block(block_id)
        .await
        .with_context(|| format!("failed to get block for {id:?}"))?
        .with_context(|| format!("no block for {id:?}"))?;

    Ok(block)
}

pub async fn timestamp_of_block_in_seconds(web3: &Web3, block_number: BlockNumber) -> Result<u32> {
    Ok(web3
        .eth()
        .block(block_number.into())
        .await
        .context("failed to get latest block")?
        .context("block should exist")?
        .timestamp
        .as_u32())
}

pub async fn timestamp_of_current_block_in_seconds(web3: &Web3) -> Result<u32> {
    timestamp_of_block_in_seconds(web3, BlockNumber::Latest).await
}

#[instrument(skip_all)]
pub async fn block_number_to_block_number_hash(
    web3: &Web3,
    block_number: BlockNumber,
) -> Result<BlockNumberHash> {
    let block = web3
        .eth()
        .block(BlockId::Number(block_number))
        .await?
        .context("block should exists")?;
    Ok((
        block.number.expect("number must exist").as_u64(),
        block.hash.expect("hash must exist"),
    ))
}

pub async fn block_by_number(web3: &Web3, block_number: BlockNumber) -> Option<Block<H256>> {
    web3.eth()
        .block(BlockId::Number(block_number))
        .await
        .ok()
        .flatten()
}

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// How much a new block number differs from the current block number.
    #[metric(buckets(0., 1., 2., 4., 8., 25.), labels("sign"))]
    block_stream_update_delta: prometheus::HistogramVec,

    /// Records newly observed block number.
    last_block_number: prometheus::core::GenericGauge<prometheus::core::AtomicU64>,
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

/// Records newly observed block number in the metrics.
fn update_current_block_metrics(block_number: u64) {
    let metric = &Metrics::instance(observe::metrics::get_storage_registry())
        .unwrap()
        .last_block_number;

    metric.set(block_number);
}

/// Awaits and returns the next block that will be pushed into the stream.
pub async fn next_block(current_block: &CurrentBlockWatcher) -> BlockInfo {
    let mut stream = into_stream(current_block.clone());
    // the stream always yields the current value right away
    // so we simply ignore it
    let _ = stream.next().await;
    stream.next().await.expect("block_stream must never end")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        futures::StreamExt,
        tokio::time::{Duration, timeout},
    };

    fn new_block(number: u64) -> BlockInfo {
        BlockInfo {
            number,
            ..Default::default()
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        observe::tracing::initialize(&observe::Config::default().with_env_filter("shared=debug"));

        let node = std::env::var("NODE_URL").unwrap().parse().unwrap();
        let receiver = current_block_stream(node).await.unwrap();
        let mut stream = into_stream(receiver);
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn current_blocks_test() {
        let web3 = Web3::new_from_env();

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
        let current_block_number = 5u64;
        let length = 25u64;
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

    // Tests that a throttled block stream indeed skips the configured
    // number of updates.
    // Always awaits the next block on a timer to not get the test stuck
    // when we want to assert that no new block is coming.
    #[tokio::test]
    async fn throttled_skips_blocks_test() {
        let (sender, receiver) = watch::channel(new_block(0));
        const TIMEOUT: Duration = Duration::from_millis(10);

        // stream that yields every other block
        let throttled = throttle(receiver, 1.try_into().unwrap());
        let mut stream = into_stream(throttled);

        // Initial block of the original stream gets yielded immediately.
        // which is consistent with an unthrottled stream.
        let block = timeout(TIMEOUT, stream.next()).await.unwrap().unwrap();
        assert_eq!(block.number, 0);

        // Doesn't yield the first block twice
        let block = timeout(TIMEOUT, stream.next()).await;
        assert!(block.is_err());

        sender.send(new_block(1)).unwrap();

        // first update gets skipped
        let block = timeout(TIMEOUT, stream.next()).await;
        assert!(block.is_err());

        sender.send(new_block(2)).unwrap();

        // second update gets forwarded
        let block = timeout(TIMEOUT, stream.next()).await.unwrap().unwrap();
        assert_eq!(block.number, 2);

        sender.send(new_block(3)).unwrap();

        // third update getes skipped again
        let block = timeout(TIMEOUT, stream.next()).await;
        assert!(block.is_err());

        sender.send(new_block(4)).unwrap();

        // fourth update gets forwarded again
        let block = timeout(TIMEOUT, stream.next()).await.unwrap().unwrap();
        assert_eq!(block.number, 4);
    }

    #[tokio::test]
    async fn test_next_block() {
        let (sender, receiver) = watch::channel(new_block(0));
        const TIMEOUT: Duration = Duration::from_millis(10);
        let result = timeout(TIMEOUT, next_block(&receiver)).await;
        // although there is already 1 block in the stream it does not get returned
        assert!(result.is_err());

        tokio::spawn(async move {
            tokio::time::sleep(TIMEOUT).await;
            let _ = sender.send(new_block(1));
        });

        let received_block = timeout(2 * TIMEOUT, next_block(&receiver)).await;
        assert_eq!(received_block, Ok(new_block(1)));
    }
}
