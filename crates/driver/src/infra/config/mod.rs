use {
    self::file::ContractsConfig,
    crate::{
        domain::eth,
        infra::{liquidity, mempool, simulator, solver},
    },
};

pub mod file;

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub disable_access_list_simulation: bool,
    pub disable_gas_simulation: Option<eth::Gas>,
    pub solvers: Vec<solver::Config>,
    pub liquidity: liquidity::Config,
    pub tenderly: Option<simulator::tenderly::Config>,
    pub mempools: Vec<mempool::Config>,
    pub contracts: ContractsConfig,
}
