//! Fans the auction out to the configured drivers.

use {
    crate::{
        domain::{
            auction::{Auction, Order, OrderKind},
            cycle::{SolanaCycle, Solution},
        },
        infra::driver::{Driver, dto},
        run_loop::SolverCompetition,
    },
    async_trait::async_trait,
    chain_types::solana::{IntentHash, Solana},
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::Duration,
    },
    winner_selection::{Side, solution},
};

/// Solana's target slot time.
const SLOT_DURATION: Duration = Duration::from_millis(400);

/// Sends `/solve` to every driver and converts the answers into attributable
/// solutions for the arbitrator.
pub struct DriverCompetition {
    drivers: Vec<Arc<Driver>>,
    /// How long a driver gets to answer `/solve`.
    solve_deadline: Duration,
}

impl DriverCompetition {
    pub fn new(drivers: Vec<Arc<Driver>>, solve_deadline: Duration) -> Self {
        Self {
            drivers,
            solve_deadline,
        }
    }
}

#[async_trait]
impl SolverCompetition<SolanaCycle> for DriverCompetition {
    async fn solve(&self, auction: &Auction) -> Vec<Solution> {
        let request = &dto::SolveRequest {
            id: auction.id,
            // The wire carries the deadline as a slot, so the wall-clock
            // budget converts at the target slot time.
            deadline_slot: auction.tip
                + (self.solve_deadline.as_millis() / SLOT_DURATION.as_millis()) as u64,
            orders: auction.orders.iter().map(dto::Order::from).collect(),
        };
        let by_uid: HashMap<IntentHash, &Order> = auction
            .orders
            .iter()
            .map(|order| (order.uid, order))
            .collect();

        // The deadline sent to the drivers is also enforced here: a response
        // landing after it is dropped, so one hung driver cannot hold the
        // loop for the HTTP client's whole ceiling.
        let budget = self.solve_deadline;
        let responses = futures::future::join_all(self.drivers.iter().enumerate().map(
            |(driver_index, driver)| async move {
                match tokio::time::timeout(budget, driver.solve(request)).await {
                    Ok(Ok(response)) => Some((driver_index, driver, response)),
                    Ok(Err(err)) => {
                        tracing::warn!(driver = %driver.name, ?err, "solve failed");
                        None
                    }
                    Err(_) => {
                        tracing::warn!(driver = %driver.name, "solve missed the deadline");
                        None
                    }
                }
            },
        ))
        .await;

        let mut solutions = Vec::new();
        // `(solver, solution id)` attributes a ranked solution back to its
        // driver, so a duplicate would misdispatch the settlement: keep the
        // first and drop the rest.
        let mut seen = HashSet::new();
        for (driver_index, driver, response) in responses.into_iter().flatten() {
            for dto_solution in response.solutions {
                let duplicate = !seen.insert((dto_solution.solver, dto_solution.solution_id));
                if duplicate {
                    tracing::warn!(
                        driver = %driver.name,
                        solution_id = dto_solution.solution_id,
                        "duplicate solver and solution id, dropped"
                    );
                    continue;
                }
                match convert(driver_index, dto_solution, &by_uid) {
                    Some(solution) => solutions.push(solution),
                    None => tracing::warn!(
                        driver = %driver.name,
                        "solution names an order outside the auction, dropped"
                    ),
                }
            }
        }
        solutions
    }
}

/// A wire solution becomes an arbitrator solution by joining each traded
/// order against the auction for its limits and tokens. A trade naming an
/// order outside the auction voids the whole solution: its intent cannot be
/// scored.
fn convert(
    driver_index: usize,
    dto_solution: dto::Solution,
    by_uid: &HashMap<IntentHash, &Order>,
) -> Option<Solution> {
    let orders = dto_solution
        .orders
        .iter()
        .map(|(uid, amounts)| {
            let order = by_uid.get(uid)?;
            Some(solution::Order::<Solana> {
                uid: *uid,
                sell_token: order.sell_token,
                buy_token: order.buy_token,
                sell_amount: order.sell_amount,
                buy_amount: order.buy_amount,
                executed_sell: amounts.executed_sell,
                executed_buy: amounts.executed_buy,
                side: match order.kind {
                    OrderKind::Sell => Side::Sell,
                    OrderKind::Buy => Side::Buy,
                },
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Solution {
        driver_index,
        inner: solution::Solution::new(dto_solution.solution_id, dto_solution.solver, orders),
    })
}
