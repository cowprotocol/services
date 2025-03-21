use {
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        signature::EcdsaSigningScheme,
    },
    orderbook::{api::IntoWarpReply, orderbook::OrderReplacementError},
    reqwest::StatusCode,
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    warp::reply::Reply,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_replace_order() {
    run_test(single_replace_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_try_replace_someone_else_order() {
    run_test(try_replace_someone_else_order_test).await;
}

// TODO: The test is not ideal, as we actually want to test the replacement of
// active orders as soon as they are being bid on, even before they are
// executed. For that we would need the ability to mock the driver in e2e tests.
#[tokio::test]
#[ignore]
async fn local_node_try_replace_executed_order() {
    run_test(try_replace_executed_order_test).await;
}

async fn try_replace_executed_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader.address(), to_wei(30)).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
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
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(15))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let balance_before = token_a.balance_of(trader.address()).call().await.unwrap();
    onchain.mint_block().await;
    let order_id = services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for the old order to be executed");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_a.balance_of(trader.address()).call().await.unwrap();
        balance_before.saturating_sub(balance_after) == to_wei(10)
    })
    .await
    .unwrap();

    // Replace order
    let new_order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(3),
        buy_token: token_b.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: OrderCreationAppData::Full {
            full: format!(
                r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{}"}}}}}}"#,
                order_id
            ),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let response = services.create_order(&new_order).await;
    let (error_code, error_message) = response.err().unwrap();

    assert_eq!(error_code, StatusCode::BAD_REQUEST);

    let expected_response = OrderReplacementError::OldOrderActivelyBidOn
        .into_warp_reply()
        .into_response()
        .into_body();
    let expected_body_bytes = warp::hyper::body::to_bytes(expected_response)
        .await
        .unwrap();
    let expected_body = String::from_utf8(expected_body_bytes.to_vec()).unwrap();
    assert_eq!(error_message, expected_body);
}

async fn try_replace_someone_else_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(30)).await;
    token_a.mint(trader_b.address(), to_wei(30)).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
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
        token_a.approve(onchain.contracts().allowance, to_wei(15))
    );
    tx!(
        trader_b.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(15))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        partially_fillable: false,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order).await.unwrap();

    // Replace order
    let new_order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(3),
        buy_token: token_b.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: OrderCreationAppData::Full {
            full: format!(
                r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{}"}}}}}}"#,
                order_id
            ),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    let balance_before = token_a.balance_of(trader_a.address()).call().await.unwrap();
    let response = services.create_order(&new_order).await;
    let (error_code, _) = response.err().unwrap();
    assert_eq!(error_code, StatusCode::UNAUTHORIZED);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_a.balance_of(trader_a.address()).call().await.unwrap();
        balance_before.saturating_sub(balance_after) == to_wei(10)
    })
    .await
    .unwrap();
}

async fn single_replace_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader.address(), to_wei(30)).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
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
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(15))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    onchain.mint_block().await;
    let order_id = services.create_order(&order).await.unwrap();

    let app_data = format!(
        r#"{{
              "version":"1.1.0",
                  "metadata":{{
                      "replacedOrder":{{
                          "uid":"{}"
                      }},
                      "customStuff": 20
                  }}
              }}"#,
        order_id
    );

    // Replace order
    let new_order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(3),
        buy_token: token_b.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let balance_before = token_a.balance_of(trader.address()).call().await.unwrap();
    let new_order_uid = services.create_order(&new_order).await.unwrap();

    // Check the previous order is cancelled
    let old_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(old_order.metadata.status, OrderStatus::Cancelled);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_a.balance_of(trader.address()).call().await.unwrap();
        onchain.mint_block().await;
        balance_before.saturating_sub(balance_after) == to_wei(3)
    })
    .await
    .unwrap();

    // Check the previous order is cancelled
    wait_for_condition(TIMEOUT, || async {
        let new_order = services.get_order(&new_order_uid).await.unwrap();
        let new_order_appdata = new_order
            .metadata
            .full_app_data
            .expect("valid full appData");
        new_order_appdata == app_data
    })
    .await
    .unwrap()
}
