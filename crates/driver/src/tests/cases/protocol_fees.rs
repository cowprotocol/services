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
            PriceImprovementQuote,
            Test,
        },
    },
};

const DEFAULT_SURPLUS_FACTOR: u64 = 2;

struct TestCase {
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

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_not_capped() {
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let order_buy_amount = 40000000000000000000u128.into();
    let order_sell_amount = 50000000000000000000u128.into();
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        executed_sell_amount: 75000000000000000000u128.into(),
        executed_buy_amount: order_buy_amount,
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
    let order_sell_amount = 50000000000000000000u128.into();
    let order_buy_amount = 40000000000000000000u128.into();
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        executed_sell_amount: order_sell_amount,
        executed_buy_amount: 30000000000000000000u128.into(),
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
        network_fee: Some(10000000000000000000u128.into()),
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
        network_fee: Some(10000000000000000000u128.into()),
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
        network_fee: Some(10000000000000000000u128.into()),
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
        network_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: 50000000000000000000u128.into(),
        quote_buy_amount: 40000000000000000000u128.into(),
        executed: 40000000000000000000u128.into(),
        executed_sell_amount: 50000000000000000000u128.into(),
        executed_buy_amount: 15000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}

// Price Improvement policy fee tests.
// Out of market order could be defined as:
//   (order.sell + order.fee) * quote.buy < (quote.sell + quote.fee) * order.buy
// In the following tests Limit orders are used only, where order fee is 0. The
// amount values are adjusted to respect the definition.

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_out_of_market_order() {
    let order_sell_amount = 50000000000000000000u128.into();
    let order_buy_amount = 40000000000000000000u128.into();
    let quote_network_fee = 20000000000000000000u128.into();
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: order_sell_amount,
            // Since order surplus factor is 2.0, order's buy amount becomes x0.5 from the original
            // value. Quote's buy amount is selected to be equal to the adjusted order's
            // buy amount.
            buy_amount: order_buy_amount / DEFAULT_SURPLUS_FACTOR,
            // Quote's fee is high enough to make the order's conditions better.
            network_fee: quote_network_fee,
        },
    };
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10000000000000000000u128.into()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        // Executed values should be the same as for the surplus policy fee.
        // ???
        executed_sell_amount: 75000000000000000000u128.into(),
        executed_buy_amount: order_buy_amount,
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order() {
    let order_sell_amount = 50000000000000000000u128.into();
    let order_buy_amount = 40000000000000000000u128.into();
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            // Since order surplus factor is 2.0, order's sell amount becomes x2 from the original
            // value. Quote's sell amount is selected to be equal to the adjusted order's
            // buy amount.
            sell_amount: order_sell_amount * DEFAULT_SURPLUS_FACTOR,
            buy_amount: order_buy_amount,
            network_fee: 20000000000000000000u128.into(),
        },
    };
    let network_fee = 10000000000000000000u128.into();
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(network_fee),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_sell_amount - network_fee,
        executed_sell_amount: order_sell_amount,
        // executed values should be the same as for the surplus policy fee
        // ???
        executed_buy_amount: 30000000000000000000u128.into(),
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
            // Since order surplus factor is 2.0, order's buy amount will be x0.5.
            // To make it inside market price, the quote buy amount should be higher than the
            // adjusted order's value.
            buy_amount: 40000000000000000000u128.into(),
            network_fee: 20000000000000000000u128.into(),
        },
    };
    let order_sell_amount = 50000000000000000000u128.into();
    let order_buy_amount = 40000000000000000000u128.into();
    let network_fee = 10000000000000000000u128.into();
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(network_fee),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        // quote.sell_amount + factor * quote.network_fee
        // ???
        executed_sell_amount: 60000000000000000000u128.into(),
        executed_buy_amount: order_buy_amount,
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
            // Since order surplus factor is 2.0, order's sell amount will be x2.
            // To make it inside market price, the quote sell amount should be lower than the
            // adjusted order's value.
            sell_amount: 500000000000000000000u128.into(),
            buy_amount: 40000000000000000000u128.into(),
            network_fee: 20000000000000000000u128.into(),
        },
    };
    let order_sell_amount: eth::U256 = 50000000000000000000u128.into();
    let order_buy_amount: eth::U256 = 40000000000000000000u128.into();
    let network_fee = 10000000000000000000u128.into();
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(network_fee),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_sell_amount - network_fee,
        executed_sell_amount: order_sell_amount,
        // ???
        executed_buy_amount: 30000000000000000000u128.into(),
    };

    protocol_fee_test_case(test_case).await;
}
