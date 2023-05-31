use {
    self::{
        baseline_solver::BaselineSolver,
        http_solver::HttpSolver,
        naive_solver::NaiveSolver,
        oneinch_solver::OneInchSolver,
        optimizing_solver::OptimizingSolver,
        paraswap_solver::ParaswapSolver,
        single_order_solver::{SingleOrderSolver, SingleOrderSolving},
        zeroex_solver::ZeroExSolver,
    },
    crate::{
        interactions::allowances::AllowanceManager,
        liquidity::{
            order_converter::OrderConverter,
            slippage::{self, SlippageCalculator},
            LimitOrder,
            Liquidity,
        },
        metrics::SolverMetrics,
        s3_instance_upload::S3InstanceUploader,
        settlement::Settlement,
        settlement_post_processing::PostProcessing,
        settlement_rater::SettlementRating,
        solver::{
            balancer_sor_solver::BalancerSorSolver,
            http_solver::{
                buffers::BufferRetriever,
                instance_cache::SharedInstanceCreator,
                instance_creation::InstanceCreator,
                InstanceType,
            },
        },
    },
    anyhow::{anyhow, Context, Result},
    contracts::{BalancerV2Vault, GPv2Settlement, WETH9},
    ethcontract::{errors::ExecutionError, Account, PrivateKey, H160, U256},
    model::{auction::AuctionId, order::Order, DomainSeparator},
    reqwest::Url,
    shared::{
        account_balances,
        balancer_sor_api::DefaultBalancerSorApi,
        baseline_solver::BaseTokens,
        ethrpc::Web3,
        external_prices::ExternalPrices,
        http_client::HttpClientFactory,
        http_solver::{
            model::{AuctionResult, SimulatedTransaction},
            DefaultHttpSolverApi,
            SolverConfig,
        },
        token_info::TokenInfoFetching,
        token_list::AutoUpdatingTokenList,
        zeroex_api::ZeroExApi,
    },
    std::{
        collections::HashMap,
        fmt::{self, Debug, Formatter},
        str::FromStr,
        sync::Arc,
        time::{Duration, Instant},
    },
    web3::types::AccessList,
};

pub mod balancer_sor_solver;
mod baseline_solver;
pub mod http_solver;
pub mod naive_solver;
mod oneinch_solver;
pub mod optimizing_solver;
mod paraswap_solver;
pub mod score_computation;
pub mod single_order_solver;
mod zeroex_solver;

/// Interface that all solvers must implement.
///
/// A `solve` method transforming a collection of `Liquidity` (sources) into a
/// list of independent `Settlements`. Solvers are free to choose which types
/// `Liquidity` they would like to process, including their own private sources.
#[mockall::automock]
#[async_trait::async_trait]
pub trait Solver: Send + Sync + 'static {
    /// Runs the solver.
    ///
    /// The returned settlements should be independent (for example not reusing
    /// the same user order) so that they can be merged by the driver at its
    /// leisure.
    ///
    /// id identifies this instance of solving by the driver in which it invokes
    /// all solvers.
    async fn solve(&self, auction: Auction) -> Result<Vec<Settlement>>;

    /// Callback to notify the solver how it performed in the given auction (if
    /// it won or failed for some reason) Has to be non-blocking to not
    /// delay settling the actual solution
    fn notify_auction_result(&self, _auction_id: AuctionId, _result: AuctionResult) {}

    /// Returns solver's account that should be used to submit settlements.
    fn account(&self) -> &Account;

    /// Returns displayable name of the solver.
    ///
    /// This method is used for logging and metrics collection.
    fn name(&self) -> &str;
}

/// A batch auction for a solver to produce a settlement for.
#[derive(Clone, Debug)]
pub struct Auction {
    /// Note that multiple consecutive driver runs may use the same ID if the
    /// previous run was unable to find a settlement.
    pub id: AuctionId,

    /// An ID that identifies a driver run.
    ///
    /// Note that this ID is not unique across multiple instances of drivers,
    /// in particular it cannot be used to uniquely identify batches across
    /// service restarts.
    pub run: u64,

    /// The GPv2 orders to match.
    pub orders: Vec<Order>,

    /// The baseline on-chain liquidity that can be used by the solvers for
    /// settling orders.
    pub liquidity: Vec<Liquidity>,

