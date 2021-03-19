use std::{collections::HashSet, fmt::Display};

use crate::{
    http_solver::{HttpSolver, SolverConfig},
    liquidity::Liquidity,
    naive_solver::NaiveSolver,
    settlement::Settlement,
    uniswap_solver::UniswapSolver,
};
use anyhow::Result;
use ethcontract::H160;
use reqwest::Url;
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
        MIP,
    }
}

pub fn create(
    solvers: Vec<SolverType>,
    base_tokens: HashSet<H160>,
    native_token: H160,
    mip_solver_url: Url,
) -> Vec<Box<dyn Solver>> {
    solvers
        .into_iter()
        .map(|solver_type| match solver_type {
            SolverType::Naive => Box::new(NaiveSolver {}) as Box<dyn Solver>,
            SolverType::UniswapBaseline => Box::new(UniswapSolver::new(base_tokens.clone())),
            SolverType::MIP => Box::new(HttpSolver::new(
                mip_solver_url.clone(),
                None,
                SolverConfig {
                    max_nr_exec_orders: 100,
                    time_limit: 30,
                },
                native_token,
            )),
        })
        .collect()
}
