use crate::domain::eth;
use clap::Args;

/// Baseline solver command line arguments.
#[derive(Args, Debug)]
pub struct Arguments {
    /// The address of the WETH contract.
    #[arg(long, env)]
    pub weth: eth::WethAddress,

    /// List of base tokens to use when path finding. This defines the tokens
    /// that can appear as intermediate "hops" within a trading route. Note that
    /// the address specified with `--weth` is always considered a base token.
    #[arg(long, env, value_delimiter = ',')]
    pub base_tokens: Vec<eth::TokenAddress>,

    /// The maximum number of hops to consider when finding the optimal trading
    /// path.
    #[arg(long, env, default_value = "2")]
    pub max_hops: usize,
}
