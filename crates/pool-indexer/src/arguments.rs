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
        /// Subgraph GraphQL endpoint. If provided, seeds the DB from the
        /// subgraph before starting the event indexer.
        #[clap(long)]
        subgraph_url: Option<String>,
        /// Block number to seed at (default: subgraph's current block).
        #[clap(long)]
        seed_block: Option<u64>,
    },
}
