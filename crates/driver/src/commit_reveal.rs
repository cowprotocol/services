use anyhow::{Context, Result};
use gas_estimation::GasPriceEstimating;
use model::order::OrderUid;
use num::ToPrimitive;
use number_conversions::big_rational_to_u256;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use shared::conversions::U256Ext;
use solver::{
    settlement::Settlement,
    settlement_rater::SettlementRating,
    solver::{Auction, Solver},
};
use std::sync::{Arc, Mutex};

/// A `SolutionSummary` holds all information solvers are willing to disclose during settlement
/// competition. It does **not** have to include the call data, yet.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
pub struct SettlementSummary {
    /// Surplus is denominated in the chain's native token and based off of the auction's external
    /// prices.
    pub surplus: f64,
    /// This is how much gas the solver would like to get reimbursed for executing this solution.
    pub gas_reimbursement: U256,
    /// Orders which would get settled by this solution. Partially fillable orders don't have to be
    /// filled completely to be considered in this list.
    pub settled_orders: Vec<OrderUid>,
}

// #[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
#[cfg_attr(feature = "test", mockall::automock)]
pub trait CommitRevealSolving: Send + Sync {
    /// Calculates a solution for a given `Auction` but does **not** disclose secret details.
    async fn commit(&self, auction: Auction) -> Result<SettlementSummary>;

    /// Finalizes solution for a previously calculated `SolutionSummary` which can be used to compute
    /// executable call data. If the solver no longer wants to execute the solution it returns
    /// `Ok(None)`.
    async fn reveal(&self, summary: SettlementSummary) -> Result<Option<Settlement>>;
}

// Wraps a legacy `Solver` implementation and makes it compatible with the commit reveal protocol.
// Because RFQ support can not be solved generically the wrapped `Solver` will not be able to opt into
// RFQ orders, yet. A solver would have to support RFQ themselves.
// For now this wrapper is only a compatibility layer to let us use the new driver with existing
// solvers for faster development.
pub struct CommitRevealSolver {
    solver: Arc<dyn Solver>,
    settlement_rater: Arc<dyn SettlementRating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    stored_solution: Mutex<Option<(SettlementSummary, Settlement)>>,
}

impl CommitRevealSolver {
    pub fn new(
        solver: Arc<dyn Solver>,
        settlement_rater: Arc<dyn SettlementRating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
    ) -> Self {
        Self {
            solver,
            settlement_rater,
            gas_estimator,
            stored_solution: Mutex::new(Default::default()),
        }
    }

    async fn _commit(&self, auction: Auction) -> Result<(SettlementSummary, Settlement)> {
        let prices = auction.external_prices.clone();
        let solutions = self.solver.solve(auction).await?;
        let solutions = solutions
            .into_iter()
            .map(|solution| (self.solver.clone(), solution))
            .collect();

        let gas_price = self.gas_estimator.estimate().await?;
        let (mut rated_settlements, _) = self
            .settlement_rater
            .rate_settlements(solutions, &prices, gas_price)
            .await?;

        rated_settlements.sort_by(|a, b| a.1.objective_value().cmp(&b.1.objective_value()));
        if let Some((_, winning_settlement, _)) = rated_settlements.pop() {
            let summary = SettlementSummary {
                surplus: winning_settlement
                    .surplus
                    .to_f64()
                    .context("couldn't convert surplus to f64")?,
                gas_reimbursement: big_rational_to_u256(
                    &(winning_settlement.gas_estimate.to_big_rational()
                        * winning_settlement.gas_price),
                )?,
                settled_orders: winning_settlement
                    .settlement
                    .traded_orders()
                    .map(|order| order.metadata.uid)
                    .collect(),
            };

            return Ok((summary, winning_settlement.settlement));
        }

        Err(anyhow::anyhow!("could not compute a valid solution"))
    }
}

#[async_trait::async_trait]
impl CommitRevealSolving for CommitRevealSolver {
    async fn commit(&self, auction: Auction) -> Result<SettlementSummary> {
        let result = self._commit(auction).await;
        let mut stored_solution = self.stored_solution.lock().unwrap();

        match result {
            Ok((summary, settlement)) => {
                *stored_solution = Some((summary.clone(), settlement));
                Ok(summary)
            }
            Err(err) => {
                // unset stored_solution so we are not able to reveal an outdated solution by accident
                *stored_solution = None;
                Err(err)
            }
        }
    }

