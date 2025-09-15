use {
    ::alloy::{primitives::U256, providers::Provider},
    e2e::setup::{
        OnchainComponents,
        Services,
        TIMEOUT,
        run_test,
        safe::Safe,
        to_wei,
        wait_for_condition,
    },
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    model::{
        order::{BUY_ETH_ADDRESS, OrderCreation, OrderKind},
        signature::{Signature, hashed_eip712_message},
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
    let safe = Safe::deploy(trader.clone(), web3.alloy.clone()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1000), to_wei(1000))
        .await;

    token.mint(trader.address(), to_wei(4)).await;
    safe.exec_alloy_call(
        token
            .approve(
                onchain.contracts().allowance.into_alloy(),
                to_wei(4).into_alloy(),
            )
            .into_transaction_request(),
    )
    .await;
    token.mint(safe.address().into_legacy(), to_wei(4)).await;

    token
        .approve(
            onchain.contracts().allowance.into_alloy(),
            to_wei(4).into_alloy(),
        )
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    tracing::info!("Placing order");
    let balance = onchain
        .contracts()
        .weth
        .balance_of(safe.address().into_legacy())
        .call()
        .await
        .unwrap();
    assert_eq!(balance, 0.into());
    let mut order = OrderCreation {
        from: Some(safe.address().into_legacy()),
        sell_token: token.address().into_legacy(),
        sell_amount: to_wei(4),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        partially_fillable: true,
        kind: OrderKind::Sell,
        receiver: Some(safe.address().into_legacy()),
        ..Default::default()
    };
    order.signature = Signature::Eip1271(safe.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));
    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        let safe_balance = web3.alloy.get_balance(safe.address()).await.unwrap();
        // the balance is slightly less because of the fee
        U256::from(3_899_000_000_000_000_000_u128) <= safe_balance
            && safe_balance <= U256::from(4_000_000_000_000_000_000_u128)
    };

    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
