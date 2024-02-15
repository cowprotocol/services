use crate::{
    domain::competition::order,
    tests::{
        self,
        setup::{ab_order, ab_order_quote, ab_pool, ab_solution, ExpectedOrder, FeePolicy},
    },
};

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_not_capped() {
    let side = order::Side::Buy;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let order = ab_order()
        .kind(order::Kind::Limit)
        .side(side)
        .solver_fee(Some(10000000000000000000u128.into()))
        .fee_policy(fee_policy)
        .quote(ab_order_quote());
    let test = tests::setup()
        .name(test_name)
        .pool(ab_pool())
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 1000000000000000000000u128.into(),
        executed_buy_amount: 2989509729399894152u128.into(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_sell_order_not_capped() {
    let side = order::Side::Buy;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // high enough so we don't get capped by volume fee
        max_volume_factor: 1.0,
    };
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let order = ab_order()
        .kind(order::Kind::Limit)
        .side(side)
        .solver_fee(Some(10000000000000000000u128.into()))
        .fee_policy(fee_policy)
        .quote(ab_order_quote());
    let test = tests::setup()
        .name(test_name)
        .pool(ab_pool())
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 1000000000000000000000u128.into(),
        executed_buy_amount: 2989509729399894152u128.into(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
}

#[tokio::test]
#[ignore]
async fn surplus_protocol_fee_buy_order_capped() {
    let side = order::Side::Sell;
    let fee_policy = FeePolicy::Surplus {
        factor: 0.5,
        // low enough so we get capped by volume fee
        max_volume_factor: 0.1,
    };
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let order = ab_order()
        .kind(order::Kind::Limit)
        .side(side)
        .solver_fee(Some(10000000000000000000u128.into()))
        .fee_policy(fee_policy)
        .quote(ab_order_quote());
    let test = tests::setup()
        .name(test_name)
        .pool(ab_pool())
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 500000000000000000000u128.into(),
        executed_buy_amount: 2684457716195823320u128.into(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
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
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let order = ab_order()
        .kind(order::Kind::Limit)
        .side(side)
        .solver_fee(Some(10000000000000000000u128.into()))
        .fee_policy(fee_policy)
        .quote(ab_order_quote());
    let test = tests::setup()
        .name(test_name)
        .pool(ab_pool())
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 500000000000000000000u128.into(),
        executed_buy_amount: 2684457716195823320u128.into(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
}
