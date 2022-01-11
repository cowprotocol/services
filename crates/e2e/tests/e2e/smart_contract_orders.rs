use crate::services::{
    create_order_converter, create_orderbook_api, deploy_mintable_token, to_wei,
    uniswap_pair_provider, OrderbookServices, API_HOST,
};
use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, Bytes, PrivateKey, U256};
use model::order::{Order, OrderBuilder, OrderKind, OrderStatus, OrderUid};
use shared::{maintenance::Maintaining, sources::uniswap_v2::pool_fetching::PoolFetcher, Web3};
use solver::{
    liquidity::uniswap_v2::UniswapLikeLiquidity,
    liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics,
    settlement_submission::{
        submitter::custom_nodes_api::CustomNodesApi, SolutionSubmitter, StrategyArgs,
    },
};
use std::{sync::Arc, time::Duration};

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

    // Note that this account is technically not a SC order. However, we allow
    // presign orders from EOAs as well, and it is easier to setup an order
    // for an EOA then SC wallet. In the future, once we add EIP-1271 support,
    // where we would **need** an SC wallet, we can also change this trader to
    // use one.
    let trader = Account::Offline(PrivateKey::from_raw(TRADER).unwrap(), None);

    // Create & Mint tokens to trade
    let token = deploy_mintable_token(&web3).await;
    tx!(
        solver_account,
        token.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(solver_account, token.mint(trader.address(), to_wei(10)));

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
    tx!(trader, token.approve(contracts.allowance, to_wei(10)));

    let OrderbookServices {
        price_estimator,
        block_stream,
        maintenance,
        solvable_orders_cache,
        base_tokens,
    } = OrderbookServices::new(&web3, &contracts).await;

    let client = reqwest::Client::new();

    // Place Orders
    let order = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(9))
        .with_fee_amount(to_wei(1))
        .with_buy_token(contracts.weth.address())
        .with_buy_amount(to_wei(8))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_presign(trader.address())
        .build()
        .order_creation;
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);

    solvable_orders_cache.update(0).await.unwrap();

    let order_uid = placement.json::<OrderUid>().await.unwrap();
    let order_status = || async {
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
            .order_meta_data
            .status
    };

    // Execute pre-sign transaction.
    assert_eq!(order_status().await, OrderStatus::PresignaturePending);
    tx!(
        trader,
        contracts
            .gp_settlement
            .set_pre_signature(Bytes(order_uid.0.to_vec()), true)
    );

    // Drive orderbook in order to check that the presignature event was received.
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();
    assert_eq!(order_status().await, OrderStatus::Open);

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        base_tokens,
        web3.clone(),
        Arc::new(PoolFetcher {
            pair_provider: uniswap_pair_provider,
            web3: web3.clone(),
        }),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        balancer_v2_liquidity: None,
    };
    let network_id = web3.net().version().await.unwrap();
    let mut driver = solver::driver::Driver::new(
        contracts.gp_settlement.clone(),
        liquidity_collector,
        price_estimator,
        vec![solver],
        Arc::new(web3.clone()),
        Duration::from_secs(30),
        contracts.weth.address(),
        Duration::from_secs(0),
        Arc::new(NoopMetrics::default()),
        web3.clone(),
        network_id,
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
                    additional_tip: 0.0,
                }),
            ],
        },
        1_000_000_000_000_000_000_u128.into(),
        10,
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        0.0,
        15000000u128,
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch token balance");
    assert_eq!(balance, U256::zero());

    let balance = contracts
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_eq!(balance, U256::from(8_972_194_924_949_384_291_u128));
}
