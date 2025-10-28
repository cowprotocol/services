//! Global block stream arguments.

use {
    anyhow::Result,
    clap::Parser,
    ethrpc::{
        Web3,
        block_stream::{BlockRetrieving, CurrentBlockWatcher, current_block_stream},
    },
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
    },
    url::Url,
};

/// Command line arguments for creating global block stream.
#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// WebSocket node URL for real-time block updates via subscriptions.
    /// If not provided, will attempt to use the regular HTTP node URL.
    #[clap(long, env)]
    pub node_ws_url: Option<Url>,
}

impl Arguments {
    pub fn retriever(&self, web3: Web3) -> Arc<dyn BlockRetrieving> {
        Arc::new(web3)
    }

    pub async fn stream(&self, http_rpc: Url) -> Result<CurrentBlockWatcher> {
        let ws_rpc = self.node_ws_url.clone().unwrap_or(http_rpc);
        current_block_stream(ws_rpc).await
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
