use crate::{
    domain::{
        competition::{order, order::Side},
        eth,
    },
    tests::{
        self,
        cases::{is_approximately_equal, EtherExt},
        setup::{
            self,
            ab_adjusted_pool,
            ab_liquidity_quote,
            ab_order,
            ab_solution,
            test_solver,
            Test,
        },
    },
};

struct Amounts {
    sell: eth::U256,
    buy: eth::U256,
}

struct Execution {
    // The executed net-amounts (including network fee) reported by the solver
    solver: Amounts,
    // The executed net-amounts (including network and protocol) reported by the driver
    driver: Amounts,
}

struct Order {
    sell_amount: eth::U256,
    buy_amount: eth::U256,
    side: order::Side,
}

struct JitOrder {
    order: Order,
}

struct Solution {
    jit_order: JitOrder,
    expected_score: eth::U256,
}

struct TestCase {
    order: Order,
    execution: Execution,
    is_surplus_capturing_jit_order: bool,
    solution: Solution,
}

#[cfg(test)]
async fn protocol_fee_test_case(test_case: TestCase) {
    let test_name = format!("JIT Order: {:?}", test_case.solution.jit_order.order.side);
    // Adjust liquidity pools so that the order is executable at the amounts
    // expected from the solver.
    let quote = ab_liquidity_quote()
        .sell_amount(test_case.execution.solver.sell)
        .buy_amount(test_case.execution.solver.buy);
    let pool = ab_adjusted_pool(quote);
    let solver_fee = test_case.execution.driver.sell / 100;

    let jit_order = setup::JitOrder {
        order: ab_order()
            .kind(order::Kind::Limit)
            .sell_amount(test_case.solution.jit_order.order.sell_amount)
            .buy_amount(test_case.solution.jit_order.order.buy_amount)
            .solver_fee(Some(solver_fee))
            .side(test_case.solution.jit_order.order.side)
            .no_surplus(),
    };

    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell_amount)
        .buy_amount(test_case.order.buy_amount)
        .solver_fee(Some(solver_fee))
        .side(test_case.order.side)
        .partial(0.into())
        .no_surplus();

    let solver = test_solver();

    let test: Test = tests::setup()
        .name(test_name)
        .pool(pool)
        .jit_order(jit_order.clone())
        .surplus_capturing_jit_order_owners(
            test_case
                .is_surplus_capturing_jit_order
                .then(|| vec![solver.address()])
                .unwrap_or_default(),
        )
        .solvers(vec![solver])
        .order(order.clone())
        .solution(ab_solution())
        .done()
        .await;

    let result = test.solve().await.ok();
    assert!(is_approximately_equal(
        result.score(),
        test_case.solution.expected_score,
    ));
    result.jit_orders(&[jit_order]);
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_jit_order_from_surplus_capturing_owner_not_capped() {
    let test_case = TestCase {
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: Side::Buy,
        },
        execution: Execution {
            // 20 ETH surplus in sell token (after network fee), half of which is kept by the
            // protocol
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        is_surplus_capturing_jit_order: true,
        solution: Solution {
            jit_order: JitOrder {
                order: Order {
                    sell_amount: 50.ether().into_wei(),
                    buy_amount: 40.ether().into_wei(),
                    side: Side::Buy,
                },
            },
            // Score is 20 x 2 since there are two orders with score 20 (user order + JIT order)
            expected_score: 40.ether().into_wei(),
        },
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_jit_order_not_capped() {
    let test_case = TestCase {
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: Side::Buy,
        },
        execution: Execution {
            // 20 ETH surplus in sell token (after network fee), half of which is kept by the
            // protocol
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        is_surplus_capturing_jit_order: false,
        solution: Solution {
            jit_order: JitOrder {
                order: Order {
                    sell_amount: 50.ether().into_wei(),
                    buy_amount: 40.ether().into_wei(),
                    side: Side::Buy,
                },
            },
            // Score is 20 since the JIT order is not from a surplus capturing owner
            expected_score: 20.ether().into_wei(),
        },
    };

    protocol_fee_test_case(test_case).await;
}
