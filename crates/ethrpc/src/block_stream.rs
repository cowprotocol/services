use {
    crate::AlloyProvider,
    alloy_eips::{BlockId, BlockNumberOrTag},
    alloy_primitives::{B256, U256},
    alloy_provider::{Provider, ProviderBuilder},
    alloy_rpc_types::Block,
    alloy_transport_ws::WsConnect,
    anyhow::{Context as _, Result, anyhow},
    futures::StreamExt,
    std::{
        fmt::Debug,
        time::{Duration, Instant},
    },
    tokio::sync::watch,
    tokio_stream::wrappers::WatchStream,
    tracing::instrument,
    url::Url,
};

pub type BlockNumberHash = (u64, B256);

/// Block information.
#[derive(Clone, Copy, Debug, Eq)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
    pub gas_limit: U256,
    pub gas_price: U256,
    pub base_fee: u64,
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
            base_fee: Default::default(),
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

impl TryFrom<Block> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(value: Block) -> std::result::Result<Self, Self::Error> {
        value.header.try_into()
    }
}

impl TryFrom<alloy_rpc_types::Header> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(value: alloy_rpc_types::Header) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            number: value.number,
            hash: value.hash,
            parent_hash: value.parent_hash,
            timestamp: value.timestamp,
            gas_limit: U256::from(value.gas_limit),
            gas_price: value
                .base_fee_per_gas
                .map(U256::from)
                .context("no gas price")?,
            base_fee: value
                .base_fee_per_gas
                .ok_or_else(|| anyhow!("no base fee available"))?,
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
pub async fn current_block_ws_stream(
    alloy_provider: AlloyProvider,
    ws_url: Url,
) -> Result<CurrentBlockWatcher> {
    tracing::info!(?ws_url, "initializing block stream");

    // Create a WS transport, which implements an automatic reconnection mechanism
    let ws_connect = WsConnect::new(ws_url.as_str());
    let ws_provider = ProviderBuilder::new()
        .connect_ws(ws_connect)
        .await
        .context("failed to connect to websocket")?;

    // Init the block subscription stream before fetching the first block to reduce
    // chance of missing blocks due to race conditions
    let mut stream = ws_provider
        .subscribe_blocks()
        .await
        .context("failed to subscribe to blocks")?
        .into_stream();

    // Fetch the current block immediately via HTTP instead of waiting for WebSocket
    tracing::info!("fetching initial block via HTTP");
    let first_block = alloy_provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .context("failed to fetch latest block via HTTP")?
        .context("latest block not found")?;

    let first_block = BlockInfo::try_from(first_block).context("failed to parse initial block")?;

    let (sender, receiver) = watch::channel(first_block);
    let update_future = async move {
        // Keep WebSocket provider alive to maintain connection
        let _ws_provider = ws_provider;
        let mut previous_block = first_block;

        // Process incoming blocks. WsConnect handles reconnection automatically,
        // so we don't need manual reconnection logic here.
        while let Some(block) = stream.next().await {
            convert_block_and_process(block, &mut previous_block, &sender);
        }

        // If we reach here, the stream ended permanently
        tracing::error!("block stream ended after max reconnection attempts");
    };

    tokio::task::spawn(update_future);
    Ok(receiver)
}

#[instrument(skip_all)]
fn convert_block_and_process(
    block: alloy_rpc_types::Header,
    previous_block: &mut BlockInfo,
    sender: &watch::Sender<BlockInfo>,
) {
    let block_info = match BlockInfo::try_from(block.clone()) {
        Ok(info) => info,
        Err(err) => {
            tracing::error!(?err, ?block, "failed to parse block, skipping");
            return;
        }
    };
    handle_new_block(previous_block, block_info, sender);
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
#[deprecated(
    note = "Use `current_block_ws_stream` instead for real-time WebSocket-based block updates"
)]
pub async fn current_block_stream(
    url: Url,
    poll_interval: Duration,
) -> Result<CurrentBlockWatcher> {
    // Build an alloy transport specifically for the current block stream to avoid
    // batching requests together on chains with a very high block frequency.
    let (provider, _) =
        crate::alloy::unbuffered_provider(url.as_str(), Some("base_currentBlockStream"));

    let first_block = get_block_at_id(&provider, BlockId::latest()).await?;
    tracing::debug!(number=%first_block.number, hash=?first_block.hash, "polled block");

    let (sender, receiver) = watch::channel(first_block);
    let update_future = async move {
        let mut previous_block = first_block;
        loop {
            tokio::time::sleep(poll_interval).await;
            fetch_block_and_process(&provider, &mut previous_block, &sender).await;
        }
    };

    tokio::task::spawn(update_future);
    Ok(receiver)
}

