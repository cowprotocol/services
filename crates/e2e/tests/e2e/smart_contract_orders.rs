use {
    crate::setup::*,
    contracts::{GnosisSafe, GnosisSafeCompatibilityFallbackHandler, GnosisSafeProxy},
    ethcontract::{Bytes, H160, H256, U256},
    model::{
        order::{OrderBuilder, OrderKind, OrderStatus, OrderUid},
        signature::hashed_eip712_message,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
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

    // Deploy and setup a Gnosis Safe.
    let safe_singleton = GnosisSafe::builder(&web3).deploy().await.unwrap();
    let safe_fallback = GnosisSafeCompatibilityFallbackHandler::builder(&web3)
        .deploy()
        .await
        .unwrap();
    let safe_proxy = GnosisSafeProxy::builder(&web3, safe_singleton.address())
        .deploy()
        .await
        .unwrap();
    let safe = GnosisSafe::at(&web3, safe_proxy.address());
    safe.setup(
        vec![trader.address()],
        1.into(),         // threshold
        H160::default(),  // delegate call
        Bytes::default(), // delegate call bytes
        safe_fallback.address(),
        H160::default(), // relayer payment token
        0.into(),        // relayer payment amount
        H160::default(), // relayer address
    )
    .send()
    .await
    .unwrap();

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

    // Place Orders
    let order_template = || {
        OrderBuilder::default()
            .with_kind(OrderKind::Sell)
            .with_sell_token(token.address())
            .with_sell_amount(to_wei(4))
            .with_fee_amount(to_wei(1))
            .with_buy_token(onchain.contracts().weth.address())
            .with_buy_amount(to_wei(3))
            .with_valid_to(model::time::now_in_epoch_seconds() + 300)
    };
    let mut orders = [
        order_template()
            .with_eip1271(
                safe.address(),
                gnosis_safe_eip1271_signature(
                    SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
                    &safe,
                    H256(hashed_eip712_message(
                        &onchain.contracts().domain_separator,
                        &order_template().build().data.hash_struct(),
                    )),
                )
                .await,
            )
            .build(),
        order_template()
            .with_app_data([1; 32])
            .with_presign(safe.address())
            .build(),
    ];

    for order in &mut orders {
        let uid = services
            .create_order(&order.clone().into_order_creation())
            .await
            .unwrap();
        order.metadata.uid = uid;
    }
    let orders = orders; // prevent further changes to `orders`.

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
    assert_eq!(
        order_status(orders[0].metadata.uid).await,
        OrderStatus::Open
    );

    // Execute pre-sign transaction.
    assert_eq!(
        order_status(orders[1].metadata.uid).await,
        OrderStatus::PresignaturePending
    );
    tx_safe!(
        trader.account(),
        safe,
        onchain
            .contracts()
            .gp_settlement
            .set_pre_signature(Bytes(orders[1].metadata.uid.0.to_vec()), true)
    );

    // Check that the presignature event was received.
    wait_for_condition(Duration::from_secs(10), || async {
        services.get_auction().await.auction.orders.len() == 2
    })
    .await
    .unwrap();
    assert_eq!(
        order_status(orders[1].metadata.uid).await,
        OrderStatus::Open
    );

    // Drive solution
    tracing::info!("Waiting for trade.");
    services.start_old_driver(solver.private_key(), vec![]);
    wait_for_condition(Duration::from_secs(10), || async {
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
