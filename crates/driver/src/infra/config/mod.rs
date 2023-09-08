use {
    crate::{
        domain::eth,
        infra::{blockchain, liquidity, mempool, simulator, solver},
    },
    primitive_types::U256,
};

pub mod file;

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub score_cap: U256,
    pub disable_access_list_simulation: bool,
    pub disable_gas_simulation: Option<eth::Gas>,
    pub solvers: Vec<solver::Config>,
    pub liquidity: liquidity::Config,
    pub tenderly: Option<simulator::tenderly::Config>,
    pub mempools: Vec<mempool::Config>,
    pub contracts: blockchain::contracts::Addresses,
}
