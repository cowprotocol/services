use {
    crate::infra::{
        blockchain::{self, gas::GasPriceParameters},
        builders,
        config::file::{AppDataFetching, GasEstimatorType, OrderPriorityStrategy},
        liquidity,
        mempool,
        notify,
        solver,
    },
    eth_domain_types as eth,
};

pub mod file;

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub disable_access_list_simulation: bool,
    pub disable_gas_simulation: Option<eth::Gas>,
    pub solvers: Vec<solver::Config>,
    pub liquidity: liquidity::Config,
    pub liquidity_sources_notifier: Option<notify::liquidity_sources::Config>,
    pub simulator: configs::simulator::Config,
    pub gas_estimator: GasEstimatorType,
    pub gas_price_parameters: GasPriceParameters,
    pub receiver: Receiver,
    pub contracts: blockchain::contracts::Addresses,
    pub order_priority_strategies: Vec<OrderPriorityStrategy>,
    pub app_data_fetching: AppDataFetching,
    pub tx_gas_limit: eth::U256,
    pub http: configs::http_client::HttpClient,
    pub balance_cache: configs::balance_cache::BalanceCacheConfig,
}

/// Where settlements get submitted to.
#[derive(Debug)]
pub enum Receiver {
    /// Nodes or a private relay, as plain transactions.
    Mempools(Vec<mempool::Config>),
    /// Block builders, as bundles.
    Builders(Vec<builders::Config>),
}
