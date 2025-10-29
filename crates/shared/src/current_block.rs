//! Global block stream arguments.

use {
    anyhow::Result,
    clap::Parser,
    ethrpc::{
        AlloyProvider,
        block_stream::{CurrentBlockWatcher, current_block_stream},
    },
    std::fmt::{self, Display, Formatter},
    url::Url,
};

/// Command line arguments for creating global block stream.
#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// WebSocket node URL for real-time block updates via subscriptions.
    #[clap(long, env, default_value = "ws://localhost:8545")]
    pub node_ws_url: Url,
}

impl Arguments {
    pub async fn stream(&self, alloy_provider: AlloyProvider) -> Result<CurrentBlockWatcher> {
        current_block_stream(alloy_provider, self.node_ws_url.clone()).await
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            node_ws_url: ws_node_url,
        } = self;

        writeln!(f, "node_ws_url: {ws_node_url:?}")?;

        Ok(())
    }
}
