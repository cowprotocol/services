use crate::{
    services::{
        create_order_converter, create_orderbook_api, deploy_mintable_token, to_wei, GPv2,
        OrderbookServices, UniswapContracts, API_HOST,
    },
    tx, tx_value,
};
use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use hex_literal::hex;
use model::{
    order::{OrderBuilder, OrderKind},
    signature::EcdsaSigningScheme,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::maintenance::Maintaining;
use shared::{
    sources::uniswap::{pair_provider::UniswapPairProvider, pool_fetching::PoolFetcher},
    Web3,
};
use solver::{
    liquidity::uniswap::UniswapLikeLiquidity, liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics, settlement_submission::SolutionSubmitter,
};
use std::{sync::Arc, time::Duration};
use web3::signing::SecretKeyRef;

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const TRADER_B_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000002");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_onchain_settlement() {
    crate::local_node::test(onchain_settlement).await;
}

async fn onchain_settlement(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);

    let gpv2 = GPv2::fetch(&web3).await;
    let UniswapContracts {
        uniswap_factory,
        uniswap_router,
    } = UniswapContracts::fetch(&web3).await;

    // Create tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    let token_b = deploy_mintable_token(&web3).await;

    // Fund trader accounts
    tx!(
        solver_account,
        token_a.mint(trader_a.address(), to_wei(101))
    );
    tx!(solver_account, token_b.mint(trader_b.address(), to_wei(51)));

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        uniswap_factory.create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver_account,
        token_a.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_a.approve(uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.approve(uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            solver_account.address(),
            U256::max_value(),
        )
    );

    // Create and fund pools for fee connections.
    for token in [&token_a, &token_b] {
        tx!(
            solver_account,
            token.mint(solver_account.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            token.approve(uniswap_router.address(), to_wei(100_000))
        );
        tx_value!(solver_account, to_wei(100_000), gpv2.native_token.deposit());
        tx!(
            solver_account,
            gpv2.native_token
                .approve(uniswap_router.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            uniswap_router.add_liquidity(
                token.address(),
                gpv2.native_token.address(),
                to_wei(100_000),
                to_wei(100_000),
                0_u64.into(),
                0_u64.into(),
                solver_account.address(),
                U256::max_value(),
            )
        );
    }

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(gpv2.allowance, to_wei(101)));
    tx!(trader_b, token_b.approve(gpv2.allowance, to_wei(51)));

    // Place Orders
    let OrderbookServices {
        price_estimator,
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
    } = OrderbookServices::new(&web3, &gpv2, &uniswap_factory).await;

    let client = reqwest::Client::new();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &gpv2.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_A_PK).unwrap()),
        )
        .build()
        .order_creation;
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order_a).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    let order_b = OrderBuilder::default()
        .with_sell_token(token_b.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(40))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::EthSign,
            &gpv2.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_B_PK).unwrap()),
        )
        .build()
        .order_creation;
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order_b).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);
    let uniswap_pair_provider = Arc::new(UniswapPairProvider {
        factory: uniswap_factory.clone(),
        chain_id,
    });

    solvable_orders_cache.update(0).await.unwrap();

    // Drive solution
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, uniswap_router.address()),
        gpv2.settlement.clone(),
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
        gpv2.settlement.clone(),
        liquidity_collector,
        price_estimator,
        vec![solver],
        Arc::new(web3.clone()),
        Duration::from_secs(30),
        gpv2.native_token.address(),
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
            contract: gpv2.settlement.clone(),
            gas_price_estimator: Arc::new(web3.clone()),
            target_confirm_time: Duration::from_secs(1),
            gas_price_cap: f64::MAX,
            transaction_strategy: solver::settlement_submission::TransactionStrategy::CustomNodes(
                vec![web3.clone()],
            ),
        },
        1_000_000_000_000_000_000_u128.into(),
        10,
        create_orderbook_api(),
        create_order_converter(&web3, gpv2.native_token.address()),
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .expect("Couldn't fetch TokenB's balance");
    assert_eq!(balance, U256::from(99_650_498_453_042_316_811_u128));

    let balance = token_a
        .balance_of(trader_b.address())
        .call()
        .await
        .expect("Couldn't fetch TokenA's balance");
    assert_eq!(balance, U256::from(50_175_363_672_226_073_523_u128));

    // Drive orderbook in order to check the removal of settled order_b
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    let orders = create_orderbook_api().get_orders().await.unwrap();
    assert!(orders.orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}
