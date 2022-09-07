use crate::{event_handling::BlockNumberHash, Web3};
use anyhow::{anyhow, ensure, Context as _, Result};
use primitive_types::H256;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use web3::{
    helpers,
    types::{BlockId, BlockNumber},
    BatchTransport, Transport,
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
    /// gets latest `length` blocks
    async fn current_blocks(&self, last_block: u64, length: u64) -> Result<Vec<BlockNumberHash>>;
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

    /// get latest `length` blocks
    /// if successful, function guarantees `length` blocks in Result (does not return partial results)
    /// `last_block` block is at the end of the resulting vector
    async fn current_blocks(&self, last_block: u64, length: u64) -> Result<Vec<BlockNumberHash>> {
        ensure!(
            length > 0 && last_block > 0 && last_block >= length,
            "invalid input"
        );

        let include_txs = helpers::serialize(&false);
        let mut batch_request = Vec::with_capacity(length as usize);
        for i in (0..length).rev() {
            let num =
                helpers::serialize(&BlockNumber::Number((last_block.saturating_sub(i)).into()));
            let request = self
                .transport()
                .prepare("eth_getBlockByNumber", vec![num, include_txs.clone()]);
            batch_request.push(request);
        }

        // send_batch guarantees the size and order of the responses to match the requests
        let mut batch_response = self
            .transport()
            .send_batch(batch_request.iter().cloned())
            .await?
            .into_iter();

        batch_request
            .into_iter()
            .map(|_req| match batch_response.next().unwrap() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{create_env_test_transport, create_test_transport};
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

    #[tokio::test]
    #[ignore]
    async fn current_blocks_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let current_block = web3.current_block().await.unwrap();
        let blocks = web3
            .current_blocks(current_block.number.unwrap().as_u64(), 0)
            .await;
        assert!(blocks.is_err());
        let blocks = web3
            .current_blocks(current_block.number.unwrap().as_u64(), 5)
            .await
            .unwrap();
        dbg!(blocks);
    }
}
