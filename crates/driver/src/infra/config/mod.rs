use {
    crate::{
        domain::eth,
        infra::{
            blockchain,
            config::file::{AppDataFetching, GasEstimatorType, OrderPriorityStrategy},
            liquidity,
            mempool,
            notify,
            pod,
            simulator,
            solver,
        },
    },
    std::time::Duration,
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
    pub simulator: Option<simulator::Config>,
    pub gas_estimator: GasEstimatorType,
    pub mempools: Vec<mempool::Config>,
    pub contracts: blockchain::contracts::Addresses,
    pub order_priority_strategies: Vec<OrderPriorityStrategy>,
    pub simulation_bad_token_max_age: Duration,
    pub app_data_fetching: AppDataFetching,
    pub tx_gas_limit: eth::U256,
    pub pod: Option<pod::config::Config>,
}
