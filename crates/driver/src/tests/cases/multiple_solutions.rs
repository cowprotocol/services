use crate::tests::{
    setup,
    setup::new::{Order, Solution},
};

/// Test that the best-scoring solution is picked when the /solve endpoint
/// returns multiple valid solutions.
#[tokio::test]
#[ignore]
async fn valid() {
    let test = setup()
        .pool(
            "A",
            1000000000000000000000u128.into(),
            "B",
            600000000000u64.into(),
        )
        .order(Order {
            name: "example order",
            amount: 500000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            ..Default::default()
        })
        .solution(Solution::Valid, &["example order"])
        .solution(
            Solution::LowerScore {
                additional_calldata: 5,
            },
            &["example order"],
        )
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().score(-46008923437586.0);
}

/// Test that the invalid solution is discarded when the /solve endpoint
/// returns multiple solutions.
#[tokio::test]
#[ignore]
async fn invalid() {
    let test = setup()
        .pool(
            "A",
            1000000000000000000000u128.into(),
            "B",
            600000000000u64.into(),
        )
        .order(Order {
            name: "example order",
            amount: 500000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            ..Default::default()
        })
        .solution(Solution::Valid, &["example order"])
        .solution(
            Solution::LowerScore {
                additional_calldata: 5,
            },
            &["example order"],
        )
        .solution(Solution::InvalidCalldata, &["example order"])
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().score(-42605070870340.0);
}
