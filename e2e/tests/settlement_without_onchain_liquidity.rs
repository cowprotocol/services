use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use hex_literal::hex;
use model::{
    order::{OrderBuilder, OrderKind},
    SigningScheme,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::{
    sources::uniswap::{pair_provider::UniswapPairProvider, pool_fetching::PoolFetcher},
    token_list::{Token, TokenList},
    Web3,
};
use solver::{
    liquidity::uniswap::UniswapLikeLiquidity, liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics, settlement_submission::SolutionSubmitter,
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
use shared::maintenance::Maintaining;

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
async fn ganache_onchain_settlement_without_liquidity() {
    ganache::test(onchain_settlement_without_liquidity).await;
}

async fn onchain_settlement_without_liquidity(web3: Web3) {
    shared::tracing::initialize("warn,orderbook=debug,solver=debug");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_account = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);

    let gpv2 = GPv2::fetch(&web3, &solver_account).await;
    let UniswapContracts {
        uniswap_factory,
        uniswap_router,
    } = UniswapContracts::fetch(&web3).await;

    // Create & Mint tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    let token_b = deploy_mintable_token(&web3).await;

    // Fund trader and settlement accounts
    tx!(
        solver_account,
        token_a.mint(trader_account.address(), to_wei(100))
    );
    tx!(
        solver_account,
        token_b.mint(gpv2.settlement.address(), to_wei(100))
    );

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
    tx!(trader_account, token_a.approve(gpv2.allowance, to_wei(100)));

    // Place Orders
    let OrderbookServices {
        price_estimator,
        maintenance,
        block_stream,
    } = OrderbookServices::new(&web3, &gpv2, &uniswap_factory).await;

    let client = reqwest::Client::new();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(90))
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
        .body(json!(order).to_string())
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
        gpv2.settlement.clone(),
        HashSet::new(),
        web3.clone(),
        Arc::new(PoolFetcher {
            pair_provider: uniswap_pair_provider,
            web3: web3.clone(),
        }),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        orderbook_api: create_orderbook_api(&web3, gpv2.native_token.address()),
        balancer_v2_liquidity: None,
    };
    let network_id = web3.net().version().await.unwrap();
    let market_makable_token_list = TokenList::new(maplit::hashmap! {
        token_a.address() => Token {
            address: token_a.address(),
            name: "Test Coin".into(),
            symbol: "TC".into(),
            decimals: 18,
        }
    });
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
        Duration::from_secs(10),
        Some(market_makable_token_list),
        block_stream,
        0.0,
        SolutionSubmitter {
            web3: web3.clone(),
            contract: gpv2.settlement.clone(),
            gas_price_estimator: Arc::new(web3.clone()),
            target_confirm_time: Duration::from_secs(1),
            gas_price_cap: f64::MAX,
            transaction_strategy: solver::settlement_submission::TransactionStrategy::PublicMempool,
        },
    );
    driver.single_run().await.unwrap();

    // Check that trader traded.
    let balance = token_a
        .balance_of(trader_account.address())
        .call()
        .await
        .expect("Couldn't fetch trader TokenA's balance");
    assert_eq!(balance, U256::from(0_u128));

    let balance = token_b
        .balance_of(trader_account.address())
        .call()
        .await
        .expect("Couldn't fetch trader TokenB's balance");
    assert!(balance > U256::zero());

    // Check that settlement buffers were traded.
    let balance = token_a
        .balance_of(gpv2.settlement.address())
        .call()
        .await
        .expect("Couldn't fetch settlements TokenA's balance");
    assert_eq!(balance, to_wei(100));

    // Drive orderbook in order to check the removal of settled order_b
    maintenance.run_maintenance().await.unwrap();

    let orders = create_orderbook_api(&web3, gpv2.native_token.address())
        .get_orders()
        .await
        .unwrap();
    assert!(orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}
