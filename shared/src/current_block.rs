use anyhow::{anyhow, Context as _, Result};
use ethcontract::transport::DynTransport;
use futures::{stream::FusedStream, Stream};
use primitive_types::H256;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::watch;
use web3::{
    types::{BlockId, BlockNumber},
    Web3,
};

pub type Block = web3::types::Block<H256>;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Creates a clonable stream that yields the current block whenever it changes.
///
/// The stream is not guaranteed to yield *every* block individually without gaps but it does yield
/// the newest block whenever it changes. In practice this means that if the node changes the
/// current block in quick succession we might only observe the last block, skipping some blocks in
/// between.
///
/// The stream is clonable so that we only have to poll the node once while being able to share the
/// result with several consumers. Calling this function again would create a new poller so it is
/// preferable to clone an existing stream instead.
pub fn current_block_stream(web3: Web3<DynTransport>) -> CurrentBlockStream {
    let (sender, receiver) = watch::channel(None);

    let update_future = async move {
        let mut previous_hash = H256::default();
        loop {
            tokio::time::delay_for(POLL_INTERVAL).await;
            let block = match current_block(&web3).await {
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
            if sender.broadcast(Some(block)).is_err() {
                break;
            }
            previous_hash = hash;
        }
    };

    tokio::task::spawn(update_future);
    CurrentBlockStream::new(receiver)
}

#[derive(Clone)]
pub struct CurrentBlockStream {
    receiver: watch::Receiver<Option<Block>>,
    is_terminated: bool,
}

impl CurrentBlockStream {
    fn new(receiver: watch::Receiver<Option<Block>>) -> Self {
        Self {
            receiver,
            is_terminated: false,
        }
    }

    async fn next(&mut self) -> Option<Block> {
        loop {
            // recv returns Option<Option<Block>>. If the outer option is None then the sender has
            // been dropped in which case the stream ends. If the inner option is None then this is
            // because we have fetched the initial default value so we loop and try again.
            match self.receiver.recv().await {
                Some(None) => (),
                Some(Some(block)) => return Some(block),
                None => {
                    self.is_terminated = true;
                    return None;
                }
            }
        }
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

async fn current_block(web3: &Web3<DynTransport>) -> Result<Block> {
    web3.eth()
        .block(BlockId::Number(BlockNumber::Latest))
        .await
        .context("failed to get current block")?
        .ok_or_else(|| anyhow!("no current block"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stream_works() {
        let (sender, receiver) = watch::channel(None);
        let block = Block::default();
        sender.broadcast(Some(block)).unwrap();
        let mut stream = CurrentBlockStream::new(receiver);
        assert!(!stream.is_terminated());
        assert_eq!(stream.next().await, Some(Block::default()));
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
        let transport = web3::transports::Http::new(node).unwrap();
        let web3 = Web3::new(DynTransport::new(transport));
        let mut stream = current_block_stream(web3);
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number.unwrap().as_u64());
        }
    }
}
