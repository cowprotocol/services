use {
    e2e::{
        setup::{safe::Safe, *},
        tx,
    },
    ethcontract::U256,
    model::{
        order::{OrderCreation, OrderKind, BUY_ETH_ADDRESS},
        signature::{hashed_eip712_message, Signature},
    },
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let safe = Safe::deploy(trader.clone(), &web3).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1000), to_wei(1000))
        .await;

    token.mint(trader.address(), to_wei(4)).await;
    safe.exec_call(token.approve(onchain.contracts().allowance, to_wei(4)))
        .await;
    token.mint(safe.address(), to_wei(4)).await;
    tx!(
        trader.account(),
        token.approve(onchain.contracts().allowance, to_wei(4))
    );

    tracing::info!("Starting services.");
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    // We force the block to start before the test, so the auction is not cut by the
    // block in the middle of the operations, creating uncertainty
    onchain.mint_block().await;

    tracing::info!("Placing order");
    let balance = onchain
        .contracts()
        .weth
        .balance_of(safe.address())
        .call()
        .await
        .unwrap();
    assert_eq!(balance, 0.into());
    let mut order = OrderCreation {
        from: Some(safe.address()),
        sell_token: token.address(),
        sell_amount: to_wei(4),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        partially_fillable: true,
        kind: OrderKind::Sell,
        receiver: Some(safe.address()),
        ..Default::default()
    };
    order.signature = Signature::Eip1271(safe.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));
    services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        let safe_balance = web3.eth().balance(safe.address(), None).await.unwrap();
        // the balance is slightly less because of the fee
        (3_899_000_000_000_000_000_u128..4_000_000_000_000_000_000_u128)
            .contains(&safe_balance.as_u128())
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
