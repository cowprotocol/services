use {
    ::alloy::{primitives::U256, providers::Provider},
    e2e::setup::{OnchainComponents, Services, TIMEOUT, run_test, safe::Safe, wait_for_condition},
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{BUY_ETH_ADDRESS, OrderCreation, OrderKind},
        signature::{Signature, hashed_eip712_message},
    },
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let safe = Safe::deploy(trader.clone(), web3.provider.clone()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1000u64.eth(), 1000u64.eth())
        .await;

    token.mint(trader.address(), 4u64.eth()).await;
    safe.exec_alloy_call(
        token
            .approve(onchain.contracts().allowance, 4u64.eth())
            .into_transaction_request(),
    )
    .await;
    token.mint(safe.address(), 4u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 4u64.eth())
        .from(trader.address())
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
        .balanceOf(safe.address())
        .call()
        .await
        .unwrap();
    assert_eq!(balance, ::alloy::primitives::U256::ZERO);
    let mut order = OrderCreation {
        from: Some(safe.address()),
        sell_token: *token.address(),
        sell_amount: 4u64.eth(),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: 3u64.eth(),
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
    onchain.mint_block().await;

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        let safe_balance = web3.provider.get_balance(safe.address()).await.unwrap();
        // the balance is slightly less because of the fee
        U256::from(3_899_000_000_000_000_000_u128) <= safe_balance
            && safe_balance <= U256::from(4_000_000_000_000_000_000_u128)
    };

    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
