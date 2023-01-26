use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use model::auction::{Auction, AuctionId};
use rand::seq::SliceRandom;
use shared::current_block::CurrentBlockStream;
use std::time::Duration;
use tracing::Instrument;

use crate::{
    database::Postgres,
    driver_api::Driver,
    driver_model::{execute, solve},
    solvable_orders::SolvableOrdersCache,
};

const SOLVE_TIME_LIMIT: Duration = Duration::from_secs(15);

pub struct RunLoop {
    solvable_orders_cache: SolvableOrdersCache,
    database: Postgres,
    drivers: Vec<Driver>,
    current_block: CurrentBlockStream,
}

impl RunLoop {
    pub async fn run_forever(&self) -> ! {
        loop {
            self.single_run().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn single_run(&self) {
        let auction = match self.solvable_orders_cache.current_auction() {
            Some(auction) => auction,
            None => {
                tracing::debug!("no current auction");
                return;
            }
        };
        let id = match self.database.replace_current_auction(&auction).await {
            Ok(id) => id,
            Err(err) => {
                tracing::error!(?err, "failed to replace current auction");
                return;
            }
        };
        self.single_run_(id, &auction)
            .instrument(tracing::info_span!("auction", id))
            .await;
    }

    async fn single_run_(&self, id: AuctionId, auction: &Auction) {
        tracing::info!("solving");
        let mut solutions = self.solve(auction, id).await;

        // Shuffle so that sorting randomly splits ties.
        solutions.shuffle(&mut rand::thread_rng());
        solutions.sort_unstable_by(|left, right| left.1.objective.total_cmp(&right.1.objective));

        // TODO: Keep going with other solutions until some deadline.
        if let Some((index, solution)) = solutions.pop() {
            tracing::info!("executing with solver {}", index);
            match self
                .execute(auction, id, &self.drivers[index], &solution)
                .await
            {
                Ok(()) => tracing::info!("settled"),
                Err(err) => {
                    tracing::error!(?err, "solver {index} failed to execute");
                }
            }
        }

        // TODO:
        // - Think about what per auction information needs to be permanently stored. We might want
        // to store the competition information and the full promised solution of the winner.
    }

    /// Returns the successful /solve responses and the index of the solver.
    async fn solve(&self, auction: &Auction, id: AuctionId) -> Vec<(usize, solve::Response)> {
        let auction = solve::Auction {
            id,
            block: self.current_block.borrow().number,
            orders: auction.orders.iter().map(|_| solve::Order {}).collect(),
            prices: auction.prices.clone(),
        };
        let deadline = Utc::now() + chrono::Duration::from_std(SOLVE_TIME_LIMIT).unwrap();
        let request = &solve::Request { auction, deadline };
        let futures = self
            .drivers
            .iter()
            .enumerate()
            .map(|(index, driver)| async move {
                let result =
                    match tokio::time::timeout(SOLVE_TIME_LIMIT, driver.solve(request)).await {
                        Ok(inner) => inner,
                        Err(_) => Err(anyhow!("timeout")),
                    };
                (index, result)
            })
            .collect::<Vec<_>>();
        let results = futures::future::join_all(futures).await;
        results
            .into_iter()
            .filter_map(|(index, result)| match result {
                Ok(result) => Some((index, result)),
                Err(err) => {
                    tracing::warn!(?err, "driver {} solve error", err);
                    None
                }
            })
            .collect()
    }

    /// Execute the solver's solution. Returns Ok when the corresponding transaction has been mined.
    async fn execute(
        &self,
        _auction: &Auction,
        id: AuctionId,
        driver: &Driver,
        _solution: &solve::Response,
    ) -> Result<()> {
        let request = execute::Request { auction_id: id };
        let _response = driver.execute(&request).await.context("execute")?;
        // TODO: Wait for transaction to be mined or deadline to be reached.
        Ok(())
    }
}
