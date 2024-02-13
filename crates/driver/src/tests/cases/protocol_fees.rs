use crate::{
    domain::competition::order,
    tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution, FeePolicy, OrderQuote},
    },
};
use crate::domain::eth;

#[tokio::test]
#[ignore]
async fn protocol_fee() {
    for side in [order::Side::Buy, order::Side::Sell] {
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
            let test = tests::setup()
                .name(format!("Protocol Fee: {side:?} {fee_policy:?}"))
                .pool(ab_pool())
                .order(
                    ab_order()
                        .kind(order::Kind::Limit)
                        .side(side)
                        .solver_fee(Some(10000000000000000000u128.into()))
                        .quote(OrderQuote {
                            sell_amount: ab_order().sell_amount.checked_add(to_wei(1)).unwrap(),
                            buy_amount: ab_order().sell_amount.checked_sub(to_wei(2)).unwrap(),
                        })
                        // .set_surplus(2.into())
                        .fee_policy(fee_policy),
                )
                .solution(ab_solution())
                .done()
                .await;

            test.solve().await.ok().orders(&[ab_order().name]);
        }
    }
}

pub fn to_wei(base: u32) -> eth::U256 {
    eth::U256::from(base) * eth::U256::exp10(18)
}
