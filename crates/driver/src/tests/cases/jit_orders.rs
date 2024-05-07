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
            SolverName,
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
    solver: SolverName,
}

struct Solver {
    name: SolverName,
    is_surplus_capturing_jit_order: bool,
    expected_score: eth::U256,
}

struct TestCase {
    order: Order,
    execution: Execution,
    jit_order: JitOrder,
    solvers: Vec<Solver>,
}

#[cfg(test)]
async fn protocol_fee_test_case(test_case: TestCase) {
    let test_name = format!("JIT Order: {:?}", test_case.order.side);
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
            .sell_amount(test_case.jit_order.order.sell_amount)
            .buy_amount(test_case.jit_order.order.buy_amount)
            .solver_fee(Some(solver_fee))
            .side(test_case.jit_order.order.side)
            .partial(0.into())
            .no_surplus(),
        solver: test_case.jit_order.solver,
    };

    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell_amount)
        .buy_amount(test_case.order.buy_amount)
        .solver_fee(Some(solver_fee))
        .side(test_case.order.side)
        .partial(0.into())
        .no_surplus();

    let solvers = test_case
        .solvers
        .iter()
        .map(|solver| {
            test_solver()
                .name(&solver.name.to_string())
                .set_surplus_capturing_jit_order_owner(solver.is_surplus_capturing_jit_order)
                .set_configure_for_jit_orders(true)
        })
        .collect::<Vec<_>>();
    let test: Test = tests::setup()
        .name(test_name)
        .pool(pool)
        .jit_order(jit_order.clone())
        .order(order.clone())
        .solution(ab_solution())
        .set_surplus_capturing_jit_order_owners(
            solvers
                .iter()
                .filter(|solver| solver.get_surplus_capturing_jit_order_owner())
                .map(|solver| solver.address())
                .collect::<Vec<_>>(),
        )
        .solvers(solvers)
        .done()
        .await;

    for solver in test_case.solvers {
        let result = test.solve_with_solver(&solver.name.to_string()).await.ok();
        assert!(is_approximately_equal(
            result.score(),
            solver.expected_score
        ));
    }
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
        jit_order: JitOrder {
            order: Order {
                sell_amount: 50.ether().into_wei(),
                buy_amount: 40.ether().into_wei(),
                side: Side::Buy,
            },
            solver: SolverName::One,
        },
        solvers: vec![Solver {
            name: SolverName::One,
            is_surplus_capturing_jit_order: true,
            // Score is 20 x 2 since there are two orders with score 20 (user order + JIT order)
            expected_score: 40.ether().into_wei(),
        }],
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
        jit_order: JitOrder {
            order: Order {
                sell_amount: 50.ether().into_wei(),
                buy_amount: 40.ether().into_wei(),
                side: Side::Buy,
            },
            solver: SolverName::One,
        },
        solvers: vec![Solver {
            name: SolverName::One,
            is_surplus_capturing_jit_order: false,
            // Score is 20 since the JIT order is not from a surplus capturing owner
            expected_score: 20.ether().into_wei(),
        }],
    };

    protocol_fee_test_case(test_case).await;
}
