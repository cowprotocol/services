use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use model::auction::{Auction, AuctionId};
use primitive_types::H256;
use rand::seq::SliceRandom;
use shared::{
    current_block::CurrentBlockStream, ethrpc::Web3, event_handling::MAX_REORG_BLOCK_COUNT,
};
use std::{collections::HashSet, time::Duration};
use tracing::Instrument;
use web3::types::Transaction;

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
    web3: Web3,
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
                Ok(()) => (),
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
        let request = execute::Request {
            auction_id: id,
            transaction_identifier: id.to_be_bytes().into(),
        };
        let _response = driver.execute(&request).await.context("execute")?;
        // TODO: React to deadline expiring.
        let transaction = self
            .wait_for_settlement_transaction(&request.transaction_identifier)
            .await
            .context("wait for settlement transaction")?;
        if let Some(tx) = transaction {
            tracing::debug!("settled in tx {:?}", tx.hash);
        }
        Ok(())
    }

    /// Tries to find a `settle` contract call with calldata ending in `tag`.
    ///
    /// Returns None if no transaction was found within the deadline.
    pub async fn wait_for_settlement_transaction(&self, tag: &[u8]) -> Result<Option<Transaction>> {
        // Start earlier than current block because there might be a delay when receiving the
        // Solver's /execute response during which it already started broadcasting the tx.
        let start_offset = MAX_REORG_BLOCK_COUNT;
        let max_wait_time = 20;
        let current = self.current_block.borrow().number;
        let start = current.saturating_sub(start_offset);
        let deadline = current.saturating_add(max_wait_time);
        tracing::debug!(%current, %start, %deadline, ?tag, "waiting for tag");

        // Use the existing event indexing infrastructure to find the transaction. We query all
        // settlement events in the block range to get tx hashes and query the node for the full
        // calldata.
        //
        // If the block range was large, we would make the query more efficient by moving the
        // starting block up while taking reorgs into account. With the current range of 30 blocks
        // this isn't necessary.
        //
        // We do keep track of hashes we have already seen to reduce load from the node.

        let mut seen_transactions: HashSet<H256> = Default::default();
        loop {
            // This could be a while loop. It isn't, because some care must be taken to not
            // accidentally keep the borrow alive, which would block senders. Technically this is
            // fine with while conditions but this is clearer.
            if self.current_block.borrow().number <= deadline {
                break;
            }
            let mut hashes = self
                .database
                .recent_settlement_tx_hashes(start..deadline + 1)
                .await?;
            hashes.retain(|hash| !seen_transactions.contains(hash));
            for hash in hashes {
                let tx: Option<Transaction> = self
                    .web3
                    .eth()
                    .transaction(web3::types::TransactionId::Hash(hash))
                    .await
                    .with_context(|| format!("web3 transaction {hash:?}"))?;
                let tx: Transaction = match tx {
                    Some(tx) => tx,
                    None => continue,
                };
                if tx.input.0.ends_with(tag) {
                    return Ok(Some(tx));
                }
                seen_transactions.insert(hash);
            }
            // It would be more correct to wait until just after the last event update run, but
            // that is hard to synchronize.
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        Ok(None)
    }
}
