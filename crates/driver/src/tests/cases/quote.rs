use crate::{
    domain::competition::order,
    tests::{
        self,
        setup::{self, ab_order, ab_pool, ab_solution},
    },
};

/// Run a matrix of tests for all meaningful combinations of order kind and
/// side, verifying that they get quoted successfully.
#[tokio::test]
#[ignore]
async fn matrix() {
    for side in [order::Side::Buy, order::Side::Sell] {
        for kind in [order::Kind::Market, order::Kind::Limit] {
            let test = tests::setup()
                .name(format!("{side:?} {kind:?}"))
                .pool(ab_pool())
                .order(ab_order().side(side).kind(kind))
                .solution(ab_solution())
                .quote()
                .done()
                .await;

            let quote = test.quote().await;

            quote.ok().amount().interactions();
        }
    }
}

#[tokio::test]
#[ignore]
async fn with_jit_order() {
    let side = order::Side::Sell;
    let kind = order::Kind::Limit;
    let jit_order = setup::JitOrder {
        order: ab_order()
            .kind(order::Kind::Limit)
            .side(side)
            .kind(kind)
            .pre_interaction(setup::blockchain::Interaction {
                address: ab_order().owner,
                calldata: std::iter::repeat(0xab).take(32).collect(),
                inputs: Default::default(),
                outputs: Default::default(),
                internalize: false,
            })
            .no_surplus(),
    };

    let test = tests::setup()
        .name(format!("{side:?} {kind:?}"))
        .pool(ab_pool())
        .jit_order(jit_order)
        .order(ab_order().side(side).kind(kind).no_surplus())
        .solution(ab_solution())
        .quote()
        .done()
        .await;

    let quote = test.quote().await;

    // Check whether the returned data aligns with the expected.
    quote.ok().amount().interactions().jit_order();
}
