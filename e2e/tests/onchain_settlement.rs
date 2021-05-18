use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use hex_literal::hex;
use model::{
    order::{OrderBuilder, OrderKind},
    SigningScheme,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::{amm_pair_provider::UniswapPairProvider, Web3};
use solver::{
    liquidity::uniswap::UniswapLikeLiquidity, liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics,
};
use std::{collections::HashSet, sync::Arc, time::Duration};
use web3::signing::SecretKeyRef;

mod ganache;
#[macro_use]
mod services;
use crate::services::{
    create_orderbook_api, deploy_mintable_token, to_wei, GPv2, OrderbookServices, UniswapContracts,
    API_HOST,
};

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const TRADER_B_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000002");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
async fn ganache_onchain_settlement() {
    ganache::test(onchain_settlement).await;
}

async fn onchain_settlement(web3: Web3) {
    shared::tracing::initialize("warn,orderbook=debug,solver=debug");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver = Account::Local(accounts[0], None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);

    let gpv2 = GPv2::fetch(&web3, &solver).await;
    let UniswapContracts {
        uniswap_factory,
        uniswap_router,
    } = UniswapContracts::fetch(&web3).await;

    // Create & Mint tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    tx!(solver, token_a.mint(solver.address(), to_wei(100_000)));
    tx!(solver, token_a.mint(trader_a.address(), to_wei(100)));

    let token_b = deploy_mintable_token(&web3).await;
    tx!(solver, token_b.mint(solver.address(), to_wei(100_000)));
    tx!(solver, token_b.mint(trader_b.address(), to_wei(100)));

    // Create and fund Uniswap pool
    tx!(
        solver,
        uniswap_factory.create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver,
        token_a.approve(uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver,
        token_b.approve(uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver,
        uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(gpv2.allowance, to_wei(100)));
    tx!(trader_b, token_b.approve(gpv2.allowance, to_wei(100)));

    // Place Orders
    let native_token = token_a.address();
    let OrderbookServices {
        orderbook,
        price_estimator,
    } = OrderbookServices::new(&web3, &gpv2, &uniswap_factory, native_token).await;

    let client = reqwest::Client::new();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .with_signing_scheme(SigningScheme::Eip712)
        .sign_with(
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
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(40))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .with_signing_scheme(SigningScheme::EthSign)
        .sign_with(
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

    // Drive solution
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, uniswap_router.address()),
        uniswap_pair_provider.clone(),
        gpv2.settlement.clone(),
        HashSet::new(),
        web3.clone(),
    );
    let solver = solver::naive_solver::NaiveSolver {};
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        orderbook_api: create_orderbook_api(&web3),
    };
    let network_id = web3.net().version().await.unwrap();
    let mut driver = solver::driver::Driver::new(
        gpv2.settlement.clone(),
        liquidity_collector,
        price_estimator,
        vec![Box::new(solver)],
        Arc::new(web3.clone()),
        Duration::from_secs(1),
        Duration::from_secs(30),
        native_token,
        Duration::from_secs(0),
        Arc::new(NoopMetrics::default()),
        web3.clone(),
        network_id,
        1,
        Duration::from_secs(30),
        None,
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .expect("Couldn't fetch TokenB's balance");
    assert_eq!(balance, U256::from(99_650_498_453_042_316_810u128));

    let balance = token_a
        .balance_of(trader_b.address())
        .call()
        .await
        .expect("Couldn't fetch TokenA's balance");
    assert_eq!(balance, U256::from(50_175_363_672_226_073_522u128));

    // Drive orderbook in order to check the removal of settled order_b
    orderbook.run_maintenance(&gpv2.settlement).await.unwrap();

    let orders = create_orderbook_api(&web3).get_orders().await.unwrap();
    assert!(orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}