    /// On which block the liquidity got fetched.
    pub liquidity_fetch_block: u64,

    /// The current gas price estimate.
    pub gas_price: f64,

    /// The deadline for computing a solution.
    ///
    /// This can be used internally for the solver to decide when to stop
    /// trying to optimize the settlement. The caller is expected poll the solve
    /// future at most until the deadline is reach, at which point the future
    /// will be dropped.
    pub deadline: Instant,

    /// The set of external prices for this auction.
    ///
    /// The objective value is calculated with these prices so they can be
    /// relevant for solvers.
    ///
    /// External prices are garanteed to exist for all orders included in the
    /// current auction.
    pub external_prices: ExternalPrices,

    /// Balances for `orders`. Not guaranteed to have an entry for all orders
    /// because balance fetching can fail.
    pub balances: HashMap<account_balances::Query, U256>,
}

impl Default for Auction {
    fn default() -> Self {
        const SECONDS_IN_A_YEAR: u64 = 31_622_400;

        // Not actually never, but good enough...
        let never = Instant::now() + Duration::from_secs(SECONDS_IN_A_YEAR);
        Self {
            id: Default::default(),
            run: Default::default(),
            orders: Default::default(),
            liquidity: Default::default(),
            liquidity_fetch_block: Default::default(),
            gas_price: Default::default(),
            deadline: never,
            external_prices: Default::default(),
            balances: Default::default(),
        }
    }
}

/// A vector of solvers.
pub type Solvers = Vec<Arc<dyn Solver>>;

/// A single settlement and a solver that produced it.
pub type SettlementWithSolver = (Arc<dyn Solver>, Settlement, Option<AccessList>);

#[derive(Debug, Clone)]
pub struct SolverInfo {
    /// Identifier used for metrics and logging.
    pub name: String,
    /// Address used for simulating settlements of that solver.
    pub account: Account,
}

pub struct Simulation {
    pub settlement: Settlement,
    pub solver: SolverInfo,
    pub transaction: SimulatedTransaction,
}

pub struct SimulationWithError {
    pub simulation: Simulation,
    pub error: SimulationError,
}

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("web3 error: {0:?}")]
    Web3(#[from] ExecutionError),
    #[error("insufficient balance: needs {needs} has {has}")]
    InsufficientBalance { needs: U256, has: U256 },
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum SolverType {
    Naive,
    Baseline,
    CowDexAg,
    OneInch,
    Paraswap,
    ZeroEx,
    Quasimodo,
    BalancerSor,
}

#[derive(Clone)]
pub enum SolverAccountArg {
    PrivateKey(PrivateKey),
    Address(H160),
}

impl Debug for SolverAccountArg {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            SolverAccountArg::PrivateKey(k) => write!(f, "PrivateKey({:?})", k.public_address()),
            SolverAccountArg::Address(a) => write!(f, "Address({a:?})"),
        }
    }
}

impl SolverAccountArg {
    pub fn into_account(self, chain_id: u64) -> Account {
        match self {
            SolverAccountArg::PrivateKey(key) => Account::Offline(key, Some(chain_id)),
            SolverAccountArg::Address(address) => Account::Local(address, None),
        }
    }
}

impl FromStr for SolverAccountArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<PrivateKey>()
            .map(SolverAccountArg::PrivateKey)
            .or_else(|pk_err| {
                Ok(SolverAccountArg::Address(s.parse().map_err(
                    |addr_err| {
                        anyhow!("could not parse as private key: {}", pk_err)
                            .context(anyhow!("could not parse as address: {}", addr_err))
                            .context(
                                "invalid solver account, it is neither a private key or an \
                                 Ethereum address",
                            )
                    },
                )?))
            })
    }
}

#[derive(Clone, Debug)]
pub struct ExternalSolverArg {
    pub name: String,
    pub url: Url,
    pub account: SolverAccountArg,
    pub use_liquidity: bool,
    pub user_balance_support: UserBalanceSupport,
}

/// Whether the solver supports assigning user sell token balance to orders or
/// whether the driver needs to do it instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserBalanceSupport {
    None,
    PartiallyFillable,
    // Will be added later.
    // All,
}

