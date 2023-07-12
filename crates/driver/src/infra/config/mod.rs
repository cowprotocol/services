use {
    self::file::{ContractsConfig, LiquidityConfig},
    crate::{
        domain::eth,
        infra::{mempool, simulator, solver},
    },
};

pub mod file;

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub disable_access_list_simulation: bool,
    pub disable_gas_simulation: Option<eth::Gas>,
    pub solvers: Vec<solver::Config>,
    pub liquidity: LiquidityConfig,
    pub tenderly: Option<simulator::tenderly::Config>,
    pub mempools: Vec<mempool::Config>,
    pub contracts: ContractsConfig,
}
