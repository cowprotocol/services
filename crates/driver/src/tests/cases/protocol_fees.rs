use crate::{
    domain::{competition::order, eth},
    tests::{
        self,
        setup::{
            ab_adjusted_pool,
            ab_liquidity_quote,
            ab_order,
            ab_solution,
            ExpectedOrderAmounts,
            FeePolicy,
            Test,
        },
    },
};

struct TestCase {
    order_side: order::Side,
    fee_policy: FeePolicy,
    order_sell_amount: eth::U256,
    solver_fee: Option<eth::U256>,
    quote_sell_amount: eth::U256,
    quote_buy_amount: eth::U256,
    executed: eth::U256,
    executed_sell_amount: eth::U256,
    executed_buy_amount: eth::U256,
}

async fn protocol_fee_test_case(test_case: TestCase) {
    let test_name = format!(
        "Protocol Fee: {:?} {:?}",
        test_case.order_side, test_case.fee_policy
    );
    let quote = ab_liquidity_quote()
        .sell_amount(test_case.quote_sell_amount)
        .buy_amount(test_case.quote_buy_amount);
    let pool = ab_adjusted_pool(quote);
    let expected_amounts = ExpectedOrderAmounts {
        sell: test_case.executed_sell_amount,
        buy: test_case.executed_buy_amount,
    };
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order_sell_amount)
        .side(test_case.order_side)
        .solver_fee(test_case.solver_fee)
        .fee_policy(test_case.fee_policy)
        .executed(test_case.executed)
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
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 50000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 100000000000000000000u128.into(),
        executed_buy_amount: 40000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: 50000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 50000000000000000000u128.into(),
        executed_buy_amount: 20000000002000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 50000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 55000000000000000000u128.into(),
        executed_buy_amount: 40000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: 50000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 50000000000000000000u128.into(),
        executed_buy_amount: 35000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 50000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 75000000000000000000u128.into(),
        executed_buy_amount: 40000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: 50000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 50000000000000000000u128.into(),
        executed_buy_amount: 15000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}
