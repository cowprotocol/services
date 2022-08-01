use crate::{
    settlement_access_list::AccessListEstimatorType,
    solver::{ExternalSolverArg, SolverAccountArg, SolverType},
};
use primitive_types::H160;
use reqwest::Url;
use shared::arguments::{display_list, display_option};
use std::time::Duration;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    /// The API endpoint to fetch the orderbook
    #[clap(long, env, default_value = "http://localhost:8080")]
    pub orderbook_url: Url,

    /// The API endpoint to call the mip solver
    #[clap(long, env, default_value = "http://localhost:8000")]
    pub mip_solver_url: Url,

    /// The API endpoint to call the mip v2 solver
    #[clap(long, env, default_value = "http://localhost:8000")]
    pub quasimodo_solver_url: Url,

    /// The API endpoint to call the cow-dex-ag-solver solver
    #[clap(long, env, default_value = "http://localhost:8000")]
    pub cow_dex_ag_solver_url: Url,

    /// The API endpoint for the Balancer SOR API for solving.
    #[clap(long, env, default_value = "http://localhost:8000")]
    pub balancer_sor_url: Url,

    /// The account used by the driver to sign transactions. This can be either
    /// a 32-byte private key for offline signing, or a 20-byte Ethereum address
    /// for signing with a local node account.
    #[clap(long, env, hide_env_values = true)]
    pub solver_account: Option<SolverAccountArg>,

    /// The target confirmation time in seconds for settlement transactions used to estimate gas price.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub target_confirm_time: Duration,

    /// Specify the interval in seconds between consecutive driver run loops.
    ///
    /// This is typically a low value to prevent busy looping in case of some
    /// internal driver error, but can be set to a larger value for running
    /// drivers in dry-run mode to prevent repeatedly settling the same
    /// orders.
    #[clap(
        long,
        env,
        default_value = "1",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub settle_interval: Duration,

    /// Which type of solver to use
    #[clap(
        long,
        env,
        default_values = &["Naive", "Baseline"],
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub solvers: Vec<SolverType>,

    /// Individual accounts for each solver. See `--solver-account` for more
    /// information about configuring accounts.
    #[clap(
        long,
        env,
        ignore_case = true,
        use_value_delimiter = true,
        hide_env_values = true
    )]
    pub solver_accounts: Option<Vec<SolverAccountArg>>,

    /// List of external solvers in the form of `name|url|account`.
    #[clap(long, env, use_value_delimiter = true)]
    pub external_solvers: Option<Vec<ExternalSolverArg>>,

    /// A settlement must contain at least one order older than this duration in seconds for it
    /// to be applied.  Larger values delay individual settlements more but have a higher
    /// coincidence of wants chance.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub min_order_age: Duration,

    /// The port at which we serve our metrics
    #[clap(long, env, default_value = "9587")]
    pub metrics_port: u16,

    /// The port at which we serve our metrics
    #[clap(long, env, default_value = "5")]
    pub max_merged_settlements: usize,

    /// The maximum amount of time in seconds a solver is allowed to take.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub solver_time_limit: Duration,

    /// The list of tokens our settlement contract is willing to buy when settling trades
    /// without external liquidity
    #[clap(
        long,
        env,
        default_value = "https://tokens.coingecko.com/uniswap/all.json"
    )]
    pub market_makable_token_list: String,

    /// The maximum gas price in Gwei the solver is willing to pay in a settlement.
    #[clap(
        long,
        env,
        default_value = "1500",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub gas_price_cap: f64,

    /// The slippage tolerance we apply to the price quoted by Paraswap
    #[clap(long, env, default_value = "10")]
    pub paraswap_slippage_bps: u32,

    /// The slippage tolerance we apply to the price quoted by zeroEx
    #[clap(long, env, default_value = "10")]
    pub zeroex_slippage_bps: u32,

    /// The default slippage tolerance we apply to the price quoted by OneInchSolver
    #[clap(long, env, default_value = "10")]
    pub oneinch_slippage_bps: u32,

    /// The maximum slippage in ETH we are willing to incur per trade on 1Inch
    #[clap(long, env)]
    pub oneinch_max_slippage_in_eth: Option<f64>,

    /// How to to submit settlement transactions.
    /// Expected to contain either:
    /// 1. One value equal to TransactionStrategyArg::DryRun or
    /// 2. One or more values equal to any combination of enum variants except TransactionStrategyArg::DryRun
    #[clap(
        long,
        env,
        default_value = "PublicMempool",
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub transaction_strategy: Vec<TransactionStrategyArg>,

    /// Which access list estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators might support different networks.
    /// `Tenderly`: supports every network.
    /// `Web3`: supports every network.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub access_list_estimators: Vec<AccessListEstimatorType>,

    /// The URL for tenderly transaction simulation.
    #[clap(long, env)]
    pub tenderly_url: Option<Url>,

    /// Tenderly requires api key to work. Optional since Tenderly could be skipped in access lists estimators.
    #[clap(long, env)]
    pub tenderly_api_key: Option<String>,

    /// The API endpoint of the Eden network for transaction submission.
    #[clap(long, env, default_value = "https://api.edennetwork.io/v1/rpc")]
    pub eden_api_url: Url,

    /// The API endpoint of the Flashbots network for transaction submission.
    /// Multiple values could be defined for different Flashbots endpoints (Flashbots Protect and Flashbots fast).
    #[clap(
        long,
        env,
        use_value_delimiter = true,
        default_value = "https://rpc.flashbots.net"
    )]
    pub flashbots_api_url: Vec<Url>,

    /// Maximum additional tip in gwei that we are willing to give to eden above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "3",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub max_additional_eden_tip: f64,

    /// The maximum time in seconds we spend trying to settle a transaction through the ethereum
    /// network before going to back to solving.
    #[clap(
        long,
        default_value = "120",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub max_submission_seconds: Duration,

    /// Maximum additional tip in gwei that we are willing to give to flashbots above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "3",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub max_additional_flashbot_tip: f64,

    /// Amount of time to wait before retrying to submit the tx to the ethereum network
    #[clap(
        long,
        default_value = "2",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub submission_retry_interval_seconds: Duration,

    /// Additional tip in percentage of max_fee_per_gas we are willing to give to miners above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "0.05",
        parse(try_from_str = shared::arguments::parse_percentage_factor)
    )]
    pub additional_tip_percentage: f64,

    /// The RPC endpoints to use for submitting transaction to a custom set of nodes.
    #[clap(long, env, use_value_delimiter = true)]
    pub transaction_submission_nodes: Vec<Url>,

    /// Fee scaling factor for objective value. This controls the constant
    /// factor by which order fees are multiplied with. Setting this to a value
    /// greater than 1.0 makes settlements with negative objective values less
    /// likely, promoting more aggressive merging of single order settlements.
    #[clap(long, env, default_value = "1", parse(try_from_str = shared::arguments::parse_unbounded_factor))]
    pub fee_objective_scaling_factor: f64,

    /// The maximum number of settlements the driver considers per solver.
    #[clap(long, env, default_value = "20")]
    pub max_settlements_per_solver: usize,

    /// Factor how much of the WETH buffer should be unwrapped if ETH buffer is not big enough to
    /// settle ETH buy orders.
    /// Unwrapping a bigger amount will cause fewer unwraps to happen and thereby reduce the cost
    /// of unwraps per settled batch.
    /// Only values in the range [0.0, 1.0] make sense.
    #[clap(long, env, default_value = "0.6", parse(try_from_str = shared::arguments::parse_percentage_factor))]
    pub weth_unwrap_factor: f64,

    /// Gas limit for simulations. This parameter is important to set correctly, such that
    /// there are no simulation errors due to: err: insufficient funds for gas * price + value,
    /// but at the same time we don't restrict solutions sizes too much
    #[clap(long, env, default_value = "15000000")]
    pub simulation_gas_limit: u128,

    /// In order to protect against malicious solvers, the driver will check that settlements prices do not
    /// exceed a max price deviation compared to the external prices of the driver, if this optional value is set.
    /// The max deviation value should be provided as a float percentage value. E.g. for a max price deviation
    /// of 3%, one should set it to 0.03f64
    #[clap(long, env)]
    pub max_settlement_price_deviation: Option<f64>,

    /// This variable allows to restrict the set of tokens for which a price deviation check of settlement
    /// prices and external prices is executed. If the value is not set, then all tokens included
    /// in the settlement are checked for price deviation.
    #[clap(long, env, use_value_delimiter = true)]
    pub token_list_restriction_for_price_checks: Option<Vec<H160>>,

    /// If liquidity pool fetcher has caching mechanism, this argument defines how old pool data is allowed
    /// to be before updating
    #[clap(
        long,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub liquidity_fetcher_max_age_update: Duration,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.shared)?;
        writeln!(f, "orderbook_url: {}", self.orderbook_url)?;
        writeln!(f, "mip_solver_url: {}", self.mip_solver_url)?;
        writeln!(f, "quasimodo_solver_url: {}", self.quasimodo_solver_url)?;
        writeln!(f, "cow_dex_ag_solver_url: {}", self.cow_dex_ag_solver_url)?;
        writeln!(f, "balancer_sor_url: {}", self.balancer_sor_url)?;
        writeln!(f, "solver_account: {:?}", self.solver_account)?;
        writeln!(f, "target_confirm_time: {:?}", self.target_confirm_time)?;
        writeln!(f, "settle_interval: {:?}", self.settle_interval)?;
        writeln!(f, "solvers: {:?}", self.solvers)?;
        writeln!(f, "solver_accounts: {:?}", self.solver_accounts)?;
        writeln!(f, "external_solvers: {:?}", self.external_solvers)?;
        writeln!(f, "min_order_age: {:?}", self.min_order_age)?;
        writeln!(f, "metrics_port: {}", self.metrics_port)?;
        writeln!(f, "max_merged_settlements: {}", self.max_merged_settlements)?;
        writeln!(f, "solver_time_limit: {:?}", self.solver_time_limit)?;
        writeln!(
            f,
            "market_makable_token_list: {}",
            self.market_makable_token_list
        )?;
        writeln!(f, "gas_price_cap: {}", self.gas_price_cap)?;
        writeln!(f, "paraswap_slippage_bps: {}", self.paraswap_slippage_bps)?;
        writeln!(f, "zeroex_slippage_bps: {}", self.zeroex_slippage_bps)?;
        writeln!(f, "oneinch_slippage_bps: {}", self.oneinch_slippage_bps)?;
        writeln!(f, "transaction_strategy: {:?}", self.transaction_strategy)?;
        writeln!(
            f,
            "access_list_estimators: {:?}",
            self.access_list_estimators
        )?;
        write!(f, "tenderly_url: ")?;
        display_option(&self.tenderly_url, f)?;
        writeln!(f)?;
        writeln!(
            f,
            "tenderly_api_key: {}",
            self.tenderly_api_key
                .as_deref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        writeln!(f, "eden_api_url: {}", self.eden_api_url)?;
        write!(f, "flashbots_api_url: ")?;
        display_list(self.flashbots_api_url.iter(), f)?;
        writeln!(f)?;
        writeln!(
            f,
            "max_additional_eden_tip: {}",
            self.max_additional_eden_tip
        )?;
        writeln!(
            f,
            "max_submission_seconds: {:?}",
            self.max_submission_seconds
        )?;
        writeln!(
            f,
            "max_additional_flashbots_tip: {}",
            self.max_additional_flashbot_tip
        )?;
        writeln!(
            f,
            "submission_retry_interval_seconds: {:?}",
            self.submission_retry_interval_seconds
        )?;
        writeln!(
            f,
            "additional_tip_percentage: {}",
            self.additional_tip_percentage
        )?;
        write!(f, "transaction_submission_nodes: ",)?;
        display_list(self.transaction_submission_nodes.iter(), f)?;
        writeln!(f)?;
        writeln!(
            f,
            "fee_objective_scaling_factor: {}",
            self.fee_objective_scaling_factor
        )?;
        writeln!(
            f,
            "max_settlements_per_solver: {}",
            self.max_settlements_per_solver
        )?;
        writeln!(f, "weth_unwrap_factor: {}", self.weth_unwrap_factor)?;
        writeln!(f, "simulation_gas_limit: {}", self.simulation_gas_limit)?;
        write!(f, "max_settlement_price_deviation: ")?;
        display_option(&self.max_settlement_price_deviation, f)?;
        writeln!(f)?;
        writeln!(
            f,
            "token_list_restriction_for_price_checks: {:?}",
            self.token_list_restriction_for_price_checks
        )?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, clap::ArgEnum)]
#[clap(rename_all = "verbatim")]
pub enum TransactionStrategyArg {
    PublicMempool,
    Eden,
    Flashbots,
    CustomNodes,
    DryRun,
}