    async fn reveal(&self, summary: SettlementSummary) -> Result<Option<Settlement>> {
        match &*self.stored_solution.lock().unwrap() {
            Some(stored_solution) if stored_solution.0 == summary => {
                // A solver could opt-out of executing the solution but since this is just a component
                // wrapping solvers which don't yet implement the commit-reveal scheme natively we
                // have no way of knowing if the solver would still execute the solution.
                // That's why we will always chose to execute the solution.
                Ok(Some(stored_solution.1.clone()))
            }
            _ => Err(anyhow::anyhow!(
                "could not find solution for requested summary"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;
    use model::order::Order;
    use num::BigRational;
    use primitive_types::H160;
    use shared::gas_price_estimation::FakeGasPriceEstimator;
    use solver::{
        driver::solver_settlements::RatedSettlement, settlement_rater::MockSettlementRating,
        solver::MockSolver,
    };
    use web3::types::AccessList;

    fn settlement(user_order_ids: &[u32], liquidity_order_ids: &[u32]) -> Settlement {
        let mut settlement = Settlement::new(hashmap! { H160::default() => U256::exp10(18) });
        let order = |id: &u32, amounts: u128| {
            let mut order = Order::default();
            order.data.buy_amount = amounts.into();
            order.data.sell_amount = amounts.into();
            order.metadata.uid = OrderUid::from_integer(*id);
            order
        };
        for id in user_order_ids {
            settlement
                .encoder
                .add_trade(order(id, 1), 1.into(), 0.into())
                .unwrap();
        }
        for id in liquidity_order_ids {
            settlement
                .encoder
                .add_liquidity_order_trade(order(id, 1), 1.into(), 0.into())
                .unwrap();
        }
        settlement
    }

    fn rated_settlement(
        id: usize,
        objective: f64,
        gas: u128,
        settlement: Settlement,
    ) -> (Arc<dyn Solver>, RatedSettlement, Option<AccessList>) {
        (
            Arc::new(MockSolver::new()) as Arc<dyn Solver>,
            RatedSettlement {
                id,
                surplus: num::BigRational::from_float(objective).unwrap(),
                settlement,
                unscaled_subsidized_fee: BigRational::from_float(0.).unwrap(),
                scaled_unsubsidized_fee: BigRational::from_float(0.).unwrap(),
                gas_estimate: gas.into(),
                gas_price: BigRational::from_float(1.).unwrap(),
            },
            None,
        )
    }

    #[tokio::test]
    async fn commits_best_solutions() {
        let auction = Auction {
            id: 1, // specific id to verify that the auction gets propagated correctly
            ..Default::default()
        };
        let gas_price_estimator = Arc::new(FakeGasPriceEstimator::new(Default::default()));

        let mut settlement_rater = MockSettlementRating::new();
        settlement_rater
            .expect_rate_settlements()
            .times(1)
            // used to verify ordering by objective value
            .returning(|settlements, _, _| {
                Ok((
                    vec![
                        rated_settlement(1, 8., 3, settlements[0].1.clone()),
                        rated_settlement(2, 10., 2, settlements[1].1.clone()),
                        rated_settlement(3, 6., 4, settlements[2].1.clone()),
                        rated_settlement(4, 4., 5, settlements[3].1.clone()),
                    ],
                    vec![],
                ))
            });
        settlement_rater
            .expect_rate_settlements()
            .times(1)
            // check solution overwrite behavior on success
            .returning(|settlements, _, _| {
                Ok((
                    vec![rated_settlement(1, 8., 3, settlements[0].1.clone())],
                    vec![],
                ))
            });

        let mut inner = MockSolver::new();
        inner
            .expect_solve()
            .times(1)
            .withf(|auction| auction.id == 1)
            // used to verify ordering by objective value
            .returning(|_| {
                Ok(vec![
                    settlement(&[1], &[2]),
                    settlement(&[3], &[4]),
                    settlement(&[5], &[6]),
                    settlement(&[7], &[8]),
                ])
            });
        inner
            .expect_solve()
            .times(1)
            // check solution overwrite behavior on success
            .returning(|_| Ok(vec![settlement(&[1], &[2])]));
        inner
            .expect_solve()
            .times(1)
            // used to check solution overwrite behavior on error
            .returning(|_| Err(anyhow::anyhow!("couldn't compute solution")));

        let solver = CommitRevealSolver::new(
            Arc::new(inner),
            Arc::new(settlement_rater),
            gas_price_estimator,
        );

        // solution with best objective value won and the summary is correct
        let first_winner = solver.commit(auction).await.unwrap();
        assert_eq!(
            first_winner,
            SettlementSummary {
                gas_reimbursement: 2.into(),
                surplus: 10.,
                settled_orders: [3, 4]
                    .iter()
                    .map(|id| OrderUid::from_integer(*id))
                    .collect()
            }
        );

        // can't reveal solution if the summary doesn't match exactly
        let modified_winner = SettlementSummary {
            surplus: 9.,
            ..first_winner.clone()
        };
        assert!(solver.reveal(modified_winner).await.is_err());

        // can correctly reveal the latest solution if the summary matches
        let revealed_solution = solver.reveal(first_winner.clone()).await.unwrap().unwrap();
        assert_eq!(
            revealed_solution
                .traded_orders()
                .map(|o| o.metadata.uid)
                .collect::<Vec<_>>(),
            vec![OrderUid::from_integer(3), OrderUid::from_integer(4)]
        );

        // new solution overwrites previous solution
        let second_winner = solver.commit(Default::default()).await.unwrap();
        assert!(solver.reveal(first_winner).await.is_err());

        // new solution can be revealed now
        let revealed_solution = solver.reveal(second_winner.clone()).await.unwrap().unwrap();
        assert_eq!(
            revealed_solution
                .traded_orders()
                .map(|o| o.metadata.uid)
                .collect::<Vec<_>>(),
            vec![OrderUid::from_integer(1), OrderUid::from_integer(2)]
        );

        // error during solution computation unsets the stored solution
        assert!(solver.commit(Default::default()).await.is_err());
        assert!(solver.reveal(second_winner).await.is_err());
    }
}
