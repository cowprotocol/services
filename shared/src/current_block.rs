use crate::Web3;
use anyhow::{anyhow, Context as _, Result};

use futures::{stream::FusedStream, Stream};
use primitive_types::H256;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::watch;
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
) -> Result<CurrentBlockStream> {
    let first_block = web3.current_block().await?;
    let first_hash = first_block.hash.ok_or_else(|| anyhow!("missing hash"))?;

    let (sender, receiver) = watch::channel(first_block.clone());

    let most_recent = Arc::new(Mutex::new(first_block));
    let stream = CurrentBlockStream::new(receiver, most_recent.clone());

    let update_future = async move {
        let mut previous_hash = first_hash;
        loop {
            tokio::time::delay_for(poll_interval).await;
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
            if sender.broadcast(block.clone()).is_err() {
                break;
            }
            *most_recent.lock().unwrap() = block;
            previous_hash = hash;
        }
    };

    tokio::task::spawn(update_future);
    Ok(stream)
}

#[derive(Clone)]
pub struct CurrentBlockStream {
    receiver: watch::Receiver<Block>,
    most_recent: Arc<Mutex<Block>>,
    is_terminated: bool,
}

impl CurrentBlockStream {
    pub fn new(receiver: watch::Receiver<Block>, most_recent: Arc<Mutex<Block>>) -> Self {
        Self {
            receiver,
            most_recent,
            is_terminated: false,
        }
    }

    async fn next(&mut self) -> Option<Block> {
        if let Some(block) = self.receiver.recv().await {
            Some(block)
        } else {
            self.is_terminated = true;
            None
        }
    }

    /// The most recent block. Cached in the struct so it is always ready and up to date.
    pub fn current_block(&self) -> Block {
        self.most_recent.lock().unwrap().clone()
    }

    pub fn current_block_number(&self) -> Result<u64> {
        self.current_block()
            .number
            .ok_or_else(|| anyhow!("no block number"))
            .map(|number| number.as_u64())
    }
}

impl Stream for CurrentBlockStream {
    type Item = Block;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = self.get_mut().next();
        futures::pin_mut!(future);
        future.poll(cx)
    }
}

impl FusedStream for CurrentBlockStream {
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

/// Trait for abstracting the retrieval of the block information such as the
/// latest block number.
#[async_trait::async_trait]
pub trait BlockRetrieving {
    async fn current_block(&self) -> Result<Block>;
    async fn current_block_number(&self) -> Result<u64>;
    // TODO - break down next method into testable components
    // https://github.com/gnosis/gp-v2-services/issues/659
    async fn block_number_from_tx_hash(&self, hash: H256) -> Result<u64>;
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
            .context("failed to get current block")?
            .as_u64())
    }

    async fn block_number_from_tx_hash(&self, hash: H256) -> Result<u64> {
        Ok(self
            .eth()
            .transaction_receipt(hash)
            .await?
            .ok_or_else(|| anyhow!("no transaction receipt found"))?
            .block_number
            .ok_or_else(|| anyhow!("no block number with transaction receipt"))?
            .as_u64())
    }
}

#[cfg(test)]
mod tests {
    use crate::transport::LoggingTransport;

    use super::*;
    use futures::FutureExt;
    use primitive_types::H256;

    #[tokio::test]
    async fn stream_works() {
        let mut block = Block {
            hash: Some(H256::from_low_u64_be(0)),
            ..Default::default()
        };
        let (sender, receiver) = watch::channel(block.clone());
        let mut stream = CurrentBlockStream::new(receiver, Default::default());

        assert!(!stream.is_terminated());
        assert_eq!(stream.next().await, Some(block.clone()));

        block.hash = Some(H256::from_low_u64_be(1));
        sender.broadcast(block.clone()).unwrap();
        assert!(!stream.is_terminated());
        assert_eq!(stream.next().await, Some(block.clone()));

        assert!(stream.next().now_or_never().is_none());
        assert!(!stream.is_terminated());

        std::mem::drop(sender);
        assert_eq!(stream.next().await, None);
        assert!(stream.is_terminated());
    }

    // cargo test current_block -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        let node = "https://dev-openethereum.mainnet.gnosisdev.com";
        let transport = LoggingTransport::new(web3::transports::Http::new(node).unwrap());
        let web3 = Web3::new(transport);
        let mut stream = current_block_stream(web3, Duration::from_secs(1))
            .await
            .unwrap();
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number.unwrap().as_u64());
        }
    }
}
