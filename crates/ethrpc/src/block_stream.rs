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
    tracing::{Instrument, instrument},
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
            let block = match get_block_at_id(&provider, BlockId::latest()).await {
                Ok(block) => block,
                Err(err) => {
                    tracing::warn!("failed to get current block: {:?}", err);
                    continue;
                }
            };

            update_current_block_metrics(block.number);

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

            tracing::info!(number=%block.number, hash=?block.hash, "noticed a new block");
            if let Err(err) = sender.send(block) {
                tracing::error!(
                    ?err,
                    "failed to send block to stream, aborting polling loop"
                );
                panic!("polling loop terminated due to sender failure");
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