impl FromStr for UserBalanceSupport {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "partially_fillable" => Ok(Self::PartiallyFillable),
            _ => Err(anyhow::anyhow!("unknown variant {}", s)),
        }
    }
}

impl FromStr for ExternalSolverArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('|');
        let name = parts.next().context("missing name")?;
        let url = parts.next().context("missing url")?;
        let account = parts.next().context("missing account")?;
        let use_liquidity = parts.next().context("missing use_liquidity")?;
        // With a default temporarily until we configure the argument in our cluster.
        let user_balance_support = parts.next().unwrap_or("none");
        Ok(Self {
            name: name.to_string(),
            url: url.parse().context("parse url")?,
            account: account.parse().context("parse account")?,
            use_liquidity: use_liquidity.parse().context("parse use_liquidity")?,
            user_balance_support: user_balance_support
                .parse()
                .context("parse user_balance_support")?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    web3: Web3,
    solvers: Vec<(Account, SolverType)>,
    base_tokens: Arc<BaseTokens>,
    native_token: WETH9,
    cow_dex_ag_solver_url: Url,
    quasimodo_solver_url: Url,
    balancer_sor_url: Url,
    settlement_contract: &GPv2Settlement,
    vault_contract: Option<&BalancerV2Vault>,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    network_id: String,
    chain_id: u64,
    disabled_one_inch_protocols: Vec<String>,
    disabled_paraswap_dexs: Vec<String>,
    paraswap_partner: Option<String>,
    http_factory: &HttpClientFactory,
    solver_metrics: Arc<dyn SolverMetrics>,
    zeroex_api: Arc<dyn ZeroExApi>,
    zeroex_disabled_sources: Vec<String>,
    zeroex_enable_rfqt: bool,
    zeroex_enable_slippage_protection: bool,
    use_internal_buffers: bool,
    one_inch_url: Url,
    one_inch_referrer_address: Option<H160>,
    external_solvers: Vec<ExternalSolverArg>,
    order_converter: OrderConverter,
    max_settlements_per_solver: usize,
    max_merged_settlements: usize,
    smallest_partial_fill: U256,
    slippage_configuration: &slippage::Arguments,
    market_makable_token_list: AutoUpdatingTokenList,
    order_prioritization_config: &single_order_solver::Arguments,
    post_processing_pipeline: Arc<dyn PostProcessing>,
    domain: &DomainSeparator,
    s3_instance_uploader: Option<Arc<S3InstanceUploader>>,
    score_configuration: &score_computation::Arguments,
    settlement_rater: Arc<dyn SettlementRating>,
    enforce_correct_fees: bool,
    ethflow_contract: Option<H160>,
) -> Result<Solvers> {
    // Tiny helper function to help out with type inference. Otherwise, all
    // `Box::new(...)` expressions would have to be cast `as Box<dyn Solver>`.
    fn shared(solver: impl Solver + 'static) -> Arc<dyn Solver> {
        Arc::new(solver)
    }

    let buffer_retriever = Arc::new(BufferRetriever::new(
        web3.clone(),
        settlement_contract.address(),
    ));
    let allowance_manager = Arc::new(AllowanceManager::new(
        web3.clone(),
        settlement_contract.address(),
    ));
    let instance_creator = InstanceCreator {
        native_token,
        ethflow_contract,
        token_info_fetcher: token_info_fetcher.clone(),
        buffer_retriever,
        market_makable_token_list: market_makable_token_list.clone(),
        environment_metadata: network_id.clone(),
    };
    let shared_instance_creator = Arc::new(SharedInstanceCreator::new(
        instance_creator,
        s3_instance_uploader,
    ));

    // Helper function to create http solver instances.
    let create_http_solver = |account: Account,
                              url: Url,
                              name: String,
                              config: SolverConfig,
                              instance_type: InstanceType,
                              slippage_calculator: SlippageCalculator,
                              use_liquidity: bool|
     -> HttpSolver {
        HttpSolver::new(
            DefaultHttpSolverApi {
                name,
                network_name: network_id.clone(),
                chain_id,
                base: url,
                solve_path: "solve".to_owned(),
                client: http_factory.create(),
                gzip_requests: false,
                config,
            },
            account,
            allowance_manager.clone(),
            order_converter.clone(),
            instance_type,
            slippage_calculator,
            market_makable_token_list.clone(),
            *domain,
            shared_instance_creator.clone(),
            use_liquidity,
            enforce_correct_fees,
        )
    };

    let mut solvers: Vec<Arc<dyn Solver>> = solvers
        .into_iter()
        .map(|(account, solver_type)| {
            let single_order = |inner: Box<dyn SingleOrderSolving>| {
                SingleOrderSolver::new(
                    inner,
                    solver_metrics.clone(),
                    max_merged_settlements,
                    max_settlements_per_solver,
                    order_prioritization_config.clone(),
                    smallest_partial_fill,
                    settlement_rater.clone(),
                    ethflow_contract,
                    order_converter.clone(),
                )
            };

            let slippage_calculator = slippage_configuration.get_calculator(solver_type);
            tracing::debug!(
                solver = ?solver_type, slippage = ?slippage_calculator,
                "configured slippage",
            );

            let score_calculator = score_configuration.get_calculator(solver_type);

            let solver = match solver_type {
                SolverType::Naive => shared(NaiveSolver::new(
                    account,
                    slippage_calculator,
                    enforce_correct_fees,
                    ethflow_contract,
                    order_converter.clone(),
                )),
                SolverType::Baseline => shared(BaselineSolver::new(
                    account,
                    base_tokens.clone(),
                    slippage_calculator,
                    ethflow_contract,
                    order_converter.clone(),
                )),
                SolverType::CowDexAg => shared(create_http_solver(
                    account,
                    cow_dex_ag_solver_url.clone(),
                    "CowDexAg".to_string(),
                    SolverConfig::default(),
                    InstanceType::Plain,
                    slippage_calculator,
                    false,
                )),
                SolverType::Quasimodo => shared(create_http_solver(
                    account,
                    quasimodo_solver_url.clone(),
                    "Quasimodo".to_string(),
                    SolverConfig {
                        use_internal_buffers: Some(use_internal_buffers),
                        ..Default::default()
                    },
                    InstanceType::Filtered,
                    slippage_calculator,
                    true,
                )),
                SolverType::OneInch => shared(single_order(Box::new(
                    OneInchSolver::with_disabled_protocols(
                        account,
                        web3.clone(),
                        settlement_contract.clone(),
                        chain_id,
                        disabled_one_inch_protocols.clone(),
                        http_factory.create(),
                        one_inch_url.clone(),
                        slippage_calculator,
                        one_inch_referrer_address,
                    )
                    .unwrap(),
                ))),
                SolverType::ZeroEx => {
                    let zeroex_solver = ZeroExSolver::new(
                        account,
                        web3.clone(),
                        settlement_contract.clone(),
                        chain_id,
                        zeroex_api.clone(),
                        zeroex_disabled_sources.clone(),
                        slippage_calculator,
                    )
                    .unwrap()
                    .with_rfqt(zeroex_enable_rfqt)
                    .with_slippage_protection(zeroex_enable_slippage_protection);
                    shared(single_order(Box::new(zeroex_solver)))
                }
                SolverType::Paraswap => shared(single_order(Box::new(ParaswapSolver::new(
                    account,
                    web3.clone(),
                    settlement_contract.clone(),
                    token_info_fetcher.clone(),
                    disabled_paraswap_dexs.clone(),
                    http_factory.create(),
                    paraswap_partner.clone(),
                    slippage_calculator,
                )))),
                SolverType::BalancerSor => shared(single_order(Box::new(BalancerSorSolver::new(
                    account,
                    vault_contract
                        .expect("missing Balancer Vault deployment for SOR solver")
                        .clone(),
                    settlement_contract.clone(),
                    Arc::new(
                        DefaultBalancerSorApi::new(
                            http_factory.create(),
                            balancer_sor_url.clone(),
                            chain_id,
                        )
                        .unwrap(),
                    ),
                    allowance_manager.clone(),
                    slippage_calculator,
                )))),
            };
            shared(OptimizingSolver {
                inner: solver,
                post_processing_pipeline: post_processing_pipeline.clone(),
                score_calculator,
            })
        })
        .collect();

    let external_solvers = external_solvers.into_iter().map(|solver| {
        shared(create_http_solver(
            solver.account.into_account(chain_id),
            solver.url,
            solver.name,
            SolverConfig {
                use_internal_buffers: Some(use_internal_buffers),
                ..Default::default()
            },
            InstanceType::Plain,
            slippage_configuration.get_global_calculator(),
            solver.use_liquidity,
        ))
    });
    solvers.extend(external_solvers);

    for solver in &solvers {
        tracing::info!(
            "initialized solver {} at address {:#x}",
            solver.name(),
            solver.account().address()
        )
    }

    Ok(solvers)
}

#[cfg(test)]
struct DummySolver;
#[cfg(test)]
#[async_trait::async_trait]
impl Solver for DummySolver {
    async fn solve(&self, _: Auction) -> Result<Vec<Settlement>> {
        todo!()
    }

    fn account(&self) -> &ethcontract::Account {
        todo!()
    }

    fn notify_auction_result(&self, _auction_id: AuctionId, _result: AuctionResult) {}

    fn name(&self) -> &'static str {
        "DummySolver"
    }
}
#[cfg(test)]
pub fn dummy_arc_solver() -> Arc<dyn Solver> {
    Arc::new(DummySolver)
}

