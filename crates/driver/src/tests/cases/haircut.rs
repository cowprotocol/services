//! Tests that verify the haircut is correctly applied to order limits before
//! sending to solvers.

use crate::{
    domain::{competition::order, eth},
    tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution, test_solver},
    },
};

/// Test that verifies the solver receives orders with adjusted limits when
/// haircut is applied.
///
/// For sell orders: the minimum buy amount is increased by the haircut factor
/// For buy orders: the maximum sell amount is reduced by the haircut factor
///
/// The test works by setting up a solver mock that expects the adjusted amounts
/// and will fail the assertion if the received amounts don't match.
#[tokio::test]
#[ignore]
async fn haircut_adjusts_order_limits() {
    // Test with 1% haircut (100 basis points)
    let haircut_bps = 100u32;

    for side in [order::Side::Sell, order::Side::Buy] {
        // Limit orders require solver-determined fees
        let order = ab_order()
            .kind(order::Kind::Limit)
            .side(side)
            .solver_fee(Some(eth::U256::from(500)));

        let test = tests::setup()
            .name(format!("Haircut: {side:?}"))
            .solvers(vec![test_solver().haircut_bps(haircut_bps)])
            .pool(ab_pool())
            .order(order.clone())
            .solution(ab_solution())
            .done()
            .await;

        // The solver mock will verify that the order limits are adjusted
        // according to the haircut. If the limits are not correctly adjusted,
        // the mock will fail with an assertion error.
        test.solve().await.ok().orders(&[order]);
    }
}
