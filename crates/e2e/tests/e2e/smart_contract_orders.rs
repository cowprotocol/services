use {
    crate::{
        onchain_components::{
            deploy_token_with_weth_uniswap_pool,
            gnosis_safe_eip1271_signature,
            to_wei,
            WethPoolConfig,
        },
        services::{solvable_orders, wait_for_condition, API_HOST},
    },
    contracts::{GnosisSafe, GnosisSafeCompatibilityFallbackHandler, GnosisSafeProxy},
    ethcontract::{transaction::TransactionBuilder, Account, Bytes, PrivateKey, H160, H256, U256},
    model::{
        order::{Order, OrderBuilder, OrderKind, OrderStatus, OrderUid},
        signature::hashed_eip712_message,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const TRADER: [u8; 32] = [1; 32];
const SOLVER: [u8; 32] = [2; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_smart_contract_orders() {
    crate::local_node::test(smart_contract_orders).await;
}

async fn smart_contract_orders(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER).unwrap(), None);
    let user = Account::Offline(PrivateKey::from_raw(TRADER).unwrap(), None);
    for account in [&user, &solver] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(1))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();

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
        vec![user.address()],
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

    // Create & mint tokens to trade, pools for fee connections
    let token = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;
    token.mint(safe.address(), to_wei(10)).await;
    let token = token.contract;

    // Approve GPv2 for trading
    tx_safe!(user, safe, token.approve(contracts.allowance, to_wei(10)));

    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    // Place Orders
    let order_template = || {
        OrderBuilder::default()
            .with_kind(OrderKind::Sell)
            .with_sell_token(token.address())
            .with_sell_amount(to_wei(4))
            .with_fee_amount(to_wei(1))
            .with_buy_token(contracts.weth.address())
            .with_buy_amount(to_wei(3))
            .with_valid_to(model::time::now_in_epoch_seconds() + 300)
    };
    let mut orders = [
        order_template()
            .with_eip1271(
                safe.address(),
                gnosis_safe_eip1271_signature(
                    SecretKeyRef::from(&SecretKey::from_slice(&TRADER).unwrap()),
                    &safe,
                    H256(hashed_eip712_message(
                        &contracts.domain_separator,
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
        let creation = order.clone().into_order_creation();
        let placement = client
            .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
            .json(&creation)
            .send()
            .await
            .unwrap();
        assert_eq!(placement.status(), 201);
        order.metadata.uid = placement.json::<OrderUid>().await.unwrap();
    }
    let orders = orders; // prevent further changes to `orders`.

    let order_status = |order_uid: OrderUid| {
        let client = client.clone();
        async move {
            client
                .get(&format!(
                    "{}{}{}",
                    API_HOST, ORDER_PLACEMENT_ENDPOINT, &order_uid
                ))
                .send()
                .await
                .unwrap()
                .json::<Order>()
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
        user,
        safe,
        contracts
            .gp_settlement
            .set_pre_signature(Bytes(orders[1].metadata.uid.0.to_vec()), true)
    );

    // Check that the presignature event was received.
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();
    assert_eq!(
        order_status(orders[1].metadata.uid).await,
        OrderStatus::Open
    );

    // Drive solution
    tracing::info!("Waiting for trade.");
    crate::services::start_old_driver(&contracts, &SOLVER, &[]);
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
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

    let balance = contracts
        .weth
        .balance_of(safe.address())
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_eq!(balance, U256::from(7_975_363_884_976_534_272_u128));
}
