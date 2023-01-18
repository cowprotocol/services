use crate::infra::{liquidity, solver};

pub mod cli;
pub mod file;

pub struct Config {
    pub solvers: Vec<solver::Config>,
    pub liquidity: liquidity::Config,
}
