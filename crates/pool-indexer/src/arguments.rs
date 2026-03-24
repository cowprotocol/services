use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Run the indexer and API server.
    Run {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// Seed the database from the Uniswap V3 subgraph, then exit.
    Seed {
        #[clap(long, env)]
        config: PathBuf,
        /// Subgraph GraphQL endpoint URL.
        #[clap(long)]
        subgraph_url: String,
        /// Block number to seed at. Defaults to the subgraph's current indexed
        /// block.
        #[clap(long)]
        block: Option<u64>,
    },
}
