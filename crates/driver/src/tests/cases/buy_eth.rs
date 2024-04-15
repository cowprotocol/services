use {
    crate::{
        domain::eth,
        tests::{
            setup,
            setup::{eth_order, eth_solution, weth_pool, TRADER_ADDRESS},
        },
    },
    std::str::FromStr,
};

/// Test that buying ETH automatically wraps and unwraps WETH.
#[tokio::test]
#[ignore]
async fn test() {
    let order = eth_order().owner(eth::H160::from_str(TRADER_ADDRESS).unwrap());
    let test = setup()
        .pool(weth_pool())
        .order(order.clone())
        .solution(eth_solution())
        .done()
        .await;

    test.solve().await.ok().orders(&[order]);
    test.settle().await.ok().await.eth_order_executed().await;
}
