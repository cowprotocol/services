use crate::{
    settlement::Settlement,
    settlement_post_processing::PostProcessingPipeline,
    solver::{Auction, Solver},
};
use anyhow::Result;
use ethcontract::Account;
use model::auction::AuctionId;
use shared::http_solver::model::AuctionResult;
use std::sync::Arc;

/// A wrapper for solvers that applies a set of optimizations to all the generated settlements.
pub struct OptimizingSolver {
    pub inner: Arc<dyn Solver>,
    pub post_processing_pipeline: Arc<PostProcessingPipeline>,
}

#[async_trait::async_trait]
impl Solver for OptimizingSolver {
    async fn solve(&self, auction: Auction) -> Result<Vec<Settlement>> {
        let results = self.inner.solve(auction).await?;
        // todo convert gas price
        let optimizations = results.into_iter().map(|settlement| {
            self.post_processing_pipeline.optimize_settlement(
                settlement,
                self.account().clone(),
                Default::default(),
            )
        });
        let optimized = futures::future::join_all(optimizations).await;
        Ok(optimized)
    }

    fn notify_auction_result(&self, auction_id: AuctionId, result: AuctionResult) {
        self.inner.notify_auction_result(auction_id, result)
    }

    fn account(&self) -> &Account {
        self.inner.account()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}
