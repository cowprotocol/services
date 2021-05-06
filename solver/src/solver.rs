use std::{collections::HashSet, sync::Arc, time::Duration};

use crate::{
    baseline_solver::BaselineSolver,
    http_solver::{HttpSolver, SolverConfig},
    liquidity::Liquidity,
    naive_solver::NaiveSolver,
    oneinch_solver::OneInchSolver,
    settlement::Settlement,
};
use anyhow::Result;
use contracts::GPv2Settlement;
use ethcontract::H160;
use reqwest::Url;
use shared::{price_estimate::PriceEstimating, token_info::TokenInfoFetching};
use structopt::clap::arg_enum;

// For solvers that enforce a timeout internally we set their timeout to the global solver timeout
// minus this duration to account for additional delay for example from the network.
const TIMEOUT_SAFETY_BUFFER: Duration = Duration::from_secs(5);

#[async_trait::async_trait]
pub trait Solver {
    // The returned settlements should be independent (for example not reusing the same user
    // order) so that they can be merged by the driver at its leisure.
    async fn solve(&self, orders: Vec<Liquidity>, gas_price: f64) -> Result<Vec<Settlement>>;

    // Displayable name of the solver. Defaults to the type name.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

arg_enum! {
    #[derive(Debug)]
    pub enum SolverType {
        Naive,
        Baseline,
        Mip,
        OneInch,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    solvers: Vec<SolverType>,
    base_tokens: HashSet<H160>,
    native_token: H160,
    mip_solver_url: Url,
    settlement_contract: &GPv2Settlement,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    price_estimator: Arc<dyn PriceEstimating>,
    network_id: String,
    chain_id: u64,
    fee_discount_factor: f64,
    solver_timeout: Duration,
) -> Result<Vec<Box<dyn Solver>>> {
    // Tiny helper function to help out with type inference. Otherwise, all
    // `Box::new(...)` expressions would have to be cast `as Box<dyn Solver>`.
    #[allow(clippy::unnecessary_wraps)]
    fn boxed(solver: impl Solver + 'static) -> Result<Box<dyn Solver>> {
        Ok(Box::new(solver))
    }

    let time_limit = solver_timeout
        .checked_sub(TIMEOUT_SAFETY_BUFFER)
        .expect("solver_timeout too low");

    solvers
        .into_iter()
        .map(|solver_type| match solver_type {
            SolverType::Naive => boxed(NaiveSolver {}),
            SolverType::Baseline => boxed(BaselineSolver::new(base_tokens.clone())),
            SolverType::Mip => boxed(HttpSolver::new(
                mip_solver_url.clone(),
                None,
                SolverConfig {
                    max_nr_exec_orders: 100,
                    time_limit: time_limit.as_secs() as u32,
                },
                native_token,
                token_info_fetcher.clone(),
                price_estimator.clone(),
                network_id.clone(),
                fee_discount_factor,
            )),
            SolverType::OneInch => {
                boxed(OneInchSolver::new(settlement_contract.clone(), chain_id)?)
            }
        })
        .collect()
}
