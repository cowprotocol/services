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
            ExpectedOrderAmounts,
            FeePolicy,
            PriceImprovementQuote,
            Test,
        },
    },
};

struct Amounts {
    sell: eth::U256,
    buy: eth::U256,
}

struct Execution {
    solver: Amounts,
    driver: Amounts,
}

struct TestCase {
    order: Amounts,
    side: order::Side,
    fee_policy: FeePolicy,
    execution: Execution,
}

async fn protocol_fee_test_case(test_case: TestCase) {
    let test_name = format!(
        "Protocol Fee: {:?} {:?}",
        test_case.side, test_case.fee_policy
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

    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell)
        .side(test_case.side)
        .fee_policy(test_case.fee_policy)
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
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        side: order::Side::Buy,
        fee_policy,
        order: Amounts {
            sell: 50.ether().into_wei(),
            buy: 40.ether().into_wei(),
        },
        execution: Execution {
            // 50 ETH surplus in sell token, half of which is kept by the driver
            solver: Amounts {
                sell: 100.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            driver: Amounts {
                sell: 75.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
    };

    protocol_fee_test_case(test_case).await;
}

/*

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        side: order::Side::Sell,
        fee_policy,
        order: Amounts { sell: 50.ether().into_wei(),
        buy: 40.ether().into_wei(),},
        execution: Execution {solver: Amounts {sell: 50.ether().into_wei(),
        buy: "20.000000002".ether().into_wei(),}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = TestCase {
        side: order::Side::Buy,
        fee_policy,
        order: Amounts { sell: 50.ether().into_wei(),
        buy: 40.ether().into_wei(),},
        execution: Execution {solver: Amounts {sell: 55.ether().into_wei(),
        buy: 40.ether().into_wei(),}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        quote_buy_amount: 40.ether().into_wei(),
        execution: Execution {solver: Amounts {sell: 50.ether().into_wei(),
        buy: 36.ether().into_wei(),}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_case = TestCase {
        side: order::Side::Buy,
        fee_policy,
        order: Amounts { sell: 50.ether().into_wei(),
        buy: 40.ether().into_wei(),},
        execution: Execution {solver: Amounts {sell: 75.ether().into_wei(),
        buy: 40.ether().into_wei(),}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_case = TestCase {
        side: order::Side::Sell,
        fee_policy,
        order: Amounts { sell: 50.ether().into_wei(),
        buy: 40.ether().into_wei(),},
        execution: Execution {solver: Amounts {sell: 50.ether().into_wei(),
        buy: 20.ether().into_wei(),}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_out_of_market_order() {
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: 50000000000000000000u128.into(),
            buy_amount: 35000000000000000000u128.into(),
        },
    };
    let order_buy_amount = 40000000000000000000u128.into();
    let test_case = TestCase {
        side: order::Side::Buy,
        fee_policy,
        buy: order_buy_amount,},
        execution: Execution {solver: Amounts {sell: 54142857142857142857u128.into(),
        buy: order_buy_amount,}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order() {
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: 50000000000000000000u128.into(),
            buy_amount: 35000000000000000000u128.into(),
        },
    };
    let order_buy_amount = 40000000000000000000u128.into();
    let test_case = TestCase {
        side: order::Side::Sell,
        fee_policy,
        buy: order_buy_amount,},
        buy: 37156862745098039215u128.into(),}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_in_market_order() {
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: 50000000000000000000u128.into(),
            buy_amount: 40000000000000000000u128.into(),
        },
    };
    let order_buy_amount = 35000000000000000000u128.into();
    let test_case = TestCase {
        side: order::Side::Buy,
        fee_policy,
        buy: order_buy_amount,},
        buy: order_buy_amount,}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_in_market_order() {
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: 50000000000000000000u128.into(),
            buy_amount: 40000000000000000000u128.into(),
        },
    };
    let order_buy_amount: eth::U256 = 35000000000000000000u128.into();
    let test_case = TestCase {
        side: order::Side::Sell,
        fee_policy,
        buy: order_buy_amount,},
        buy: order_buy_amount,}, driver: Amounts { sell: U256::zero(), buy:U256.one()}}
    };

    protocol_fee_test_case(test_case).await;
}
*/
