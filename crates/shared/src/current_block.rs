//! Global block stream arguments.

#[allow(deprecated)]
use ethrpc::block_stream::current_block_stream;
use {
    anyhow::Result,
    clap::Parser,
    ethrpc::{
        AlloyProvider,
        block_stream::{CurrentBlockWatcher, current_block_ws_stream},
    },
    std::{
        fmt::{self, Display, Formatter},
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
        value_parser = humantime::parse_duration,
    )]
    pub block_stream_poll_interval: Option<Duration>,

    /// WebSocket node URL for real-time block updates via subscriptions.
    /// Fallbacks to the legacy HTTP polling.
    #[clap(long, env)]
    pub node_ws_url: Option<Url>,
}

impl Arguments {
    /// The default poll interval for the block stream updating task.
    const BLOCK_POLL_INTERVAL: Duration = Duration::from_millis(500);

    pub async fn stream(
        &self,
        http_url: Url,
        alloy_provider: AlloyProvider,
    ) -> Result<CurrentBlockWatcher> {
        match &self.node_ws_url {
            Some(ws_url) => current_block_ws_stream(alloy_provider, ws_url.clone()).await,
            #[allow(deprecated)]
            None => {
                current_block_stream(
                    http_url,
                    self.block_stream_poll_interval
                        .unwrap_or(Self::BLOCK_POLL_INTERVAL),
                )
                .await
            }
        }
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            block_stream_poll_interval,
            node_ws_url: ws_node_url,
        } = self;

        writeln!(
            f,
            "block_stream_poll_interval: {block_stream_poll_interval:?}"
        )?;
        writeln!(f, "node_ws_url: {ws_node_url:?}")?;

        Ok(())
    }
}
