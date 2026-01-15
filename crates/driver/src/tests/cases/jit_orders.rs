use crate::{
    domain::{
        competition::{order, order::Side},
        eth,
    },
    tests::{
        self,
        cases::EtherExt,
        setup::{
            self,
            ExpectedOrderAmounts,
            Test,
            ab_adjusted_pool,
            ab_liquidity_quote,
            ab_order,
            ab_solution,
            test_solver,
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
    use crate::tests::cases::ApproxEq;

    let test_name = format!("JIT Order: {:?}", test_case.solution.jit_order.order.side);
    // Adjust liquidity pools so that the order is executable at the amounts
    // expected from the solver.
    let quote = ab_liquidity_quote()
        .sell_amount(test_case.execution.solver.sell)
        .buy_amount(test_case.execution.solver.buy);
    let pool = ab_adjusted_pool(quote);
    let solver_fee = test_case.execution.driver.sell / eth::U256::from(100);
    // Amounts expected to be returned by the driver after fee processing
    let jit_order_expected_amounts = if test_case.is_surplus_capturing_jit_order {
        ExpectedOrderAmounts {
            sell: test_case.execution.solver.sell,
            buy: test_case.execution.solver.buy,
        }
    } else {
        ExpectedOrderAmounts {
            sell: test_case.solution.jit_order.order.sell_amount,
            buy: test_case.solution.jit_order.order.buy_amount,
        }
    };

    let jit_order = setup::JitOrder {
        order: ab_order()
            .kind(order::Kind::Limit)
            .sell_amount(test_case.solution.jit_order.order.sell_amount)
            .buy_amount(test_case.solution.jit_order.order.buy_amount)
            .solver_fee(Some(solver_fee))
            .side(test_case.solution.jit_order.order.side)
            .no_surplus()
            .expected_amounts(jit_order_expected_amounts),
    };

    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell_amount)
        .buy_amount(test_case.order.buy_amount)
        .solver_fee(Some(solver_fee))
        .side(test_case.order.side)
        .partial(eth::U256::ZERO)
        .no_surplus();

    let solver = test_solver();

    let test: Test = tests::setup()
        .name(test_name)
        .pool(pool)
        .jit_order(jit_order.clone())
        .order(order.clone())
        .solution(ab_solution())
        .surplus_capturing_jit_order_owners(if test_case.is_surplus_capturing_jit_order {
            vec![solver.address()]
        } else {
            Vec::default()
        })
        .solvers(vec![solver])
        .done()
        .await;

    let result = test.solve().await.ok();
    assert!(
        result
            .score()
            .is_approx_eq(test_case.solution.expected_score, None),
    );
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
            // Surplus is 40 ETH worth of sell tokens, converted to buy tokens using the order's
            // limit price (50 / 60 = 80%) this leaves us with a score of 32 ETH.
            expected_score: 32.ether().into_wei(),
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
            // Surplus is 20 ETH worth of sell tokens, converted to buy tokens using the order's
            // limit price (40 / 50 = 80%) this leaves us with a score of 16 ETH.
            expected_score: 16.ether().into_wei(),
        },
    };

    protocol_fee_test_case(test_case).await;
}
