use crate::{
    domain::competition::order,
    tests::{
        self,
        cases::IntoWei,
        setup::{ab_liquidity_quote, ab_order, ab_pmm_pool, ab_solution, ExpectedOrder, FeePolicy},
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
    let quote = ab_liquidity_quote()
        .sell_amount(50u32.to_wei())
        .buy_amount(40u32.to_wei());
    let pool = ab_pmm_pool(quote);
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50u32.to_wei())
        .side(side)
        .solver_fee(Some(10u32.to_wei()))
        .fee_policy(fee_policy)
        .executed(40u32.to_wei());
    let test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 100u32.to_wei(),
        executed_buy_amount: 40u32.to_wei(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
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
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let quote = ab_liquidity_quote()
        .sell_amount(50u32.to_wei())
        .buy_amount(40u32.to_wei());
    let pool = ab_pmm_pool(quote);
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50u32.to_wei())
        .side(side)
        .solver_fee(Some(10u32.to_wei()))
        .fee_policy(fee_policy)
        .executed(40u32.to_wei());
    let test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 50u32.to_wei(),
        executed_buy_amount: 20000000002000000000u128.into(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
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
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let quote = ab_liquidity_quote()
        .sell_amount(50u32.to_wei())
        .buy_amount(40u32.to_wei());
    let pool = ab_pmm_pool(quote);
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50u32.to_wei())
        .side(side)
        .solver_fee(Some(10u32.to_wei()))
        .fee_policy(fee_policy)
        .executed(40u32.to_wei());
    let test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 55u32.to_wei(),
        executed_buy_amount: 40u32.to_wei(),
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
    let quote = ab_liquidity_quote()
        .sell_amount(50u32.to_wei())
        .buy_amount(40u32.to_wei());
    let pool = ab_pmm_pool(quote);
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50u32.to_wei())
        .side(side)
        .solver_fee(Some(10u32.to_wei()))
        .fee_policy(fee_policy)
        .executed(40u32.to_wei());
    let test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 50u32.to_wei(),
        executed_buy_amount: 35u32.to_wei(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_buy_order() {
    let side = order::Side::Buy;
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let quote = ab_liquidity_quote()
        .sell_amount(50u32.to_wei())
        .buy_amount(40u32.to_wei());
    let pool = ab_pmm_pool(quote);
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50u32.to_wei())
        .side(side)
        .solver_fee(Some(10u32.to_wei()))
        .fee_policy(fee_policy)
        .executed(40u32.to_wei());
    let test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 75u32.to_wei(),
        executed_buy_amount: 40u32.to_wei(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
}

#[tokio::test]
#[ignore]
async fn volume_protocol_fee_sell_order() {
    let side = order::Side::Sell;
    let fee_policy = FeePolicy::Volume { factor: 0.5 };
    let test_name = format!("Protocol Fee: {side:?} {fee_policy:?}");
    let quote = ab_liquidity_quote()
        .sell_amount(50u32.to_wei())
        .buy_amount(40u32.to_wei());
    let pool = ab_pmm_pool(quote);
    let executed_price = 40u32.to_wei();
    let order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(50u32.to_wei())
        .side(side)
        .solver_fee(Some(10u32.to_wei()))
        .fee_policy(fee_policy)
        .executed(executed_price);
    let test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order)
        .solution(ab_solution())
        .done()
        .await;
    let expected = ExpectedOrder {
        name: ab_order().name,
        executed_sell_amount: 50u32.to_wei(),
        executed_buy_amount: 15u32.to_wei(),
    };

    test.solve().await.ok().expected_orders(&[expected]);
}
