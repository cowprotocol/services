use std::collections::HashSet;

use crate::{
    liquidity::Liquidity, naive_solver::NaiveSolver, settlement::Settlement,
    uniswap_solver::UniswapSolver,
};
use anyhow::Result;
use ethcontract::H160;
use structopt::clap::arg_enum;

#[async_trait::async_trait]
pub trait Solver {
    async fn solve(&self, orders: Vec<Liquidity>) -> Result<Option<Settlement>>;
}

arg_enum! {
    #[derive(Debug)]
    pub enum SolverType {
        Naive,
        UniswapBaseline,
    }
}

pub fn create(solver_type: SolverType, base_tokens: HashSet<H160>) -> Box<dyn Solver> {
    match solver_type {
        SolverType::Naive => Box::new(NaiveSolver {}),
        SolverType::UniswapBaseline => Box::new(UniswapSolver::new(base_tokens)),
    }
}