#[instrument(skip_all)]
async fn fetch_block_and_process(
    provider: &AlloyProvider,
    previous_block: &mut BlockInfo,
    sender: &watch::Sender<BlockInfo>,
) {
    let block = match get_block_at_id(provider, BlockId::latest()).await {
        Ok(block) => block,
        Err(err) => {
            tracing::warn!("failed to get current block: {:?}", err);
            return;
        }
    };
    handle_new_block(previous_block, block, sender);
}

/// Applies a freshly observed block to the shared watcher, updating metrics
/// and forwarding it if it represents progress relative to `previous_block`.
fn handle_new_block(
    previous_block: &mut BlockInfo,
    new_block: BlockInfo,
    sender: &watch::Sender<BlockInfo>,
) {
    // If the block is exactly the same as the previous one, ignore it.
    if previous_block.hash == new_block.hash {
        return;
    }

    tracing::debug!(number=%new_block.number, hash=?new_block.hash, "observed new block");

    // Only update the stream if the number has increased.
    if new_block.number <= previous_block.number {
        return;
    }

    update_block_metrics(previous_block, &new_block);

    tracing::info!(number=%new_block.number, hash=?new_block.hash, "noticed a new block");
    if let Err(err) = sender.send(new_block) {
        tracing::error!(?err, "failed to send block to stream, aborting loop");
        panic!("block stream loop terminated due to sender failure");
    }

    *previous_block = new_block;
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

pub async fn get_block_at_id(provider: &AlloyProvider, id: BlockId) -> Result<BlockInfo> {
    let block = provider
        .get_block(id)
        .await
        .with_context(|| format!("failed to get block for {id:?}"))?
        .with_context(|| format!("no block for {id:?}"))?
        .try_into()?;

    Ok(block)
}

pub async fn timestamp_of_block_in_seconds(
    provider: &AlloyProvider,
    block_number: BlockNumberOrTag,
) -> Result<u32> {
    u32::try_from(
        provider
            .get_block_by_number(block_number)
            .await
            .with_context(|| format!("failed to get block {block_number:?}"))?
            .with_context(|| format!("no block for {block_number:?}"))?
            .header
            .timestamp,
    )
    .with_context(|| format!("block {block_number:?} timestamp is not u32"))
}

pub async fn timestamp_of_current_block_in_seconds(provider: &AlloyProvider) -> Result<u32> {
    timestamp_of_block_in_seconds(provider, BlockNumberOrTag::Latest).await
}

#[instrument(skip_all)]
pub async fn block_number_to_block_number_hash(
    provider: &AlloyProvider,
    block_number: BlockNumberOrTag,
) -> Result<BlockNumberHash> {
    let block = provider
        .get_block_by_number(block_number)
        .await?
        .with_context(|| format!("failed to find block {}", block_number))?;
    Ok((block.header.number, block.header.hash))
}

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// How much a new block number differs from the current block number.
    #[metric(buckets(0., 1., 2., 4., 8., 25.), labels("sign"))]
    block_stream_update_delta: prometheus::HistogramVec,

    /// Records newly observed block number.
    last_block_number: prometheus::core::GenericGauge<prometheus::core::AtomicU64>,

    /// Measures how much time passes between 2 blocks
    // buckets were chosen to have high resolution around target block times of various
    // chains (250ms, 500ms, 1s, 2s, 5s, 12s)
    #[metric(buckets(
        0., 0.1, 0.2, 0.25, 0.3, // 250ms
        0.4, 0.5, // 500ms
        0.75, 1., 1.25, // 1s
        1.5, 1.75, 2., 2.25, 2.5, // 2s
        4.25, 4.5, 5., 5.25, 5.5, // 5s
        10.25, 10.5, 10.75, 11., 11.25, 11.5, 11.75, 12., 12.25, 12.5, 12.75, 13., 13.25, 13.5,
        13.75, 14. // 12s
    ))]
    time_since_last_block: prometheus::Histogram,
}

fn update_block_metrics(previous_block: &BlockInfo, new_block: &BlockInfo) {
    let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();

    let delta = (i128::from(new_block.number) - i128::from(previous_block.number)) as f64;
    if delta <= 0. {
        metrics
            .block_stream_update_delta
            .with_label_values(&["negative"])
            .observe(delta.abs());
    } else {
        metrics
            .block_stream_update_delta
            .with_label_values(&["positive"])
            .observe(delta.abs());
    }

    metrics.last_block_number.set(new_block.number);
    metrics
        .time_since_last_block
        .observe(previous_block.observed_at.elapsed().as_secs_f64());
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
        crate::Web3,
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
        observe::tracing::init::initialize(
            &observe::Config::default().with_env_filter("shared=debug"),
        );

        let alloy_provider = Web3::new_from_env();
        let ws_node = std::env::var("NODE_WS_URL").unwrap().parse().unwrap();
        let receiver = current_block_ws_stream(alloy_provider.provider, ws_node)
            .await
            .unwrap();
        let mut stream = into_stream(receiver);
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number);
        }
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
