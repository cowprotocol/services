use crate::{
    domain::{competition::order, eth},
    infra::config::file::FeeHandler,
    tests::{
        self,
        cases::EtherExt,
        setup::{
            ab_adjusted_pool,
            ab_liquidity_quote,
            ab_order,
            ab_solution,
            fee::{Policy, Quote},
            test_solver,
            ExpectedOrderAmounts,
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

struct TestCase {
    order: Order,
    fee_policy: Vec<Policy>,
    execution: Execution,
    expected_score: eth::U256,
    fee_handler: FeeHandler,
}

// because of rounding errors, it's good enough to check that the expected value
// is within a very narrow range of the executed value
#[cfg(test)]
fn is_approximately_equal(executed_value: eth::U256, expected_value: eth::U256) -> bool {
    let lower =
        expected_value * eth::U256::from(99999999999u128) / eth::U256::from(100000000000u128); // in percents = 99.999999999%
    let upper =
        expected_value * eth::U256::from(100000000001u128) / eth::U256::from(100000000000u128); // in percents = 100.000000001%
    executed_value >= lower && executed_value <= upper
}

#[cfg(test)]
async fn protocol_fee_test_case(test_case: TestCase) {
    let test_name = format!(
        "Protocol Fee: {:?} {:?}",
        test_case.order.side, test_case.fee_policy
    );
    // Adjust liquidity pools so that the order is executable at the amounts
    // expected from the solver.
    let quote = ab_liquidity_quote()
        .sell_amount(test_case.execution.solver.sell)
        .buy_amount(test_case.execution.solver.buy);
    let pool = ab_adjusted_pool(quote);
    let solver_fee = test_case.execution.driver.sell / 100;
    let executed = match test_case.order.side {
        order::Side::Buy => (test_case.order.buy_amount > test_case.execution.solver.buy)
            .then_some(test_case.execution.solver.buy),
        order::Side::Sell => (test_case.order.sell_amount > test_case.execution.solver.sell)
            .then_some(test_case.execution.solver.sell - solver_fee),
    };
    // Amounts expected to be returned by the driver after fee processing
    let expected_amounts = ExpectedOrderAmounts {
        sell: test_case.execution.driver.sell,
        buy: test_case.execution.driver.buy,
    };
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell_amount)
        .buy_amount(test_case.order.buy_amount)
        // Expected amounts already account for network fee, so it doesn't matter for the math.
        // However, it cannot be zero, otherwise the order would be perceived as a StaticFee orders (which cannot have Protocol Fees)
        // todo: can be cleaned up after https://github.com/cowprotocol/services/issues/2507
        .solver_fee(Some(solver_fee))
        .side(test_case.order.side)
        .fee_policy(test_case.fee_policy)
        .executed(executed)
        .partial(0.into())
        // Surplus is configured explicitly via executed/quoted amounts
        .no_surplus()
        .expected_amounts(expected_amounts);

    let test: Test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order.clone())
        .solution(ab_solution())
        .solvers(vec![test_solver().fee_handler(test_case.fee_handler)])
        .done()
        .await;

    let result = test.solve().await.ok();
    assert!(is_approximately_equal(
        result.score(),
        test_case.expected_score
    ));
    result.orders(&[order]);
}

#[tokio::test]
#[ignore]
async fn triple_surplus_protocol_fee_buy_order_not_capped() {
    let fee_policy_surplus = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        fee_policy: vec![
            fee_policy_surplus.clone(),
            fee_policy_surplus.clone(),
            fee_policy_surplus,
        ],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // 20 ETH surplus in sell token (after network fee)
            // The protocol fees are applied one by one to the orders, altering the order pricing
            // For this example:
            // -> First fee policy:
            //  20 ETH surplus (50 ETH - 30 ETH), surplus policy: 50 %, fee 10 ETH
            // -> Second fee policy:
            // New sell amount in the Order: 40 ETH + 10 ETH (fee) = 50 ETH
            // New surplus: 10 ETH, fee 5 ETH
            // -> Third fee policy:
            // New sell amount in the Order: 50 ETH + 5 ETH (fee) = 55 ETH
            // New surplus: 5 ETH, fee 2.5 ETH
            // Total fee: 17.5 ETH
            // Out of the 20 ETH of surplus, 17.5 ETH goes to fees and 2.5 ETH goes to the trader
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 47.5.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn triple_surplus_protocol_fee_sell_order_not_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy.clone(), fee_policy.clone(), fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 20 ETH surplus in sell token (after network fee)
            // The protocol fees are applied one by one to the orders, altering the order pricing
            // For this example:
            // -> First fee policy:
            //  20 ETH surplus (60 ETH - 40 ETH), surplus policy: 50 %, fee 10 ETH
            // -> Second fee policy:
            // New buy amount in the Order: 40 ETH + 10 ETH (fee) = 50 ETH
            // -> Third fee policy:
            // New buy amount in the Order: 50 ETH + 5 ETH (fee) = 55 ETH
            // New surplus: 5 ETH, fee 2.5 ETH
            // Total fee: 17.5 ETH
            // Out of the 20 ETH of surplus, 17.5 ETH goes to fees and 2.5 ETH goes to the trader
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 42.5.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_and_volume_protocol_fee_buy_order_not_capped() {
    let fee_policy_surplus = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let fee_policy_volume = Policy::Volume { factor: 0.25 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy_volume, fee_policy_surplus],
        order: Order {
            sell_amount: 60.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // -> First fee policy:
            // 25% of the solver proposed sell volume is kept by the protocol
            // solver executes at the adjusted limit price ( 50 / (1 + 0.25) = 40 )
            // Fee = 50 ETH (limit price) - 40 ETH => 10 ETH
            // -> Second fee policy:
            // New buy amount in the Order: 40 ETH + 10 ETH (fee) = 50 ETH
            // New surplus: 10 ETH, fee 5 ETH
            // Total fee: 15 ETH
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            // driver executes at limit price
            driver: Amounts {
                sell: 55.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_and_price_improvement_protocol_fee_sell_order_not_capped() {
    let fee_policy_surplus = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
    };
    let fee_policy_price_improvement = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
        quote: Quote {
            sell: 50.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 5.ether().into_wei(), // 50 sell for 45 buy
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy_price_improvement, fee_policy_surplus],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 25.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // -> First fee policy:
            // Quote is 50 sell for 45 buy, which is equal to 20 sell for 18 buy
            // Solver returns 20 sell for 30 buy, so the price improvement is 12 in buy token
            // Receive 12 ETH more than quoted, half of which gets captured by the protocol
            // Fee = 12 ETH * 0.5 => 6 ETH
            // -> Second fee policy:
            // Order is 50 sell for 25 buy, which is equal to 20 sell for 10 buy
            // New buy amount in the Order: 10 ETH + 6 ETH (fee) = 16 ETH
            // New surplus: 30 ETH - 16 ETH = 14 ETH, fee 7 ETH
            // Total fee: 6 ETH + 7 ETH = 13 ETH
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                // 30 ETH - 13 ETH = 17 ETH
                buy: 17.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_and_price_improvement_fee_buy_in_market_order_not_capped() {
    let fee_policy_surplus = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let fee_policy_price_improvement = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy_price_improvement, fee_policy_surplus],
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 60.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // -> First fee policy:
            // Receive 10 ETH more than quoted, half of which gets captured by the protocol (10 ETH)
            // Fee = 10 ETH * 0.5 => 5 ETH
            // -> Second fee policy:
            // New buy amount in the Order: 40 ETH + 5 ETH (fee) = 45 ETH
            // New surplus: 5 ETH, fee 2.5 ETH
            // Total fee: 7.5 ETH
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 52.5.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_not_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
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
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn protocol_fee_calculated_on_the_solver_side() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 35.ether().into_wei(),
        fee_handler: FeeHandler::Solver,
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 20 ETH surplus, half of which gets captured by the protocol
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 50.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_partial_buy_order_not_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // 10 ETH surplus in sell token (after network fee), half of which is kept by the
            // protocol
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 15.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_partial_sell_order_not_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 10 ETH surplus, half of which gets captured by the protocol
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 25.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed sell volume
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 33.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // log enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed buy volume
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 54.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_partial_buy_order_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.2,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Fee is capped at 20% of solver proposed sell volume
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 12.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_partial_sell_order_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // log enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed buy volume
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 27.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let fee_policy = Policy::Volume { factor: 0.5 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Half of the solver proposed sell volume is kept by the protocol
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 45.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order_at_limit_price() {
    let fee_policy = Policy::Volume { factor: 0.25 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // 25% of the solver proposed sell volume is kept by the protocol
            // solver executes at the adjusted limit price ( 50 / (1 + 0.25) = 40 )
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            // driver executes at limit price
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let fee_policy = Policy::Volume { factor: 0.1 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 10% of the solver proposed buy value is kept by the protocol
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 50.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 45.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order_at_limit_price() {
    let fee_policy = Policy::Volume { factor: 0.2 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 20% of the solver proposed buy value is kept by the protocol
            // solver executes at the adjusted limit price ( 40 / (1 - 0.2) = 50 )
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 50.ether().into_wei(),
            },
            // driver executes at limit price
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_partial_buy_order() {
    let fee_policy = Policy::Volume { factor: 0.5 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Half of the solver proposed sell volume is kept by the protocol
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 15.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_partial_buy_order_at_limit_price() {
    let fee_policy = Policy::Volume { factor: 0.25 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // 25% of the solver proposed sell volume is kept by the protocol
            // solver executes at the adjusted limit price ( 50 / (1 + 0.25) = 40 ), which scaled
            // for partially fillable order gives 16
            solver: Amounts {
                sell: 16.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            // driver executes at limit price
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 4.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_partial_sell_order() {
    let fee_policy = Policy::Volume { factor: 0.1 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 10% of the solver proposed buy value is kept by the protocol
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 27.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_partial_sell_order_at_limit_price() {
    let fee_policy = Policy::Volume { factor: 0.2 };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // 20% of the solver proposed buy value is kept by the protocol
            // solver executes at the adjusted limit price ( 50 / (1 - 0.2) = 62.5 ), which scaled
            // for partially fillable order gives 25
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 25.ether().into_wei(),
            },
            // driver executes at limit price
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 5.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_in_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 60.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Sell 10 ETH less than quoted, half of which is kept by the protocol
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 45.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_in_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
        quote: Quote {
            sell: 50.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 4.ether().into_wei(), // 50 sell for 46 buy
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Receive 14 ETH more than quoted, half of which gets captured by the protocol
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 53.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_out_of_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: Quote {
            sell: 59.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell less than quoted (out-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Sell 10 ETH less than requested, half of which is kept by the protocol
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 45.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive more than quoted (out-market)
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Receive 10 ETH more than quoted, half of which gets captured by the protocol
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 55.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_in_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.05,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 60.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Fee is capped at 5% of solver proposed sell volume
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 42.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_in_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.05,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Fee is capped at 5% of solver proposed buy volume
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 57.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_out_of_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.05,
        quote: Quote {
            sell: 59.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell less than quoted (out-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Fee is capped at 5% of solver proposed sell volume
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 42.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.05,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive more than quoted (out-market)
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Fee is capped at 5% of solver proposed buy volume
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 60.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 57.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_buy_in_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: Quote {
            sell: 39.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Sell 10 ETH less than quoted, half of which is kept by the protocol
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 15.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 15.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_sell_in_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
        quote: Quote {
            sell: 50.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 5.ether().into_wei(), // 50 sell for 45 buy
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 25.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Quote is 50 sell for 45 buy, which is equal to 20 sell for 18 buy
            // Solver returns 20 sell for 30 buy, so the price improvement is 12 in buy token
            // Receive 12 ETH more than quoted, half of which gets captured by the protocol
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 24.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_buy_out_of_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: Quote {
            sell: 59.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell less than quoted (out-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Sell 10 ETH less than requested, half of which is kept by the protocol
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 15.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_sell_out_of_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive more than quoted (out-market)
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Receive 10 ETH more than quoted, half of which gets captured by the protocol
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 25.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_buy_in_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 75.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed sell volume
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 11.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_sell_in_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 25.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed buy volume
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 27.ether().into_wei(),
            },
        },
        expected_score: 20.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_buy_out_of_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
        quote: Quote {
            sell: 59.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            // Demanding to sell less than quoted (out-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed sell volume
            solver: Amounts {
                sell: 10.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            driver: Amounts {
                sell: 11.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_partial_sell_out_of_market_order_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 40.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive more than quoted (out-market)
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Fee is capped at 10% of solver proposed buy volume
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 30.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 27.ether().into_wei(),
            },
        },
        expected_score: 10.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_no_improvement() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 0.9,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy: vec![fee_policy],
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        execution: Execution {
            // Receive 5 ETH less than quoted, no improvement
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 45.ether().into_wei(),
            },
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 45.ether().into_wei(),
            },
        },
        expected_score: 5.ether().into_wei(),
        fee_handler: FeeHandler::Driver,
    };
    protocol_fee_test_case(test_case).await;
}
