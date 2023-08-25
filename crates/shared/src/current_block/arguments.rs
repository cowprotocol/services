//! Global block stream arguments.

use {
    super::{current_block_stream, get_block_and_call, BlockRetrieving, CurrentBlockStream},
    crate::arguments::duration_from_seconds,
    anyhow::Result,
    clap::Parser,
    ethrpc::Web3,
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
        time::Duration,
    },
};

/// Command line arguments for creating global block stream.
#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// How often in seconds we poll the node to check if the current block has
    /// changed.
    #[clap(
        long,
        env,
        default_value = "5",
        value_parser = duration_from_seconds,
    )]
    pub block_stream_poll_interval_seconds: Duration,
}

impl Arguments {
    pub fn retriever(&self, web3: Web3) -> Arc<dyn BlockRetrieving> {
        Arc::new(get_block_and_call::BlockRetriever(web3))
    }

    pub async fn stream(&self, web3: Web3) -> Result<CurrentBlockStream> {
        current_block_stream(
            self.retriever(web3),
            self.block_stream_poll_interval_seconds,
        )
        .await
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(
            f,
            "block_stream_poll_interval_seconds: {:?}",
            self.block_stream_poll_interval_seconds
        )?;

        Ok(())
    }
}
