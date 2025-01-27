use {
    crate::{
        domain::eth,
        infra::{
            blockchain,
            config::file::{AppDataFetching, GasEstimatorType, OrderPriorityStrategy},
            liquidity,
            mempool,
            simulator,
            solver,
        },
    },
    std::time::Duration,
    url::Url,
};

pub mod file;

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub disable_access_list_simulation: bool,
    pub disable_gas_simulation: Option<eth::Gas>,
    pub solvers: Vec<solver::Config>,
    pub liquidity: liquidity::Config,
    pub simulator: Option<simulator::Config>,
    pub gas_estimator: GasEstimatorType,
    pub mempools: Vec<mempool::Config>,
    pub contracts: blockchain::contracts::Addresses,
    pub order_priority_strategies: Vec<OrderPriorityStrategy>,
    pub archive_node_url: Option<Url>,
    pub simulation_bad_token_max_age: Duration,
    pub flashloans: AppDataFetching,
}
