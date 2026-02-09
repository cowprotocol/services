use {
    ::alloy::primitives::U256,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::{EcdsaSigningScheme, Signature, SigningScheme},
    },
    number::units::EthUnit,
    orderbook::dto::order::Status,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    // Use a shallow pool to make partial fills easier to setup.
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(10u64.eth(), 10u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 4u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(4u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    tracing::info!("Placing order");
    let balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(balance, U256::ZERO);
    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 4u64.eth(),
        buy_token: *token.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        partially_fillable: true,
        kind: OrderKind::Sell,
        signature: Signature::default_with(SigningScheme::EthSign),
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::EthSign,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    onchain.mint_block().await;

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        !token
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // We expect the partially fillable order to only fill half-way.
    let sell_balance = onchain
        .contracts()
        .weth
        .balanceOf(trader.address())
        .call()
        .await
        .unwrap();
    assert!(
        // Sell balance is strictly less than 2.0 because of the fee.
        (1_999_000_000_000_000_000_u128..2_000_000_000_000_000_000_u128)
            .contains(&u128::try_from(sell_balance).unwrap())
    );
    let buy_balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert!(
        (1_650_000_000_000_000_000_u128..1_670_000_000_000_000_000_u128)
            .contains(&u128::try_from(buy_balance).unwrap())
    );

    let settlement_event_processed = || async {
        onchain.mint_block().await;
        let order = services.get_order(&uid).await.unwrap();
        order.metadata.executed_fee > U256::ZERO
    };
    wait_for_condition(TIMEOUT, settlement_event_processed)
        .await
        .unwrap();

    let tx_hash = services.get_trades(&uid).await.unwrap()[0].tx_hash.unwrap();
    let competition = services.get_solver_competition(tx_hash).await.unwrap();
    assert!(!competition.solutions.is_empty());
    assert!(competition.auction.orders.contains(&uid));
    let latest_competition = services.get_latest_solver_competition().await.unwrap();
    assert_eq!(latest_competition, competition);

    let Status::Traded(solutions) = services.get_order_status(&uid).await.unwrap() else {
        panic!("last status of order was not traded");
    };
    assert_eq!(solutions.len(), 1);
    assert_eq!(solutions[0].solver, "test_solver");
    assert!(solutions[0].executed_amounts.is_some());
}
