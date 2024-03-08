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

const DEFAULT_SURPLUS_FACTOR: u64 = 2;

struct CommonTestCase {
    order_side: order::Side,
    fee_policy: FeePolicy,
    order_sell_amount: eth::U256,
    network_fee: Option<eth::U256>,
    quote_sell_amount: eth::U256,
    quote_buy_amount: eth::U256,
    executed: eth::U256,
    executed_sell_amount: eth::U256,
    executed_buy_amount: eth::U256,
}

struct PriceImprovementTestCase {
    order_side: order::Side,
    policy_factor: f64,
    policy_max_volume_factor: f64,
    quote: PriceImprovementQuote,
    order_sell_amount: eth::U256,
    order_buy_amount: eth::U256,
    network_fee: Option<eth::U256>,
    executed: eth::U256,
    executed_sell_amount: eth::U256,
    executed_buy_amount: eth::U256,
}

async fn common_fee_test_case(test_case: CommonTestCase) {
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
        .surplus(DEFAULT_SURPLUS_FACTOR.into())
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order_sell_amount)
        .side(test_case.order_side)
        .solver_fee(test_case.network_fee)
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

async fn price_improvement_fee_test_case(test_case: PriceImprovementTestCase) {
    let test_name = format!("Protocol Fee: {:?} PriceImprovement", test_case.order_side);
    let liquidity_quote = ab_liquidity_quote()
        .sell_amount(test_case.order_sell_amount)
        .buy_amount(test_case.order_buy_amount);
    let pool = ab_adjusted_pool(liquidity_quote);
    let expected_amounts = ExpectedOrderAmounts {
        sell: test_case.executed_sell_amount,
        buy: test_case.executed_buy_amount,
    };
    let fee_policy = FeePolicy::PriceImprovement {
        factor: test_case.policy_factor,
        max_volume_factor: test_case.policy_max_volume_factor,
        quote: test_case.quote,
    };
    let order = ab_order()
        .no_surplus()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order_sell_amount)
        .side(test_case.order_side)
        .solver_fee(test_case.network_fee)
        .fee_policy(fee_policy)
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
    let order_buy_amount = 40.ether().into_wei();
    let order_sell_amount = 50.ether().into_wei();
    let test_case = CommonTestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        executed_sell_amount: 75.ether().into_wei(),
        executed_buy_amount: order_buy_amount,
    };

    common_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let order_sell_amount = 50.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let test_case = CommonTestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        executed_sell_amount: order_sell_amount,
        executed_buy_amount: 30.ether().into_wei(),
    };

    common_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = CommonTestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 50.ether().into_wei(),
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 55.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
    };

    common_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_case = CommonTestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: 50.ether().into_wei(),
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 36.ether().into_wei(),
    };

    common_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_case = CommonTestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount: 50.ether().into_wei(),
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 75.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
    };

    common_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_case = CommonTestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount: 50.ether().into_wei(),
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 20.ether().into_wei(),
    };

    common_fee_test_case(test_case).await;
}

// Price Improvement policy fee tests.
// Out of market order could be defined as:
//   (order.sell + order.fee) * quote.buy < (quote.sell + quote.fee) * order.buy
// In the following tests Limit orders are used only, where order fee is 0. The
// amount values are adjusted to respect the definition.

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_out_of_market_order() {
    let order_sell_amount = 50.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let quote_network_fee = 20.ether().into_wei();
    let quote = PriceImprovementQuote {
        sell_amount: order_sell_amount,
        buy_amount: order_buy_amount,
        network_fee: quote_network_fee,
    };
    let test_case = PriceImprovementTestCase {
        order_side: order::Side::Buy,
        policy_factor: 0.5,
        policy_max_volume_factor: 1.0,
        quote,
        order_sell_amount,
        order_buy_amount,
        network_fee: Some(1.ether().into_wei()),
        executed: order_buy_amount,
        // order sell amount + quote network fee * factor
        executed_sell_amount: 60.ether().into_wei(),
        executed_buy_amount: order_buy_amount,
    };

    price_improvement_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order() {
    let order_sell_amount = 50.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let quote = PriceImprovementQuote {
        sell_amount: order_sell_amount,
        buy_amount: order_buy_amount,
        network_fee: 20.ether().into_wei(),
    };
    let network_fee = 10.ether().into_wei();
    let test_case = PriceImprovementTestCase {
        order_side: order::Side::Sell,
        policy_factor: 0.5,
        policy_max_volume_factor: 1.0,
        quote,
        order_sell_amount,
        order_buy_amount,
        network_fee: Some(network_fee),
        executed: order_sell_amount - network_fee,
        executed_sell_amount: order_sell_amount,
        // todo: how to prove the value?
        executed_buy_amount: "34.285714285714285714".ether().into_wei(),
    };

    price_improvement_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_in_market_order() {
    let order_sell_amount = 100.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let quote = PriceImprovementQuote {
        sell_amount: 50.ether().into_wei(),
        buy_amount: 40.ether().into_wei(),
        network_fee: 20.ether().into_wei(),
    };
    let network_fee = 10.ether().into_wei();
    let test_case = PriceImprovementTestCase {
        order_side: order::Side::Buy,
        policy_factor: 0.5,
        policy_max_volume_factor: 1.0,
        quote,
        order_sell_amount,
        order_buy_amount,
        network_fee: Some(network_fee),
        executed: order_buy_amount,
        // no price improvement since quote provides better conditions
        executed_sell_amount: order_sell_amount,
        executed_buy_amount: order_buy_amount,
    };

    price_improvement_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_in_market_order() {
    let order_sell_amount: eth::U256 = 50.ether().into_wei();
    let order_buy_amount: eth::U256 = 10.ether().into_wei();
    let quote = PriceImprovementQuote {
        sell_amount: 50.ether().into_wei(),
        buy_amount: 40.ether().into_wei(),
        network_fee: 20.ether().into_wei(),
    };
    let network_fee = 10.ether().into_wei();
    let test_case = PriceImprovementTestCase {
        order_side: order::Side::Sell,
        policy_factor: 0.5,
        policy_max_volume_factor: 1.0,
        quote,
        order_sell_amount,
        order_buy_amount,
        network_fee: Some(network_fee),
        executed: order_sell_amount - network_fee,
        // no price improvement since quote provides better conditions
        executed_sell_amount: order_sell_amount,
        executed_buy_amount: order_buy_amount,
    };

    price_improvement_fee_test_case(test_case).await;
}
