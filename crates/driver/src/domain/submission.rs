use {
    crate::infra::Ethereum,
    ethrpc::block_stream::{BlockInfo, into_stream},
    futures::StreamExt,
    tokio_stream::wrappers::WatchStream,
};

/// The blocks after the current one: the ones that can include a tx sent now.
pub(crate) async fn next_blocks(ethereum: &Ethereum) -> WatchStream<BlockInfo> {
    let mut blocks = into_stream(ethereum.current_block().clone());
    blocks.next().await;
    blocks
}
