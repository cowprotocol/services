use crate::services::{
    create_order_converter, create_orderbook_api, deploy_mintable_token,
    gnosis_safe_eip1271_signature, to_wei, uniswap_pair_provider, OrderbookServices, API_HOST,
};
use contracts::{
    GnosisSafe, GnosisSafeCompatibilityFallbackHandler, GnosisSafeProxy, IUniswapLikeRouter,
};
use ethcontract::{Account, Address, Bytes, PrivateKey, H160, H256, U256};
use model::{
    order::{Order, OrderBuilder, OrderKind, OrderStatus, OrderUid},
    signature::hashed_eip712_message,
};
use secp256k1::SecretKey;
use shared::{maintenance::Maintaining, sources::uniswap_v2::pool_fetching::PoolFetcher, Web3};
use solver::{
    liquidity::uniswap_v2::UniswapLikeLiquidity,
    liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics,
    settlement_access_list::{create_priority_estimator, AccessListEstimatorType},
    settlement_submission::{
        submitter::{custom_nodes_api::CustomNodesApi, Strategy},
        GlobalTxPool, SolutionSubmitter, StrategyArgs,
    },
};
use std::{sync::Arc, time::Duration};
use web3::signing::SecretKeyRef;

const TRADER: [u8; 32] = [1; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_smart_contract_orders() {
    crate::local_node::test(smart_contract_orders).await;
}

async fn smart_contract_orders(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug");
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);

    let user = Account::Offline(PrivateKey::from_raw(TRADER).unwrap(), None);

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

    // Create & Mint tokens to trade
    let token = deploy_mintable_token(&web3).await;
    tx!(
        solver_account,
        token.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(solver_account, token.mint(safe.address(), to_wei(10)));

    tx_value!(solver_account, to_wei(100_000), contracts.weth.deposit());

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token.address(), contracts.weth.address())
    );
    tx!(
        solver_account,
        token.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        contracts
            .weth
            .approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        contracts.uniswap_router.add_liquidity(
            token.address(),
            contracts.weth.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            solver_account.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx_safe!(user, safe, token.approve(contracts.allowance, to_wei(10)));

    let OrderbookServices {
        block_stream,
        maintenance,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts).await;

    let client = reqwest::Client::new();

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
            .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
            .json(&creation)
            .send()
            .await
            .unwrap();
        assert_eq!(placement.status(), 201);
        order.metadata.uid = placement.json::<OrderUid>().await.unwrap();
    }
    let orders = orders; // prevent further changes to `orders`.

    solvable_orders_cache.update(0).await.unwrap();

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

    // Drive orderbook in order to check that the presignature event was received.
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();
    assert_eq!(
        order_status(orders[1].metadata.uid).await,
        OrderStatus::Open
    );

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        base_tokens,
        web3.clone(),
        Arc::new(PoolFetcher::uniswap(uniswap_pair_provider, web3.clone())),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        balancer_v2_liquidity: None,
        zeroex_liquidity: None,
    };
    let network_id = web3.net().version().await.unwrap();
    let submitted_transactions = GlobalTxPool::default();
    let mut driver = solver::driver::Driver::new(
        contracts.gp_settlement.clone(),
        liquidity_collector,
        vec![solver],
        Arc::new(web3.clone()),
        Duration::from_secs(30),
        contracts.weth.address(),
        Duration::from_secs(0),
        Arc::new(NoopMetrics::default()),
        web3.clone(),
        network_id.clone(),
        1,
        Duration::from_secs(30),
        None,
        block_stream,
        SolutionSubmitter {
            web3: web3.clone(),
            contract: contracts.gp_settlement.clone(),
            gas_price_estimator: Arc::new(web3.clone()),
            target_confirm_time: Duration::from_secs(1),
            gas_price_cap: f64::MAX,
            max_confirm_time: Duration::from_secs(120),
            retry_interval: Duration::from_secs(5),
            transaction_strategies: vec![
                solver::settlement_submission::TransactionStrategy::CustomNodes(StrategyArgs {
                    submit_api: Box::new(CustomNodesApi::new(vec![web3.clone()])),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::CustomNodes),
                }),
            ],
            access_list_estimator: Arc::new(
                create_priority_estimator(
                    &client,
                    &web3,
                    &[AccessListEstimatorType::Web3],
                    None,
                    None,
                    network_id,
                )
                .await
                .unwrap(),
            ),
        },
        10,
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        0.0,
        15000000u128,
        1.0,
        None,
        None.into(),
        None,
    );
    driver.single_run().await.unwrap();

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
