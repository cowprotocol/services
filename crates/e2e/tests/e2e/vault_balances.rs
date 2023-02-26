use {
    crate::{
        helpers,
        onchain_components::{to_wei, OnchainComponents},
        services::{solvable_orders, wait_for_condition, API_HOST},
    },
    ethcontract::prelude::U256,
    model::{
        order::{OrderBuilder, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_vault_balances() {
    crate::local_node::test(vault_balances).await;
}

async fn vault_balances(web3: Web3) {
    helpers::init().await;

    crate::services::clear_database().await;
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
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

    crate::services::start_autopilot(onchain.contracts(), &[]);
    crate::services::start_api(onchain.contracts(), &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    // Place Orders
    let order = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(9))
        .with_sell_token_balance(SellTokenSource::External)
        .with_fee_amount(to_wei(1))
        .with_buy_token(onchain.contracts().weth.address())
        .with_buy_amount(to_wei(8))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);
    let balance_before = onchain
        .contracts()
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 1
    })
    .await
    .unwrap();
    crate::services::start_old_driver(onchain.contracts(), solver.private_key(), &[]);
    crate::services::wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
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
