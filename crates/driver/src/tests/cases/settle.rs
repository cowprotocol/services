use crate::tests::{
    setup,
    setup::new::{Balance, Order},
};

/// Test that the /settle endpoint broadcasts a valid settlement transaction.
#[tokio::test]
#[ignore]
async fn test() {
    let test = setup()
        .pool(
            "A",
            1000000000000000000000u128.into(),
            "B",
            600000000000u64.into(),
        )
        .order(Order {
            amount: 500000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            ..Default::default()
        })
        .done()
        .await;

    let id = test.solve().await.ok().solution_id();
    let settle = test.settle(id).await;

    settle
        .ok()
        .await
        .balance("A", Balance::SmallerBy(500000000000000000u64.into()))
        .await
        .balance("B", Balance::Greater)
        .await;
}
