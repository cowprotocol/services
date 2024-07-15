use crate::{
    domain::eth,
    infra::{blockchain, config::file::GasEstimatorType, liquidity, mempool, simulator, solver},
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
}
