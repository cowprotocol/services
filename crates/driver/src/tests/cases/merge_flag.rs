use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution, cd_order, cd_pool, cd_solution, Solution},
};

// tests that flagging for settlement merge is possible
#[tokio::test]
async fn possible() {
        let ab_order = ab_order();
        let cd_order = cd_order();
        let test = setup()
        .pool(cd_pool())
        .pool(ab_pool())
        .order(ab_order.clone())
        .order(cd_order.clone())
        .solution(cd_solution())
        .solution(ab_solution())
        .done()
        .await;
    test.solve().await.ok().orders(&[ab_order, cd_order]);
    test.reveal().await.ok().calldata();



}


// tests that flag is not valid when solver already solved for multiple settlements
// [tokio::test]
async fn impossible() {
    todo!()
}
