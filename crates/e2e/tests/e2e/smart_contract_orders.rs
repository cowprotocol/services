use {
    e2e::{
        setup::{safe::Safe, *},
        tx,
    },
    ethcontract::{Bytes, H160, U256},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus, OrderUid},
        signature::Signature,
    },
    reqwest::StatusCode,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_smart_contract_orders() {
    run_test(smart_contract_orders).await;
}

#[tokio::test]
#[ignore]
async fn local_node_max_gas_limit() {
    run_test(erc1271_gas_limit).await;
}

async fn smart_contract_orders(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let safe = Safe::deploy(trader, &web3).await;

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(100_000), to_wei(100_000))
        .await;
    token.mint(safe.address(), to_wei(10)).await;

    // Approve GPv2 for trading
    safe.exec_call(token.approve(onchain.contracts().allowance, to_wei(10)))
        .await;

    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order_template = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: token.address(),
        sell_amount: to_wei(5),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    };
    let signature1271 = safe.order_eip1271_signature(&order_template, &onchain);

    // Check that we can't place invalid orders.
    let orders = [
        OrderCreation {
            from: Some(safe.address()),
            signature: Signature::Eip1271(b"invalid signature".to_vec()),
            ..order_template.clone()
        },
        OrderCreation {
            from: Some(H160(*b"invalid address\0\0\0\0\0")),
            signature: Signature::Eip1271(signature1271.clone()),
            ..order_template.clone()
        },
    ];
    for order in &orders {
        let (_, err) = dbg!(services.create_order(order).await.unwrap_err());
        assert!(err.contains("InvalidEip1271Signature"));
    }

    // Place orders
    let orders = [
        OrderCreation {
            from: Some(safe.address()),
            signature: Signature::Eip1271(signature1271),
            ..order_template.clone()
        },
        OrderCreation {
            app_data: OrderCreationAppData::Full {
                full: "{\"salt\": \"second\"}".to_string(),
            },
            from: Some(safe.address()),
            signature: Signature::PreSign,
            ..order_template.clone()
        },
    ];

    let mut uids = Vec::new();
    for order in &orders {
        let uid = services.create_order(order).await.unwrap();
        uids.push(uid);
    }
    let uids = uids;

    let order_status = |order_uid: OrderUid| {
        let services = &services;
        async move {
            services
                .get_order(&order_uid)
                .await
                .unwrap()
                .metadata
                .status
        }
    };

    // Check that the EIP-1271 order was received.
    assert_eq!(order_status(uids[0]).await, OrderStatus::Open);

    // Execute pre-sign transaction.
    assert_eq!(
        order_status(uids[1]).await,
        OrderStatus::PresignaturePending
    );
    safe.exec_call(
        onchain
            .contracts()
            .gp_settlement
            .set_pre_signature(Bytes(uids[1].0.to_vec()), true),
    )
    .await;

    // Check that the presignature event was received.
    wait_for_condition(TIMEOUT, || async {
        services.get_auction().await.auction.orders.len() == 2
    })
    .await
    .unwrap();
    assert_eq!(order_status(uids[1]).await, OrderStatus::Open);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        services.get_auction().await.auction.orders.is_empty()
    })
    .await
    .unwrap();

    // Check matching
    let balance = token
        .balance_of(safe.address())
        .call()
        .await
        .expect("Couldn't fetch token balance");
    assert_eq!(balance, U256::zero());

    let balance = onchain
        .contracts()
        .weth
        .balance_of(safe.address())
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_eq!(balance, U256::from(9_968_506_205_772_730_824_u128));
}

async fn erc1271_gas_limit(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let trader = contracts::test::GasHog::builder(&web3)
        .deploy()
        .await
        .unwrap();

    let cow = onchain
        .deploy_cow_weth_pool(to_wei(1_000_000), to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts and approve relayer
    cow.fund(trader.address(), to_wei(5)).await;
    tx!(
        solver.account(),
        trader.approve(cow.address(), onchain.contracts().allowance, to_wei(10))
    );

    let services = Services::new(onchain.contracts()).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec!["--max-gas-per-order=1000000".to_string()],
                ..Default::default()
            },
            solver,
        )
        .await;

    // Use 1M gas units during signature verification
    let mut signature = [0; 32];
    U256::exp10(6).to_big_endian(&mut signature);

    let order = OrderCreation {
        sell_token: cow.address(),
        sell_amount: to_wei(4),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        signature: Signature::Eip1271(signature.to_vec()),
        from: Some(trader.address()),
        ..Default::default()
    };

    let error = services.create_order(&order).await.unwrap_err();
    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    assert!(error.1.contains("TooMuchGas"));
}
