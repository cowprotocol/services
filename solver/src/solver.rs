use std::{collections::HashSet, fmt::Display};

use crate::{
    liquidity::Liquidity, naive_solver::NaiveSolver, settlement::Settlement,
    uniswap_solver::UniswapSolver,
};
use anyhow::Result;
use ethcontract::H160;
use structopt::clap::arg_enum;

#[async_trait::async_trait]
pub trait Solver: Display {
    async fn solve(&self, orders: Vec<Liquidity>) -> Result<Option<Settlement>>;
}

arg_enum! {
    #[derive(Debug)]
    pub enum SolverType {
        Naive,
        UniswapBaseline,
    }
}

pub fn create(solvers: Vec<SolverType>, base_tokens: HashSet<H160>) -> Vec<Box<dyn Solver>> {
    solvers
        .into_iter()
        .map(|solver_type| match solver_type {
            SolverType::Naive => Box::new(NaiveSolver {}) as Box<dyn Solver>,
            SolverType::UniswapBaseline => Box::new(UniswapSolver::new(base_tokens.clone())),
        })
        .collect()
}
