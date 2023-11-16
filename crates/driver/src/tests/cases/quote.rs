use crate::{
    domain::competition::order,
    tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution},
    },
};

/// Run a matrix of tests for all meaningful combinations of order kind and
/// side, verifying that they get quoted successfully.
#[tokio::test]
#[ignore]
async fn matrix() {
    for side in [order::Side::Buy, order::Side::Sell] {
        for kind in [order::Kind::Market, order::Kind::Limit] {
            // need to execute sequentially to make sure the Test struct is created
            // correctly for each test (specifially the deadline, since we don't want to
            // build deadline for all tests, and then execute tests sequentially, which
            // would make some deadlines expired before even starting the test)
            futures::executor::block_on(async {
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
            });
        }
    }
}
