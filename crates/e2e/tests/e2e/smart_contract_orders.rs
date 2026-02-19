use {
    ::alloy::primitives::{Address, U256},
    e2e::setup::{safe::Safe, *},
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus, OrderUid},
        signature::Signature,
    },
    number::units::EthUnit,
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

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let safe = Safe::deploy(trader, web3.provider.clone()).await;

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    token.mint(safe.address(), 10u64.eth()).await;

    // Approve GPv2 for trading
    safe.exec_alloy_call(
        token
            .approve(onchain.contracts().allowance, 10u64.eth())
            .into_transaction_request(),
    )
    .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order_template = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: *token.address(),
        sell_amount: 5u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 3u64.eth(),
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
            from: Some(Address::new(*b"invalid address\0\0\0\0\0")),
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
    safe.exec_alloy_call(
        onchain
            .contracts()
            .gp_settlement
            .setPreSignature(uids[1].0.into(), true)
            .into_transaction_request(),
    )
    .await;

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let token_balance = token
            .balanceOf(safe.address())
            .call()
            .await
            .expect("Couldn't fetch token balance");
        let weth_balance = onchain
            .contracts()
            .weth
            .balanceOf(safe.address())
            .call()
            .await
            .expect("Couldn't fetch native token balance");

        token_balance.is_zero() && weth_balance > 6u64.eth()
    })
    .await
    .unwrap();
}

async fn erc1271_gas_limit(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let trader = contracts::alloy::test::GasHog::Instance::deploy(web3.provider.clone())
        .await
        .unwrap();

    let cow = onchain
        .deploy_cow_weth_pool(1_000_000u64.eth(), 1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts and approve relayer
    cow.fund(*trader.address(), 5u64.eth()).await;
    trader
        .approve(*cow.address(), onchain.contracts().allowance, 10u64.eth())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
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
    let signature = U256::from(1_000_000).to_be_bytes::<32>();

    let order = OrderCreation {
        sell_token: *cow.address(),
        sell_amount: 4u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        signature: Signature::Eip1271(signature.to_vec()),
        from: Some(*trader.address()),
        ..Default::default()
    };

    let error = services.create_order(&order).await.unwrap_err();
    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    assert!(error.1.contains("TooMuchGas"));
}
