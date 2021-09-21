use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use model::{
    order::{OrderBuilder, OrderKind, SellTokenSource},
    signature::EcdsaSigningScheme,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::{
    sources::uniswap::{pair_provider::UniswapPairProvider, pool_fetching::PoolFetcher},
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

const TRADER: [u8; 32] = [1; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
async fn ganache_vault_balances() {
    ganache::test(vault_balances).await;
}

async fn vault_balances(web3: Web3) {
    shared::tracing::initialize("warn,orderbook=debug,solver=debug");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader = Account::Offline(PrivateKey::from_raw(TRADER).unwrap(), None);

    let gpv2 = GPv2::fetch(&web3).await;
    let UniswapContracts {
        uniswap_factory,
        uniswap_router,
    } = UniswapContracts::fetch(&web3).await;

    // Create & Mint tokens to trade
    let token = deploy_mintable_token(&web3).await;
    tx!(
        solver_account,
        token.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(solver_account, token.mint(trader.address(), to_wei(10)));

    tx_value!(solver_account, to_wei(100_000), gpv2.native_token.deposit());

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        uniswap_factory.create_pair(token.address(), gpv2.native_token.address())
    );
    tx!(
        solver_account,
        token.approve(uniswap_router.address(), to_wei(100_000))
    );
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

    // Approve GPv2 for trading
    tx!(trader, token.approve(gpv2.vault.address(), to_wei(10)));
    tx!(
        trader,
        gpv2.vault
            .set_relayer_approval(trader.address(), gpv2.allowance, true)
    );

    let OrderbookServices {
        price_estimator,
        block_stream,
        solvable_orders_cache,
        ..
    } = OrderbookServices::new(&web3, &gpv2, &uniswap_factory).await;

    let client = reqwest::Client::new();

    // Place Orders
    let order = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(9))
        .with_sell_token_balance(SellTokenSource::External)
        .with_fee_amount(to_wei(1))
        .with_buy_token(gpv2.native_token.address())
        .with_buy_amount(to_wei(8))
        .with_valid_to(shared::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &gpv2.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER).unwrap()),
        )
        .build()
        .order_creation;
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    solvable_orders_cache.update(0).await.unwrap();

    // Drive solution
    let uniswap_pair_provider = Arc::new(UniswapPairProvider {
        factory: uniswap_factory.clone(),
        chain_id,
    });

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
        1.0,
        SolutionSubmitter {
            web3: web3.clone(),
            contract: gpv2.settlement.clone(),
            gas_price_estimator: Arc::new(web3.clone()),
            target_confirm_time: Duration::from_secs(1),
            gas_price_cap: f64::MAX,
            transaction_strategy: solver::settlement_submission::TransactionStrategy::PublicMempool,
        },
        1_000_000_000_000_000_000_u128.into(),
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch token balance");
    assert_eq!(balance, U256::zero());

    let balance = gpv2
        .native_token
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_eq!(balance, U256::from(8_972_194_924_949_384_291_u128));
}
