use {
    ::alloy::primitives::U256,
    e2e::{
        nodes::local_node::TestNodeApi,
        setup::{eth, *},
    },
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        signature::EcdsaSigningScheme,
    },
    orderbook::{
        api::IntoWarpReply,
        orderbook::{OrderCancellationError, OrderReplacementError},
    },
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

#[tokio::test]
#[ignore]
async fn local_node_try_replace_executed_order() {
    run_test(try_replace_unreplaceable_order_test).await;
}

async fn try_replace_unreplaceable_order_test(web3: Web3) {
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
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            eth(1000),
            eth(1000),
            U256::ZERO,
            U256::ZERO,
            solver.address().into_alloy(),
            U256::MAX,
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance.into_alloy(), eth(15))
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // disable auto mining to prevent order being immediately executed
    web3.api::<TestNodeApi<_>>()
        .set_automine_enabled(false)
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(10),
        buy_token: token_b.address().into_legacy(),
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
    let balance_before = token_a
        .balanceOf(trader.address().into_alloy())
        .call()
        .await
        .unwrap();
    onchain.mint_block().await;
    let order_id = services.create_order(&order).await.unwrap();

    // mine 1 block to trigger auction
    onchain.mint_block().await;

    tracing::info!("Waiting for the old order to be bid on");
    wait_for_condition(TIMEOUT, || async {
        // here don't wait for the order to be filled, just for it to be bid on
        // so we can make sure that such an order cannot be replaced anymore
        services.get_latest_solver_competition().await.is_ok()
    })
    .await
    .unwrap();

    // Replace order
    let new_order = OrderCreation {
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(3),
        buy_token: token_b.address().into_legacy(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: OrderCreationAppData::Full {
            full: format!(
                r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{order_id}"}}}}}}"#
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

    // Continue automining so our order can be executed
    web3.api::<TestNodeApi<_>>()
        .set_automine_enabled(true)
        .await
        .expect("Must be able to disable auto-mining");

    tracing::info!("Waiting for the old order to be executed");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_a
            .balanceOf(trader.address().into_alloy())
            .call()
            .await
            .unwrap();
        balance_before.saturating_sub(balance_after) == eth(10)
            && !services.get_trades(&order_id).await.unwrap().is_empty()
    })
    .await
    .unwrap();

    // post replacement order again, this time it will already be executed and
    // therefore should be rejected for a different reason
    let response = services.create_order(&new_order).await;
    let (error_code, error_message) = response.err().unwrap();

    assert_eq!(error_code, StatusCode::BAD_REQUEST);
    let expected_response = OrderCancellationError::OrderFullyExecuted
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
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            eth(1000),
            eth(1000),
            U256::ZERO,
            U256::ZERO,
            solver.address().into_alloy(),
            U256::MAX,
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance.into_alloy(), eth(15))
        .from(trader_a.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(onchain.contracts().allowance.into_alloy(), eth(15))
        .from(trader_b.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    let order = OrderCreation {
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(10),
        buy_token: token_b.address().into_legacy(),
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
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(3),
        buy_token: token_b.address().into_legacy(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: OrderCreationAppData::Full {
            full: format!(
                r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{order_id}"}}}}}}"#
            ),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    let balance_before = token_a
        .balanceOf(trader_a.address().into_alloy())
        .call()
        .await
        .unwrap();
    let response = services.create_order(&new_order).await;
    let (error_code, _) = response.err().unwrap();
    assert_eq!(error_code, StatusCode::UNAUTHORIZED);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_a
            .balanceOf(trader_a.address().into_alloy())
            .call()
            .await
            .unwrap();
        balance_before.saturating_sub(balance_after) == eth(10)
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
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            eth(1000),
            eth(1000),
            U256::ZERO,
            U256::ZERO,
            solver.address().into_alloy(),
            U256::MAX,
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance.into_alloy(), eth(15))
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // disble solver to prevent orders from being settled while we
    // want to replace them
    onchain.set_solver_allowed(solver.address(), false).await;

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                // To avoid race conditions we have to start the protocol
                // with the solver being banned. To allow us to still create
                // orders we override the quote verification to be disabled.
                api: vec!["--quote-verification=prefer".into()],
                ..Default::default()
            },
            solver.clone(),
        )
        .await;

    let balance_before = token_a
        .balanceOf(trader.address().into_alloy())
        .call()
        .await
        .unwrap();
    let order = OrderCreation {
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(10),
        buy_token: token_b.address().into_legacy(),
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
    let order_id = services.create_order(&order).await.unwrap();

    let app_data = format!(
        r#"{{
              "version":"1.1.0",
                  "metadata":{{
                      "replacedOrder":{{
                          "uid":"{order_id}"
                      }},
                      "customStuff": 20
                  }}
              }}"#
    );

    // Replace order
    let new_order = OrderCreation {
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(3),
        buy_token: token_b.address().into_legacy(),
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
    let new_order_uid = services.create_order(&new_order).await.unwrap();

    {
        // assert that the new order has the expected appdata
        let new_order = services.get_order(&new_order_uid).await.unwrap();
        let new_order_appdata = new_order
            .metadata
            .full_app_data
            .expect("valid full appData");
        assert_eq!(new_order_appdata, app_data);
    }

    // Check the previous order is cancelled
    let old_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(old_order.metadata.status, OrderStatus::Cancelled);

    tracing::info!("wait for old order to be removed");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let auction = services.get_auction().await.auction;
        auction.orders.len() == 1 && auction.orders[0].uid == new_order_uid
    })
    .await
    .unwrap();
    // now that the order has been cancelled and the original order
    // is no longer part of the auction we can reenable the solver
    onchain.set_solver_allowed(solver.address(), true).await;

    // Drive solution to verify that new order can be settled
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_a
            .balanceOf(trader.address().into_alloy())
            .call()
            .await
            .unwrap();
        onchain.mint_block().await;
        balance_before.saturating_sub(balance_after) == eth(3)
    })
    .await
    .unwrap();
}
