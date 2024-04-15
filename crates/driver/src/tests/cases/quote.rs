use {
    crate::{
        domain::{competition::order, eth},
        tests::{
            self,
            setup::{ab_order, ab_pool, ab_solution, TRADER_ADDRESS},
        },
    },
    std::str::FromStr,
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
                .order(
                    ab_order()
                        .side(side)
                        .kind(kind)
                        .owner(eth::H160::from_str(TRADER_ADDRESS).unwrap()),
                )
                .solution(ab_solution())
                .quote()
                .done()
                .await;

            let quote = test.quote().await;

            quote.ok().amount().interactions();
        }
    }
}
