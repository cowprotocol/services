use {
    app_data::Hook,
    contracts::GnosisSafe,
    e2e::{
        setup::{safe::Safe, *},
        tx,
        tx_value,
    },
    ethcontract::{Bytes, H160, U256},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::{hashed_eip712_message, EcdsaSigningScheme, Signature},
    },
    reqwest::StatusCode,
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_allowance() {
    run_test(allowance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_signature() {
    run_test(signature).await;
}

#[tokio::test]
#[ignore]
async fn local_node_partial_fills() {
    run_test(partial_fills).await;
}

#[tokio::test]
#[ignore]
async fn local_node_gas_limit() {
    run_test(gas_limit).await;
}

async fn gas_limit(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let cow = onchain
        .deploy_cow_weth_pool(to_wei(1_000_000), to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts and approve relayer
    cow.fund(trader.address(), to_wei(5)).await;
    tx!(
        trader.account(),
        cow.approve(onchain.contracts().allowance, to_wei(5))
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: cow.address(),
        sell_amount: to_wei(4),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [Hook {
                            target: trader.address(),
                            call_data: Default::default(),
                            gas_limit: 10_000_000,
                        }],
                        "post": [],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let error = services.create_order(&order).await.unwrap_err();
    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    assert!(error.1.contains("TooMuchGas"));
}

async fn allowance(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let cow = onchain
        .deploy_cow_weth_pool(to_wei(1_000_000), to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    cow.fund(trader.address(), to_wei(5)).await;

    // Sign a permit pre-interaction for trading.
    let permit = cow
        .permit(&trader, onchain.contracts().allowance, to_wei(5))
        .await;
    // Setup a malicious interaction for setting approvals to steal funds from
    // the settlement contract.
    let steal_cow = hook_for_transaction(
        cow.approve(trader.address(), U256::max_value())
            .from(solver.account().clone())
            .tx,
    )
    .await;
    let steal_weth = hook_for_transaction(
        onchain
            .contracts()
            .weth
            .approve(trader.address(), U256::max_value())
            .from(solver.account().clone())
            .tx,
    )
    .await;

    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: cow.address(),
        sell_amount: to_wei(5),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [permit, steal_cow],
                        "post": [steal_weth],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    let balance = cow.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, to_wei(5));

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        cow.balance_of(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let balance = onchain
        .contracts()
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    assert!(balance >= order.buy_amount);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();

    // Check malicious custom interactions did not work.
    let allowance = cow
        .allowance(
            onchain.contracts().gp_settlement.address(),
            trader.address(),
        )
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::zero());
    let allowance = onchain
        .contracts()
        .weth
        .allowance(
            onchain.contracts().gp_settlement.address(),
            trader.address(),
        )
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::zero());

    // Note that the allowances were set with the `HooksTrampoline` contract!
    // This is OK since the `HooksTrampoline` contract is not used for holding
    // any funds.
    let allowance = cow
        .allowance(onchain.contracts().hooks.address(), trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::max_value());
    let allowance = onchain
        .contracts()
        .weth
        .allowance(onchain.contracts().hooks.address(), trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::max_value());
}

async fn signature(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let chain_id = web3.eth().chain_id().await.unwrap();

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let safe_infra = safe::Infrastructure::new(&web3).await;

    // Prepare the Safe creation transaction, but don't execute it! This will
    // be executed as a pre-hook.
    let safe_creation_builder = safe_infra.factory.create_proxy(
        safe_infra.singleton.address(),
        ethcontract::Bytes(
            safe_infra
                .singleton
                .setup(
                    vec![trader.address()], // owners
                    1.into(),               // threshold
                    H160::default(),        // delegate call
                    Bytes::default(),       // delegate call bytes
                    safe_infra.fallback.address(),
                    H160::default(), // relayer payment token
                    0.into(),        // relayer payment amount
                    H160::default(), // relayer address
                )
                .tx
                .data
                .unwrap()
                .0,
        ),
    );
    let safe_creation = hook_for_transaction(safe_creation_builder.tx.clone()).await;

    // Create a contract instance at the would-be address of the Safe we are
    // creating with the pre-hook.
    let safe_address = safe_creation_builder.clone().view().call().await.unwrap();
    let safe = Safe::deployed(
        chain_id,
        GnosisSafe::at(&web3, safe_address),
        trader.clone(),
    );

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(100_000), to_wei(100_000))
        .await;
    token.mint(safe.address(), to_wei(5)).await;

    // Sign an approval transaction for trading. This will be at nonce 0 because
    // it is the first transaction evah!
    let approval_builder = safe.sign_transaction(
        token.address(),
        token
            .approve(onchain.contracts().allowance, to_wei(5))
            .tx
            .data
            .unwrap()
            .0,
        0.into(),
    );
    let approval = Hook {
        target: approval_builder.tx.to.unwrap(),
        call_data: approval_builder.tx.data.unwrap().0,
        // The contract isn't deployed, so we can't estimate this. Instead, we
        // just guess an amount that should be high enough.
        gas_limit: 100_000,
    };

    tracing::info!("Starting services.");
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    // Place Orders
    let mut order = OrderCreation {
        from: Some(safe.address()),
        sell_token: token.address(),
        sell_amount: to_wei(5),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [safe_creation, approval],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    };
    order.signature = Signature::Eip1271(safe.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));

    services.create_order(&order).await.unwrap();

    let balance = token.balance_of(safe.address()).call().await.unwrap();
    assert_eq!(balance, to_wei(5));

    // Check that the Safe really hasn't been deployed yet.
    let code = web3.eth().code(safe.address(), None).await.unwrap();
    assert_eq!(code.0.len(), 0);

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        token
            .balance_of(safe.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let balance = onchain
        .contracts()
        .weth
        .balance_of(safe.address())
        .call()
        .await
        .unwrap();
    assert!(balance >= order.buy_amount);

    // Check Safe was deployed
    let code = web3.eth().code(safe.address(), None).await.unwrap();
    assert_ne!(code.0.len(), 0);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();
}

async fn partial_fills(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(3)).await;

    let counter = contracts::test::Counter::builder(&web3)
        .deploy()
        .await
        .unwrap();

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(2))
    );
    tx_value!(
        trader.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    tracing::info!("Placing order");
    let order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(2),
        buy_token: token.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [hook_for_transaction(counter.increment_counter("pre".to_string()).tx).await],
                        "post": [hook_for_transaction(counter.increment_counter("post".to_string()).tx).await],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for first trade.");
    let trade_happened = || async {
        onchain
            .contracts()
            .weth
            .balance_of(trader.address())
            .call()
            .await
            .unwrap()
            == 0.into()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
    assert_eq!(
        counter.counters("pre".to_string()).call().await.unwrap(),
        1.into()
    );
    assert_eq!(
        counter.counters("post".to_string()).call().await.unwrap(),
        1.into()
    );

    tracing::info!("Fund remaining sell balance.");
    tx_value!(
        trader.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Waiting for second trade.");
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
    assert_eq!(
        counter.counters("pre".to_string()).call().await.unwrap(),
        1.into()
    );
    assert_eq!(
        counter.counters("post".to_string()).call().await.unwrap(),
        2.into()
    );
}
