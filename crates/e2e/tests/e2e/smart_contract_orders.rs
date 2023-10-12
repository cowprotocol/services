use {
    e2e::{setup::*, tx_safe},
    ethcontract::{Bytes, H160, H256, U256},
    model::{
        app_data::AppDataHash,
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus, OrderUid},
        signature::{hashed_eip712_message, Signature},
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_smart_contract_orders() {
    run_test(smart_contract_orders).await;
}

async fn smart_contract_orders(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let safe_infra = GnosisSafeInfrastructure::new(&web3).await;
    let safe = safe_infra.deploy_safe(vec![trader.address()], 1).await;

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(100_000), to_wei(100_000))
        .await;
    token.mint(safe.address(), to_wei(10)).await;

    // Approve GPv2 for trading
    tx_safe!(
        trader.account(),
        safe,
        token.approve(onchain.contracts().allowance, to_wei(10))
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services.start_api(vec![]).await;

    let order_template = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: token.address(),
        sell_amount: to_wei(4),
        fee_amount: to_wei(1),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    };
    let signature1271 = gnosis_safe_eip1271_signature(
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        &safe,
        H256(hashed_eip712_message(
            &onchain.contracts().domain_separator,
            &order_template.data().hash_struct(),
        )),
    )
    .await;

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
            app_data: OrderCreationAppData::Hash {
                hash: AppDataHash([1; 32]),
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
    tx_safe!(
        trader.account(),
        safe,
        onchain
            .contracts()
            .gp_settlement
            .set_pre_signature(Bytes(uids[1].0.to_vec()), true)
    );

    // Check that the presignature event was received.
    wait_for_condition(TIMEOUT, || async {
        services.get_auction().await.auction.orders.len() == 2
    })
    .await
    .unwrap();
    assert_eq!(order_status(uids[1]).await, OrderStatus::Open);

    // Drive solution
    tracing::info!("Waiting for trade.");
    services.start_old_driver(solver.private_key(), vec![]);
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
    assert_eq!(balance, U256::from(7_975_363_884_976_534_272_u128));
}
