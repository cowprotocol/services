use contracts::{IUniswapLikeRouter, WETH9};
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use model::{
    order::{OrderBuilder, OrderKind, BUY_ETH_ADDRESS},
    SigningScheme,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::{amm_pair_provider::UniswapPairProvider, maintenance::Maintaining, Web3};
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

const TRADER_BUY_ETH_A_PK: [u8; 32] = [1; 32];
const TRADER_BUY_ETH_B_PK: [u8; 32] = [2; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";
const FEE_ENDPOINT: &str = "/api/v1/fee/";

#[tokio::test]
async fn ganache_eth_integration() {
    ganache::test(eth_integration).await;
}

async fn eth_integration(web3: Web3) {
    shared::tracing::initialize("warn,orderbook=debug,solver=debug");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver = Account::Local(accounts[0], None);
    let trader_buy_eth_a =
        Account::Offline(PrivateKey::from_raw(TRADER_BUY_ETH_A_PK).unwrap(), None);
    let trader_buy_eth_b =
        Account::Offline(PrivateKey::from_raw(TRADER_BUY_ETH_B_PK).unwrap(), None);

    let gpv2 = GPv2::fetch(&web3, &solver).await;
    let UniswapContracts {
        uniswap_factory,
        uniswap_router,
    } = UniswapContracts::fetch(&web3).await;

    // Create & Mint tokens to trade
    let token = deploy_mintable_token(&web3).await;
    tx!(solver, token.mint(solver.address(), to_wei(100_000)));
    tx!(solver, token.mint(trader_buy_eth_a.address(), to_wei(50)));
    tx!(solver, token.mint(trader_buy_eth_b.address(), to_wei(50)));

    let weth = WETH9::builder(&web3)
        .deploy()
        .await
        .expect("WETH deployment failed");
    tx_value!(solver, to_wei(100_000), weth.deposit());

    // Create and fund Uniswap pool
    tx!(
        solver,
        uniswap_factory.create_pair(token.address(), weth.address())
    );
    tx!(
        solver,
        token.approve(uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver,
        weth.approve(uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver,
        uniswap_router.add_liquidity(
            token.address(),
            weth.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader_buy_eth_a, token.approve(gpv2.allowance, to_wei(50)));
    tx!(trader_buy_eth_b, token.approve(gpv2.allowance, to_wei(50)));

    let native_token = weth.address();
    let OrderbookServices {
        maintenance,
        price_estimator,
        block_stream,
        ..
    } = OrderbookServices::new(&web3, &gpv2, &uniswap_factory, native_token).await;

    let client = reqwest::Client::new();

    // Test fee endpoint
    let client_ref = &client;
    let estimate_fee = |sell_token, buy_token| async move {
        client_ref
            .get(&format!(
                "{}{}?sellToken={:?}&buyToken={:?}&amount={}&kind=sell",
                API_HOST,
                FEE_ENDPOINT,
                sell_token,
                buy_token,
                to_wei(42)
            ))
            .send()
            .await
            .unwrap()
    };
    let fee_buy_eth = estimate_fee(token.address(), BUY_ETH_ADDRESS).await;
    assert_eq!(fee_buy_eth.status(), 200);
    // Eth is only supported as the buy token
    let fee_invalid_token = estimate_fee(BUY_ETH_ADDRESS, token.address()).await;
    assert_eq!(fee_invalid_token.status(), 404);

    // Place Orders
    assert_ne!(weth.address(), BUY_ETH_ADDRESS);
    let order_buy_eth_a = OrderBuilder::default()
        .with_kind(OrderKind::Buy)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(50))
        .with_buy_token(BUY_ETH_ADDRESS)
        .with_buy_amount(to_wei(49))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_signing_scheme(SigningScheme::Eip712)
        .sign_with(
            &gpv2.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_BUY_ETH_A_PK).unwrap()),
        )
        .build()
        .order_creation;
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order_buy_eth_a).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);
    let order_buy_eth_b = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(50))
        .with_buy_token(BUY_ETH_ADDRESS)
        .with_buy_amount(to_wei(49))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .with_signing_scheme(SigningScheme::Eip712)
        .sign_with(
            &gpv2.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_BUY_ETH_B_PK).unwrap()),
        )
        .build()
        .order_creation;
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order_buy_eth_b).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    // Drive solution
    let uniswap_pair_provider = Arc::new(UniswapPairProvider {
        factory: uniswap_factory.clone(),
        chain_id,
    });

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
        orderbook_api: create_orderbook_api(&web3, weth.address()),
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
        f64::MAX,
        None,
        block_stream,
    );
    driver.single_run().await.unwrap();

    // Check matching
    let web3_ref = &web3;
    let eth_balance = |trader: Account| async move {
        web3_ref
            .eth()
            .balance(trader.address(), None)
            .await
            .expect("Couldn't fetch ETH balance")
    };
    assert_eq!(eth_balance(trader_buy_eth_a).await, to_wei(49));
    assert_eq!(
        eth_balance(trader_buy_eth_b).await,
        U256::from(49_800_747_827_208_136_743u128)
    );

    // Drive orderbook in order to check that all orders were settled
    maintenance.run_maintenance().await.unwrap();

    let orders = create_orderbook_api(&web3, weth.address())
        .get_orders()
        .await
        .unwrap();
    assert!(orders.is_empty());
}
