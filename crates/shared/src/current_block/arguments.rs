//! Global block stream arguments.

use super::{current_block_stream, eth_call, BlockRetrieving, CurrentBlockStream};
use crate::{arguments::duration_from_seconds, ethrpc::Web3};
use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
    time::Duration,
};

/// Command line arguments for creating global block stream.
#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// How often in seconds we poll the node to check if the current block has changed.
    #[clap(
        long,
        env,
        default_value = "5",
        value_parser = duration_from_seconds,
    )]
    pub block_stream_poll_interval_seconds: Duration,

    /// Flag for enabling eth_call based block fetching. This is useful when
    /// conneting to Nethermind nodes where they can return block headers before
    /// the state is available which causes issues updating internal state.
    #[clap(long, env, default_value = "get-block")]
    pub block_stream_retriever_strategy: BlockRetrieverStrategy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum BlockRetrieverStrategy {
    #[default]
    GetBlock,
    EthCall,
}

impl Arguments {
    pub fn retriever(&self, web3: Web3) -> Arc<dyn BlockRetrieving> {
        match self.block_stream_retriever_strategy {
            BlockRetrieverStrategy::GetBlock => Arc::new(web3),
            BlockRetrieverStrategy::EthCall => Arc::new(eth_call::BlockRetriever(web3)),
        }
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
        writeln!(
            f,
            "block_stream_enable_eth_call_block_fetching: {:?}",
            self.block_stream_retriever_strategy
        )?;

        Ok(())
    }
}
