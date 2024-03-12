use crate::{
    domain::{competition::order, eth},
    tests::{
        self,
        cases::EtherExt,
        setup::{
            ab_adjusted_pool,
            ab_liquidity_quote,
            ab_order,
            ab_solution,
            fee::{Policy, Quote},
            ExpectedOrderAmounts,
            Partial,
            Test,
        },
    },
};

#[derive(Clone)]
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
    // For partially fillable orders
    partially_executed: Option<Amounts>,
}

struct TestCase {
    order: Order,
    fee_policy: Policy,
    execution: Execution,
}

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

    // Amounts expected to be returned by the driver after fee processing
    let expected_amounts = ExpectedOrderAmounts {
        sell: test_case.execution.driver.sell,
        buy: test_case.execution.driver.buy,
    };
    let executed =
        test_case
            .order
            .partially_executed
            .clone()
            .map(|executed| match test_case.order.side {
                order::Side::Sell => executed.sell,
                order::Side::Buy => executed.buy,
            });

    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell_amount)
        .buy_amount(test_case.order.buy_amount)
        // Expected amounts already account for network fee, so it doesn't matter for the math.
        // However, it cannot be zero, otherwise the order would be perceived as a StaticFee orders (which cannot have Protocol Fees)
        // todo: can be cleaned up after https://github.com/cowprotocol/services/issues/2507
        .solver_fee(Some(test_case.execution.driver.sell / 100))
        .side(test_case.order.side)
        .fee_policy(test_case.fee_policy)
        .executed(executed)
        .partial(match test_case.order.partially_executed {
            Some(executed) => Partial::Yes {executed_sell: executed.sell, executed_buy: executed.buy},
            None => Partial::No
        })
        // Surplus is configured explicitly via executed/quoted amounts
        .no_surplus()
        .expected_amounts(expected_amounts);

    let test: Test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order.clone())
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok().orders(&[order]);
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
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_partial_order() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: Some(Amounts {
                sell: 25.ether().into_wei(),
                buy: 20.ether().into_wei(),
            }),
        },
        execution: Execution {
            // 20 ETH surplus in sell token (after network fee), half of which is kept by the
            // protocol
            solver: Amounts {
                sell: 30.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let fee_policy = Policy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
    };
    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let fee_policy = Policy::Volume { factor: 0.5 };
    let test_case = TestCase {
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let fee_policy = Policy::Volume { factor: 0.1 };
    let test_case = TestCase {
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 60.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_in_market_order_not_capped() {
    let fee_policy = Policy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: Quote {
            sell: 49.ether().into_wei(),
            buy: 50.ether().into_wei(),
            network_fee: 1.ether().into_wei(),
        },
    };
    let test_case = TestCase {
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            // Demanding to sell less than quoted (out-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order_not_capped() {
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
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive more than quoted (out-market)
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            // Demanding to sell more than quoted (in-market)
            sell_amount: 60.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive less than quoted (in-market)
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            // Demanding to sell less than quoted (out-market)
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
            partially_executed: None,
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
        fee_policy,
        order: Order {
            sell_amount: 50.ether().into_wei(),
            // Demanding to receive more than quoted (out-market)
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
            partially_executed: None,
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
    };
    protocol_fee_test_case(test_case).await;
}
