use {
    crate::{
        helpers,
        onchain_components::{to_wei, OnchainComponents},
        services::{get_auction, API_HOST},
        tx,
    },
    ethcontract::prelude::U256,
    model::{
        order::{OrderBuilder, OrderKind},
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
async fn local_node_onchain_settlement() {
    crate::local_node::test(onchain_settlement).await;
}

async fn onchain_settlement(web3: Web3) {
    helpers::init().await;

    crate::services::clear_database().await;
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(101)).await;
    token_b.mint(trader_b.address(), to_wei(51)).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        token_b.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(101))
    );
    tx!(
        trader_b.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(51))
    );

    crate::services::start_autopilot(onchain.contracts(), &[]);
    crate::services::start_api(onchain.contracts(), &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_a)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    let order_b = OrderBuilder::default()
        .with_sell_token(token_b.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(40))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::EthSign,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_b)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let balance = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());

    tracing::info!("Waiting for trade.");
    crate::services::start_old_driver(onchain.contracts(), solver.private_key(), &[]);
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != 0.into() };
    crate::services::wait_for_condition(Duration::from_secs(10), trade_happened)
        .await
        .unwrap();

    // Check matching
    let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(balance >= order_a.data.buy_amount);
    let balance = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert!(balance >= order_b.data.buy_amount);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { get_auction().await.unwrap().auction.orders.is_empty() };
    crate::services::wait_for_condition(Duration::from_secs(10), auction_is_empty)
        .await
        .unwrap();
}
