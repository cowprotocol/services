use std::{collections::HashSet, sync::Arc};

use crate::{
    baseline_solver::BaselineSolver,
    http_solver::{HttpSolver, SolverConfig},
    liquidity::Liquidity,
    naive_solver::NaiveSolver,
    settlement::Settlement,
};
use anyhow::Result;
use ethcontract::H160;
use reqwest::Url;
use shared::{price_estimate::PriceEstimating, token_info::TokenInfoFetching};
use structopt::clap::arg_enum;

#[async_trait::async_trait]
pub trait Solver {
    // The returned settlements should be independent (for example not reusing the same user
    // order) so that they can be merged by the driver at its leisure.
    async fn solve(&self, orders: Vec<Liquidity>, gas_price: f64) -> Result<Vec<Settlement>>;

    // Displayable name of the solver.
    fn name(&self) -> &'static str;
}

arg_enum! {
    #[derive(Debug)]
    pub enum AmmSources {
        Uniswap,
        Sushiswap,
    }
}

arg_enum! {
    #[derive(Debug)]
    pub enum SolverType {
        Naive,
        UniswapBaseline,
        Mip,
    }
}

pub fn create(
    solvers: Vec<SolverType>,
    base_tokens: HashSet<H160>,
    native_token: H160,
    mip_solver_url: Url,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    price_estimator: Arc<dyn PriceEstimating>,
) -> Vec<Box<dyn Solver>> {
    solvers
        .into_iter()
        .map(|solver_type| match solver_type {
            SolverType::Naive => Box::new(NaiveSolver {}) as Box<dyn Solver>,
            SolverType::UniswapBaseline => Box::new(BaselineSolver::new(base_tokens.clone())),
            SolverType::Mip => Box::new(HttpSolver::new(
                mip_solver_url.clone(),
                None,
                SolverConfig {
                    max_nr_exec_orders: 100,
                    time_limit: 30,
                },
                native_token,
                token_info_fetcher.clone(),
                price_estimator.clone(),
            )),
        })
        .collect()
}
