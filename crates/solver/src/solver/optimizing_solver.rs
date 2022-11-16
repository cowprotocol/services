use crate::{
    settlement::Settlement,
    settlement_post_processing::PostProcessing,
    solver::{Auction, Solver},
};
use anyhow::Result;
use ethcontract::Account;
use model::auction::AuctionId;
use shared::http_solver::model::AuctionResult;
use std::sync::Arc;
use gas_estimation::GasPrice1559;

/// A wrapper for solvers that applies a set of optimizations to all the generated settlements.
pub struct OptimizingSolver {
    pub inner: Arc<dyn Solver>,
    pub post_processing_pipeline: Arc<dyn PostProcessing>,
}

#[async_trait::async_trait]
impl Solver for OptimizingSolver {
    async fn solve(&self, auction: Auction) -> Result<Vec<Settlement>> {
        let gas_price = GasPrice1559 {
            base_fee_per_gas: auction.gas_price,
            max_fee_per_gas: auction.gas_price,
            max_priority_fee_per_gas: 0.
        };
        let results = self.inner.solve(auction).await?;
        let optimizations = results.into_iter().map(|settlement| {
            self.post_processing_pipeline.optimize_settlement(
                settlement,
                self.account().clone(),
                gas_price
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interactions::UnwrapWethInteraction, settlement_post_processing::MockPostProcessing,
        solver::MockSolver,
    };
    use contracts::WETH9;
    use futures::FutureExt;
    use primitive_types::H160;
    use shared::dummy_contract;
    use hex_literal::hex;
    use ethcontract::PrivateKey;

    #[tokio::test]
    async fn optimizes_solutions() {
        const PRIVATE_KEY: [u8; 32] =
            hex!("0000000000000000000000000000000000000000000000000000000000000001");
        let account = Account::Offline(PrivateKey::from_raw([0x1;32]).unwrap(), None);

        let mut inner = MockSolver::new();
        inner
            .expect_solve()
            .returning(|_| Ok(vec![Default::default()]));
        inner.expect_account().return_const(account);

        let mut post_processing = MockPostProcessing::new();
        post_processing
            .expect_optimize_settlement()
            .withf(|settlement, _, gas_price| {
                gas_price.effective_gas_price() == 9_999.
                    && settlement.encoder.amount_to_unwrap(H160([0x42; 20])).is_zero()
            })
            .returning(|_, _, _| {
                async {
                    let mut settlement = Settlement::default();
                    settlement.encoder.add_unwrap(UnwrapWethInteraction {
                        amount: 42.into(),
                        weth: dummy_contract!(WETH9, [0x42; 20]),
                    });
                    settlement
                }
                .boxed()
            })
            .times(1);

        let optimizing_solver = OptimizingSolver {
            inner: Arc::new(inner),
            post_processing_pipeline: Arc::new(post_processing),
        };

        let auction = Auction {
            gas_price: 9_999.,
            ..Default::default()
        };
        let solutions = optimizing_solver.solve(auction).await.unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(
            solutions[0].encoder.amount_to_unwrap(H160([0x42; 20])),
            42.into()
        );
    }
}
