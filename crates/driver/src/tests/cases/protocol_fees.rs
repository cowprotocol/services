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
    surplus_factor: eth::U256,
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
        .surplus(test_case.surplus_factor)
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
    let order_buy_amount = 40.ether().into_wei();
    let order_sell_amount = 50.ether().into_wei();
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        executed_sell_amount: 75.ether().into_wei(),
        executed_buy_amount: order_buy_amount,
        surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
    let order_sell_amount = 50.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        executed_sell_amount: order_sell_amount,
        executed_buy_amount: 30.ether().into_wei(),
        surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 55.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
        surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 36.ether().into_wei(),
        surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 75.ether().into_wei(),
        executed_buy_amount: 40.ether().into_wei(),
        surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
        network_fee: Some(10.ether().into_wei()),
        quote_sell_amount: 50.ether().into_wei(),
        quote_buy_amount: 40.ether().into_wei(),
        executed: 40.ether().into_wei(),
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 20.ether().into_wei(),
        surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
    let order_sell_amount = 50.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let quote_network_fee = 20.ether().into_wei();
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: order_sell_amount,
            buy_amount: order_buy_amount / DEFAULT_SURPLUS_FACTOR,
            network_fee: quote_network_fee,
        },
    };
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(1.ether().into_wei()),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        // order sell amount + quote network fee * factor
        executed_sell_amount: 60.ether().into_wei(),
        executed_buy_amount: order_buy_amount,
        surplus_factor: 1.into(),
    };

    protocol_fee_test_case(test_case).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_out_of_market_order() {
    let order_sell_amount = 50.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let fee_policy = FeePolicy::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 1.0,
        quote: PriceImprovementQuote {
            sell_amount: order_sell_amount * DEFAULT_SURPLUS_FACTOR,
            buy_amount: order_buy_amount,
            network_fee: 20.ether().into_wei(),
        },
    };
    let network_fee = 10.ether().into_wei();
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(network_fee),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_sell_amount - network_fee,
        executed_sell_amount: order_sell_amount,
        // order buy amount - quote network fee * factor (in sell token)
        executed_buy_amount: 32.ether().into_wei(),
        surplus_factor: 1.into(),
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
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            network_fee: 20.ether().into_wei(),
        },
    };
    let order_sell_amount = 100.ether().into_wei();
    let order_buy_amount = 40.ether().into_wei();
    let network_fee = 10.ether().into_wei();
    let test_case = TestCase {
        order_side: order::Side::Buy,
        fee_policy,
        order_sell_amount,
        network_fee: Some(network_fee),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_buy_amount,
        // todo: no price improvement since quote provides better conditions, but the fee isn't
        // considered
        executed_sell_amount: order_sell_amount,
        executed_buy_amount: order_buy_amount,
        surplus_factor: 1.into(),
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
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            network_fee: 20.ether().into_wei(),
        },
    };
    let order_sell_amount: eth::U256 = 50.ether().into_wei();
    let order_buy_amount: eth::U256 = 10.ether().into_wei();
    let network_fee = 10.ether().into_wei();
    let test_case = TestCase {
        order_side: order::Side::Sell,
        fee_policy,
        order_sell_amount,
        network_fee: Some(network_fee),
        quote_sell_amount: order_sell_amount,
        quote_buy_amount: order_buy_amount,
        executed: order_sell_amount - network_fee,
        // todo: no price improvement since quote provides better conditions, but the fee isn't
        // considered
        executed_sell_amount: 50.ether().into_wei(),
        executed_buy_amount: 10.ether().into_wei(),
        surplus_factor: 1.into(),
    };

    protocol_fee_test_case(test_case).await;
}
