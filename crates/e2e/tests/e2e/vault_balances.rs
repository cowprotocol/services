use {
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_vault_balances() {
    run_test(vault_balances).await;
}

async fn vault_balances(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token.mint(trader.address(), to_wei(10)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token.approve(onchain.contracts().balancer_vault.address(), to_wei(10))
    );
    tx!(
        trader.account(),
        onchain.contracts().balancer_vault.set_relayer_approval(
            trader.address(),
            onchain.contracts().allowance,
            true
        )
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    // Place Orders
    let order = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: token.address(),
        sell_amount: to_wei(10),
        sell_token_balance: SellTokenSource::External,
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(8),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();
    let balance_before = onchain
        .contracts()
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        services.get_auction().await.auction.orders.len() == 1
    })
    .await
    .unwrap();
    wait_for_condition(TIMEOUT, || async {
        services.get_auction().await.auction.orders.is_empty()
    })
    .await
    .unwrap();

    // Check matching
    let balance = token
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch token balance");
    assert_eq!(balance, U256::zero());

    let balance_after = onchain
        .contracts()
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(8));
}
