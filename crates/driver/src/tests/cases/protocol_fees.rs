use crate::{
    domain::{competition::order, eth},
    tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution, FeePolicy, Order, OrderQuote, Pool},
    },
};

#[tokio::test]
#[ignore]
async fn protocol_fee() {
    for side in [/* order::Side::Buy, */ order::Side::Sell] {
        for fee_policy in [
            FeePolicy::Surplus {
                factor: 0.5,
                // high enough so we don't get capped by volume fee
                max_volume_factor: 1.0,
            },
            FeePolicy::Surplus {
                factor: 0.5,
                // low enough so we get capped by volume fee
                max_volume_factor: 0.1,
            },
        ] {
            let order = Order {
                    sell_amount: to_wei(10),
                    ..ab_order()
                }
                .kind(order::Kind::Limit)
                .side(side)
                .no_surplus()
                // .solver_fee(Some(10000000000000000000u128.into()))
                .solver_fee(Some(0u128.into()))
                .fee_policy(fee_policy.clone());
            let quote = match order.side {
                order::Side::Sell => OrderQuote {
                    sell_amount: to_wei(10),
                    buy_amount: to_wei(9),
                },
                order::Side::Buy => panic!("buy is not supported"),
            };
            let pool = Pool {
                amount_a: to_wei(100),
                ..ab_pool()
            };
            let pool = adjust_pool_reserve_b(pool, &quote);
            let order = order.quote(quote);
            let test = tests::setup()
                .name(format!("Protocol Fee: {side:?} {fee_policy:?}"))
                .pool(pool)
                .order(order)
                .solution(ab_solution())
                .done()
                .await;

            test.solve().await.ok().orders(&[ab_order().name]);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_adjust_pool_reserve_b() {
    let pool = Pool {
        amount_a: to_wei(100),
        ..ab_pool()
    };
    let quote = OrderQuote {
        sell_amount: to_wei(10),
        buy_amount: to_wei(9),
    };

    let pool = adjust_pool_reserve_b(pool, &quote);

    assert_eq!(pool.amount_b, to_wei(99));
    assert_eq!(pool.amount_a, to_wei(100));
}

fn adjust_pool_reserve_b(pool: Pool, quote: &OrderQuote) -> Pool {
    let reserve_a_plus_sell = pool.amount_a.checked_add(quote.sell_amount).unwrap();
    let reserve_b = reserve_a_plus_sell
        .checked_mul(quote.buy_amount)
        .unwrap()
        .checked_div(quote.sell_amount)
        .unwrap();
    Pool {
        amount_b: reserve_b,
        ..pool
    }
}

fn to_wei(base: u32) -> eth::U256 {
    eth::U256::from(base) * eth::U256::exp10(18)
}
