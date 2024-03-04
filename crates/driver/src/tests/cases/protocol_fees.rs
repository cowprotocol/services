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
        order_sell_amount: 50.ether().into_wei(),
        solver_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 100.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
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
        order_sell_amount: 50.ether().into_wei(),
        solver_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: "20.000000002".ether().into_wei(),
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
        order_sell_amount: 50.ether().into_wei(),
        solver_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 55.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
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
        order_sell_amount: 50.ether().into_wei(),
        solver_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 36.ether().into_wei(),
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
        order_sell_amount: 50.ether().into_wei(),
        solver_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 75.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
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
        order_sell_amount: 50.ether().into_wei(),
        solver_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 20.ether().into_wei(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_out_market_order() {
    let quote_sell_amount = 21000000000000000000u128;
    let quote_buy_amount = 18000000000000000000u128;
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: quote_sell_amount.into(),
            buy_amount: quote_buy_amount.into(),
            fee: 1000000000000000000u128.into(),
        },
    };
    let executed_buy = 17143028023069342830u128;
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 20000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: quote_sell_amount.into(),
        quote_buy_amount: quote_buy_amount.into(),
        executed: executed_buy.into(),
        executed_sell_amount: 20476294902986820618u128.into(),
        // executed buy amount should match order buy amount
        executed_buy_amount: executed_buy.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_market_order() {
    let quote_sell_amount = 21000000000000000000u128;
    let quote_buy_amount = 18000000000000000000u128;
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: quote_sell_amount.into(),
            buy_amount: quote_buy_amount.into(),
            fee: 1000000000000000000u128.into(),
        },
    };
    let order_sell_amount = 20000000000000000000u128;
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: order_sell_amount.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: quote_sell_amount.into(),
        quote_buy_amount: quote_buy_amount.into(),
        executed: 10000000000000000000u128.into(),
        // executed sell amount should match order sell amount
        executed_sell_amount: order_sell_amount.into(),
        executed_buy_amount: 16753332193352853234u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_in_market_order() {
    let quote_sell_amount = 17000000000000000000u128;
    let quote_buy_amount = 10000000000000000000u128;
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: quote_sell_amount.into(),
            buy_amount: quote_buy_amount.into(),
            fee: 1000000000000000000u128.into(),
        },
    };
    let executed_buy_amount = 11764354070151352996u128;
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 20000000000000000000u128.into(),
        solver_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: quote_sell_amount.into(),
        quote_buy_amount: quote_buy_amount.into(),
        executed: executed_buy_amount.into(),
        executed_sell_amount: 20587918663136217696u128.into(),
        // executed buy amount should match order buy amount
        executed_buy_amount: executed_buy_amount.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_in_market_order() {
    let quote_sell_amount = 9000000000000000000u128;
    let quote_buy_amount = 25000000000000000000u128;
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: quote_sell_amount.into(),
            buy_amount: quote_buy_amount.into(),
            fee: 1000000000000000000u128.into(),
        },
    };
    let order_sell_amount = 10000000000000000000u128;
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: order_sell_amount.into(),
        solver_fee: Some(5000000000000000000u128.into()),
        quote_sell_amount: quote_sell_amount.into(),
        quote_buy_amount: quote_buy_amount.into(),
        executed: 5000000000000000000u128.into(),
        // executed sell amount should match order sell amount
        executed_sell_amount: order_sell_amount.into(),
        executed_buy_amount: 26388750430470970935u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}
