use crate::tests::{
    setup,
    setup::new::{Balance, Order, Solution},
};

/// Test that settlements can be merged.
#[tokio::test]
#[ignore]
async fn possible() {
    let test = setup()
        .pool(
            "A",
            1000000000000000000000u128.into(),
            "B",
            600000000000u64.into(),
        )
        .pool(
            "C",
            1000000000000000000000u128.into(),
            "D",
            600000000000u64.into(),
        )
        .order(Order {
            name: "first order",
            amount: 500000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            ..Default::default()
        })
        .order(Order {
            name: "second order",
            amount: 400000000000000000u64.into(),
            sell_token: "C",
            buy_token: "D",
            ..Default::default()
        })
        .solution(Solution::Valid, &["first order"])
        .solution(Solution::Valid, &["second order"])
        .done()
        .await;

    let id = test.solve().await.ok().solution_id();
    let settle = test.settle(id).await;

    // Even though the solver returned two solutions, the executed settlement is a
    // combination of the two, meaning the settlements were merged successfully.
    settle
        .ok()
        .await
        .balance("A", Balance::SmallerBy(500000000000000000u64.into()))
        .await
        .balance("B", Balance::Greater)
        .await
        .balance("C", Balance::SmallerBy(400000000000000000u64.into()))
        .await
        .balance("D", Balance::Greater)
        .await;
}

/// Test that settlements are not merged if the clearing prices don't permit it.
#[tokio::test]
#[ignore]
async fn impossible() {
    let test = setup()
        .pool(
            "A",
            1000000000000000000000u128.into(),
            "B",
            600000000000u64.into(),
        )
        .order(Order {
            name: "first order",
            amount: 500000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            ..Default::default()
        })
        .order(Order {
            name: "second order",
            amount: 400000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            ..Default::default()
        })
        // These two solutions result in different clearing prices and can't be merged.
        .solution(Solution::Valid, &["first order"])
        .solution(Solution::LowerScore { additional_calldata: 3 }, &["second order"])
        .done()
        .await;

    let id = test.solve().await.ok().solution_id();
    let settle = test.settle(id).await;

    // Only one solution is executed.
    settle
        .ok()
        .await
        .balance("A", Balance::SmallerBy(500000000000000000u64.into()))
        .await
        .balance("B", Balance::Greater)
        .await;
}
