use {
    ::alloy::providers::ext::AnvilApi,
    e2e::setup::*,
    ethrpc::{
        Web3,
        alloy::{CallBuilderExt, EvmProviderExt},
        block_stream::timestamp_of_current_block_in_seconds,
    },
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_valid_from() {
    run_test(valid_from_test).await;
}

async fn valid_from_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 10u64.eth()).await;
    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let now = timestamp_of_current_block_in_seconds(&web3.provider)
        .await
        .unwrap();
    let valid_from = now + 30;

    let app_data = format!(r#"{{"metadata":{{"validFrom":{valid_from}}}}}"#);
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: now + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    // The order should not be settled while valid_from is in the future.
    // Mine a few blocks and confirm it stays Open.
    for _ in 0..5 {
        onchain.mint_block().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let status = services.get_order(&uid).await.unwrap().metadata.status;
    assert_eq!(
        status,
        OrderStatus::Open,
        "order should not be solvable before valid_from"
    );

    // Advance blockchain time past valid_from.
    web3.provider
        .evm_set_next_block_timestamp(valid_from as u64 + 5)
        .await
        .unwrap();
    web3.provider.evm_mine(None).await.unwrap();

    // Now the order should be settled.
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .map(|o| o.metadata.status == OrderStatus::Fulfilled)
            .unwrap_or(false)
    })
    .await
    .expect("order was not settled after valid_from elapsed");
}
