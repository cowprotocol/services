use crate::{
    domain::competition::order,
    tests::{
        self,
        setup::{
            ab_adjusted_pool,
            ab_liquidity_quote,
            ab_order,
            ab_solution,
            ExecutedOrderAmounts,
            FeePolicy,
            Test,
        },
    },
};

async fn protocol_fee_test_case(
    side: order::Side,
    fee_policy: FeePolicy,
    expected_amounts: ExecutedOrderAmounts,
) {
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let quote = ab_liquidity_quote()
        .sell_amount(50000000000000000000u128.into())
        .buy_amount(40000000000000000000u128.into());
    let pool = ab_adjusted_pool(quote);
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50000000000000000000u128.into())
        .side(side)
        .solver_fee(Some(10000000000000000000u128.into()))
        .fee_policy(fee_policy)
        .executed_price(40000000000000000000u128.into())
        .executed_amounts(expected_amounts);
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
    let side = order::Side::Buy;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let expected = ExecutedOrderAmounts {
        sell: 100000000000000000000u128.into(),
        buy: 40000000000000000000u128.into(),
    };

    protocol_fee_test_case(side, fee_policy, expected).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let side = order::Side::Sell;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let expected = ExecutedOrderAmounts {
        sell: 50000000000000000000u128.into(),
        buy: 20000000002000000000u128.into(),
    };

    protocol_fee_test_case(side, fee_policy, expected).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_capped() {
    let side = order::Side::Buy;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let expected = ExecutedOrderAmounts {
        sell: 55000000000000000000u128.into(),
        buy: 40000000000000000000u128.into(),
    };

    protocol_fee_test_case(side, fee_policy, expected).await;
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_capped() {
    let side = order::Side::Sell;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let expected = ExecutedOrderAmounts {
        sell: 50000000000000000000u128.into(),
        buy: 35000000000000000000u128.into(),
    };

    protocol_fee_test_case(side, fee_policy, expected).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let side = order::Side::Buy;
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let expected = ExecutedOrderAmounts {
        sell: 75000000000000000000u128.into(),
        buy: 40000000000000000000u128.into(),
    };

    protocol_fee_test_case(side, fee_policy, expected).await;
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let side = order::Side::Sell;
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let expected = ExecutedOrderAmounts {
        sell: 50000000000000000000u128.into(),
        buy: 15000000000000000000u128.into(),
    };

    protocol_fee_test_case(side, fee_policy, expected).await;
}
