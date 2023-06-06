use crate::tests::{
    setup,
    setup::{eth_order, eth_solution, weth_pool},
};

/// Test that buying ETH automatically wraps and unwraps WETH.
#[tokio::test]
#[ignore]
async fn test() {
    let test = setup()
        .pool(weth_pool())
        .order(eth_order())
        .solution(eth_solution())
        .done()
        .await;

    let id = test.solve().await.ok().solution_id();
    let settle = test.settle(id).await;

    settle.ok().await.eth_order_executed().await;
}
