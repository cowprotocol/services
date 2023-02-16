//! Legacy HTTP solver adapter implementation.
//!
//! In order to faciliate the transition from the legacy HTTP solver API to the
//! new HTTP API, we provide a solver "wrapper" that just marshals the API
//! request types to and from the legacy format.

use {
    crate::{
        boundary::legacy,
        domain::{auction, eth, solution, Solver},
        infra::config::legacy::LegacyConfig,
    },
    anyhow::Result,
    futures::future::{BoxFuture, FutureExt},
    shared::http_solver::{DefaultHttpSolverApi, HttpSolverApi, SolverConfig},
};

pub struct Legacy {
    solver: DefaultHttpSolverApi,
    weth: eth::WethAddress,
}

impl Legacy {
    pub fn new(config: LegacyConfig) -> Self {
        Self {
            solver: DefaultHttpSolverApi {
                name: config.solver_name,
                network_name: format!("{:?}", config.chain_id),
                chain_id: config.chain_id.value().as_u64(),
                base: config.base_url,
                client: reqwest::Client::new(),
                config: SolverConfig {
                    // Note that we unconditionally set this to "true". This is
                    // because the auction that we are solving for already
                    // contains which tokens can and can't be internalized,
                    // and we don't need to duplicate this setting here. Ergo,
                    // in order to disable internalization, the driver would be
                    // configured to have 0 trusted tokens.
                    use_internal_buffers: Some(true),
                    ..Default::default()
                },
            },
            weth: config.weth,
        }
    }

    async fn solve_(&self, auction: auction::Auction) -> Result<solution::Solution> {
        let (mapping, auction_model) = legacy::to_boundary_auction(&auction, self.weth);
        let solving_time = (auction.deadline - chrono::Utc::now()).to_std()?;
        let solution = self.solver.solve(&auction_model, solving_time).await?;
        legacy::to_domain_solution(&solution, mapping)
    }
}

impl Solver for Legacy {
    fn solve(&self, auction: auction::Auction) -> BoxFuture<Vec<solution::Solution>> {
        async move {
            match self.solve_(auction).await {
                Ok(solution) => vec![solution],
                Err(err) => {
                    tracing::warn!(?err, "failed to solve auction");
                    vec![]
                }
            }
        }
        .boxed()
    }
}
