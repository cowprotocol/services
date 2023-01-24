use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use model::auction::{Auction, AuctionId};
use shared::current_block::CurrentBlockStream;
use std::time::Duration;

use crate::{
    database::Postgres, driver_api::Driver, driver_model::solve,
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
            match self.single_run().await {
                Ok(()) => tracing::debug!("single run ok"),
                Err(err) => tracing::error!(?err, "single run err"),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn single_run(&self) -> Result<()> {
        let auction = match self.solvable_orders_cache.current_auction() {
            Some(auction) => auction,
            None => {
                tracing::debug!("no current auction");
                return Ok(());
            }
        };
        let id = self
            .database
            .replace_current_auction(&auction)
            .await
            .context("replace_current_auction")?;

        let _responses = self.solve(&auction, id).await;

        // TODO:
        // - find winner
        // - send execute to winner
        // - store per auction info

        Ok(())
    }

    async fn solve(&self, auction: &Auction, id: AuctionId) -> Vec<Result<solve::Response>> {
        let auction = solve::Auction {
            id,
            block: self.current_block.borrow().number,
            orders: auction.orders.iter().map(|_| solve::Order {}).collect(),
            prices: auction.prices.clone(),
        };
        let deadline = Utc::now() + chrono::Duration::from_std(SOLVE_TIME_LIMIT).unwrap();
        let request = solve::Request { auction, deadline };
        let futures = self
            .drivers
            .iter()
            .map(|driver| async {
                match tokio::time::timeout(SOLVE_TIME_LIMIT, driver.solve(&request)).await {
                    Ok(inner) => inner,
                    Err(_) => Err(anyhow!("timeout")),
                }
            })
            .collect::<Vec<_>>();
        futures::future::join_all(futures).await
    }
}