fn balance_and_convert_orders(
    ethflow_contract: Option<H160>,
    converter: &OrderConverter,
    mut balances: HashMap<account_balances::Query, U256>,
    orders: Vec<Order>,
) -> Vec<LimitOrder> {
    crate::order_balance_filter::balance_orders(orders, &mut balances, ethflow_contract)
        .into_iter()
        .filter_map(|order| match converter.normalize_limit_order(order) {
            Ok(order) => Some(order),
            Err(err) => {
                tracing::debug!(?err, "error normalizing limit order");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dummy solver returning no settlements
    pub struct NoopSolver();
    #[async_trait::async_trait]
    impl Solver for NoopSolver {
        async fn solve(&self, _: Auction) -> Result<Vec<Settlement>> {
            Ok(Vec::new())
        }

        fn notify_auction_result(&self, _auction_id: AuctionId, _result: AuctionResult) {}

        fn account(&self) -> &Account {
            unimplemented!()
        }

        fn name(&self) -> &'static str {
            "NoopSolver"
        }
    }

    impl PartialEq for SolverAccountArg {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (SolverAccountArg::PrivateKey(a), SolverAccountArg::PrivateKey(b)) => {
                    a.public_address() == b.public_address()
                }
                (SolverAccountArg::Address(a), SolverAccountArg::Address(b)) => a == b,
                _ => false,
            }
        }
    }

    #[test]
    fn parses_solver_account_arg() {
        assert_eq!(
            "0x4242424242424242424242424242424242424242424242424242424242424242"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::PrivateKey(PrivateKey::from_raw([0x42; 32]).unwrap())
        );
        assert_eq!(
            "0x4242424242424242424242424242424242424242"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::Address(H160([0x42; 20])),
        );
    }

    #[test]
    fn errors_on_invalid_solver_account_arg() {
        assert!("0x010203040506070809101112131415161718192021"
            .parse::<SolverAccountArg>()
            .is_err());
        assert!("not an account".parse::<SolverAccountArg>().is_err());
    }

    #[test]
    fn parse_external_solver_arg() {
        let arg = "name|http://solver.com/|0x4242424242424242424242424242424242424242424242424242424242424242|true|partially_fillable";
        let parsed = ExternalSolverArg::from_str(arg).unwrap();
        assert_eq!(parsed.name, "name");
        assert_eq!(parsed.url.to_string(), "http://solver.com/");
        assert_eq!(
            parsed.account,
            SolverAccountArg::PrivateKey(PrivateKey::from_raw([0x42; 32]).unwrap())
        );
        assert!(parsed.use_liquidity);
        assert_eq!(
            parsed.user_balance_support,
            UserBalanceSupport::PartiallyFillable
        );
    }

    #[test]
    fn parse_external_solver_arg_user_balance_default() {
        let arg = "name|http://solver.com/|0x4242424242424242424242424242424242424242424242424242424242424242|false";
        let parsed = ExternalSolverArg::from_str(arg).unwrap();
        assert_eq!(parsed.user_balance_support, UserBalanceSupport::None);
    }
}
