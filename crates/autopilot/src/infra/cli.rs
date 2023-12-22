use reqwest::Url;

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// The node RPC API endpoint.
    #[clap(long, env)]
    pub ethrpc: Url,
}
