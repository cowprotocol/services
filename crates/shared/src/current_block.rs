//! Global block stream arguments.

use {
    anyhow::Result,
    clap::Parser,
    ethrpc::{
        current_block::{BlockRetrieving, CurrentBlockStream},
        Web3,
    },
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
        time::Duration,
    },
    url::Url,
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
        default_value = "5s",
        value_parser = humantime::parse_duration,
    )]
    pub block_stream_poll_interval: Duration,
}

impl Arguments {
    pub fn retriever(&self, web3: Web3) -> Arc<dyn BlockRetrieving> {
        Arc::new(web3)
    }

    pub async fn stream(&self, rpc: Url) -> Result<CurrentBlockStream> {
        CurrentBlockStream::new(rpc, self.block_stream_poll_interval).await
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            block_stream_poll_interval,
        } = self;

        writeln!(
            f,
            "block_stream_poll_interval: {:?}",
            block_stream_poll_interval
        )?;

        Ok(())
    }
}
