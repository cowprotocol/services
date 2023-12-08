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

    test.solve().await.ok().orders(&[eth_order().name]);
    test.settle().await.ok().await.eth_order_executed().await;
}
