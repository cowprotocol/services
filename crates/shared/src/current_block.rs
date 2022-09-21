use crate::{event_handling::MAX_REORG_BLOCK_COUNT, Web3};
use anyhow::{anyhow, Context as _, Result};
use primitive_types::H256;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use web3::{
    types::{BlockId, BlockNumber},
    Transport,
};

pub type Block = web3::types::Block<H256>;

/// Creates a cloneable stream that yields the current block whenever it changes.
///
/// The stream is not guaranteed to yield *every* block individually without gaps but it does yield
/// the newest block whenever it changes. In practice this means that if the node changes the
/// current block in quick succession we might only observe the last block, skipping some blocks in
/// between.
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

    let current_block_number = Arc::new(Mutex::new(first_number.as_u64()));

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

            if !updated_current_block_with_new_block(current_block_number.as_ref(), number) {
                tracing::error!("new block number is too old");
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
}

#[async_trait::async_trait]
impl<T> BlockRetrieving for web3::Web3<T>
where
    T: Transport + Send + Sync,
    T::Out: Send,
{
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
}

/// Determines if a new block is recent enough that it would warrant informing the system about it.
/// This is used to protect against receiving very old blocks after the load balancer switched to a
/// different node that is stuck in the past. The new block is allowed to be at most
/// `MAX_REORG_BLOCK_COUNT` blocks older than the current block to still get notified about reorgs.
fn updated_current_block_with_new_block(current_block: &Mutex<u64>, new_block: u64) -> bool {
    let mut block_guard = current_block.lock().unwrap();
    if new_block >= (*block_guard).saturating_sub(MAX_REORG_BLOCK_COUNT) {
        // new block is within reorg depth -> update current block
        *block_guard = new_block;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::create_test_transport;
    use futures::StreamExt;

    // cargo test current_block -- --ignored --nocapture
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

    #[test]
    fn more_recent_block_gets_propagated() {
        let current_block = Mutex::new(100);
        assert!(updated_current_block_with_new_block(&current_block, 101));
        assert_eq!(*current_block.lock().unwrap(), 101);
    }

    #[test]
    fn block_within_reorg_depth_gets_propagated() {
        let current_block = Mutex::new(100);
        assert!(updated_current_block_with_new_block(&current_block, 100));
        assert_eq!(*current_block.lock().unwrap(), 100);

        let current_block = Mutex::new(100);
        assert!(updated_current_block_with_new_block(
            &current_block,
            100 - MAX_REORG_BLOCK_COUNT
        ));
        assert_eq!(*current_block.lock().unwrap(), 100 - MAX_REORG_BLOCK_COUNT);
    }

    #[test]
    fn vastly_outdated_block_does_not_get_propagated() {
        let current_block = Mutex::new(100);
        assert!(!updated_current_block_with_new_block(
            &current_block,
            100 - (MAX_REORG_BLOCK_COUNT + 1)
        ));
        assert_eq!(*current_block.lock().unwrap(), 100);
    }
}
