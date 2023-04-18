use crate::tests::{setup, setup::new::Order};

/// Test that the /solve endpoint calculates the correct score.
#[tokio::test]
#[ignore]
async fn test() {
    let test = setup()
        .tenderly(setup::new::Tenderly {
            username: "gp-v2".to_owned(),
            project: "niksa-".to_owned(),
            key: "LU0ldRKOdvSKsooOFYGdEOK6Vpy2AFOL".to_owned(),
        })
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

    let solve = test.solve().await;

    solve.ok().score(-51517992626182.0);
}
