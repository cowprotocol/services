use {
    crate::{domain::eth, util::serialize},
    reqwest::Url,
    serde::Deserialize,
    serde_with::serde_as,
};

mod load;

pub use load::load;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Config {
    /// Disable access list simulation, useful for environments that don't
    /// support this, such as less popular blockchains.
    #[serde(default)]
    disable_access_list_simulation: bool,

    /// Parameters related to settlement submission.
    #[serde(default)]
    submission: SubmissionConfig,

    /// Override smart contract addresses.
    #[serde(default)]
    contracts: ContractsConfig,

    /// Use Tenderly for transaction simulation.
    tenderly: Option<TenderlyConfig>,

    #[serde(rename = "solver")]
    solvers: Vec<SolverConfig>,

    #[serde(default)]
    liquidity: LiquidityConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SubmissionConfig {
    /// Additional tip in percentage of max_fee_per_gas we are willing to give
    /// to miners above regular gas price estimation. Expects a floating point
    /// value between 0 and 1.
    #[serde(default = "default_additional_tip_percentage")]
    pub additional_tip_percentage: f64,

    /// The maximum gas price in Gwei the solver is willing to pay in a
    /// settlement.
    #[serde(default = "default_gas_price_cap")]
    pub gas_price_cap: f64,

    /// The target confirmation time for settlement transactions used
    /// to estimate gas price. Specified in seconds.
    #[serde(default = "default_target_confirm_time_secs")]
    pub target_confirm_time_secs: u64,

    /// Amount of time to wait before retrying to submit the tx to
    /// the ethereum network. Specified in seconds.
    #[serde(default = "default_retry_interval_secs")]
    pub retry_interval_secs: u64,

    /// The maximum time to spend trying to settle a transaction through the
    /// Ethereum network before giving up. Specified in seconds.
    #[serde(default = "default_max_confirm_time_secs")]
    pub max_confirm_time_secs: u64,

    /// The mempools to submit settlement transactions to. Can be the public
    /// mempool of a node or the private Flashbots mempool.
    #[serde(rename = "mempool", default)]
    pub mempools: Vec<Mempool>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "mempool")]
#[serde(rename_all = "kebab-case")]
enum Mempool {
    Public {
        /// Don't submit transactions with high revert risk (i.e. transactions
        /// that interact with on-chain AMMs) to the public mempool.
        /// This can be enabled to avoid MEV when private transaction
        /// submission strategies are available.
        #[serde(default)]
        disable_high_risk_public_mempool_transactions: bool,
    },
    Flashbots {
        /// The Flashbots URL to use.
        url: Url,
        /// Maximum additional tip in Gwei that we are willing to give to
        /// Flashbots above regular gas price estimation.
        #[serde(default = "default_max_additional_flashbots_tip")]
        max_additional_tip: f64,
    },
}

fn default_additional_tip_percentage() -> f64 {
    0.05
}

fn default_gas_price_cap() -> f64 {
    1500.0
}

fn default_target_confirm_time_secs() -> u64 {
    30
}

fn default_retry_interval_secs() -> u64 {
    2
}

fn default_max_confirm_time_secs() -> u64 {
    120
}

fn default_max_additional_flashbots_tip() -> f64 {
    3.0
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SolverConfig {
    /// The endpoint of this solver. `POST`ing an auction to this endpoint
    /// should prompt the solver to calculate and return a solution.
    endpoint: url::Url,

    /// The unique name for this solver. Used to disambiguate multiple solvers
    /// running behind a single driver.
    name: String,

    /// The relative slippage allowed by the solver.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: bigdecimal::BigDecimal,

    /// The absolute slippage allowed by the solver.
    #[serde_as(as = "Option<serialize::U256>")]
    absolute_slippage: Option<eth::U256>,

    /// The private key used to sign transactions. Expects a 32-byte hex encoded
    /// string.
    private_key: eth::H256,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContractsConfig {
    /// Override the default address of the GPv2Settlement contract.
    pub gp_v2_settlement: Option<eth::H160>,

    /// Override the default address of the WETH contract.
    pub weth: Option<eth::H160>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct TenderlyConfig {
    /// Optionally override the Tenderly API URL.
    url: Option<Url>,

    /// Authentication key for the Tenderly API.
    api_key: String,

    /// The Tenderly user associated with the API key.
    user: String,

    /// The Tenderly project associated with the API key.
    project: String,

    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    save: bool,

    /// Save the transaction even in the case of failure.
    save_if_fails: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LiquidityConfig {
    /// Additional tokens for which liquidity is always fetched, regardless of
    /// whether or not the token appears in the auction.
    #[serde(default)]
    base_tokens: Vec<eth::H160>,

    /// Liquidity provided by a Uniswap V2 compatible contract.
    #[serde(default)]
    uniswap_v2: Vec<UniswapV2Config>,
}

// TODO it would be nice to provide presets so that you can write:
// ```
// [[liquidity.uniswap-v2]]
// preset = "uniswap"
//
// [[liquidity.uniswap-v2]]
// preset = "sushiswap"
// ```
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct UniswapV2Config {
    /// The address of the Uniswap V2 compatible router contract.
    router: eth::H160,

    /// The digest of the pool initialization code.
    pool_code: eth::H256,
}
