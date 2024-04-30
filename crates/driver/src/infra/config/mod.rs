use crate::{
    domain::eth,
    infra::{blockchain, liquidity, mempool, simulator, solver},
};

pub mod file;
pub use file::encoding;

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub disable_access_list_simulation: bool,
    pub disable_gas_simulation: Option<eth::Gas>,
    pub solvers: Vec<solver::Config>,
    pub liquidity: liquidity::Config,
    pub simulator: Option<simulator::Config>,
    pub mempools: Vec<mempool::Config>,
    pub contracts: blockchain::contracts::Addresses,
    pub encoding: encoding::Strategy,
}
