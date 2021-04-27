use contracts::{ERC20Mintable, IUniswapLikeRouter, UniswapV2Factory, UniswapV2Router02, WETH9};
use ethcontract::{
    prelude::{Account, Address, PrivateKey, U256},
    H160,
};
use hex_literal::hex;
use model::{
    order::{OrderBuilder, OrderKind},
    DomainSeparator, SigningScheme,
};
use orderbook::{
    account_balances::Web3BalanceFetcher, database::Database, event_updater::EventUpdater,
    fee::MinFeeCalculator, orderbook::Orderbook,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::{
    amm_pair_provider::UniswapPairProvider,
    current_block::current_block_stream,
    pool_fetching::{CachedPoolFetcher, PoolFetcher},
    price_estimate::UniswapPriceEstimator,
    Web3,
};
use solver::{
    liquidity::uniswap::UniswapLikeLiquidity, liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics, orderbook::OrderBookApi,
};
use std::{collections::HashSet, str::FromStr, sync::Arc, time::Duration};
use web3::signing::SecretKeyRef;

mod ganache;

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const TRADER_B_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000002");

const API_HOST: &str = "http://127.0.0.1:8080";
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

    let deploy_mintable_token = || async {
        ERC20Mintable::builder(&web3)
            .deploy()
            .await
            .expect("MintableERC20 deployment failed")
    };

    macro_rules! tx {
        ($acc:ident, $call:expr) => {{
            const NAME: &str = stringify!($call);
            $call
                .from($acc.clone())
                .send()
                .await
                .expect(&format!("{} failed", NAME))
        }};
    }

    // Fetch deployed instances
    let uniswap_factory = UniswapV2Factory::deployed(&web3)
        .await
        .expect("Failed to load deployed UniswapFactory");
    let uniswap_router = UniswapV2Router02::deployed(&web3)
        .await
        .expect("Failed to load deployed UniswapRouter");
    let gp_settlement = solver::get_settlement_contract(&web3, solver.clone())
        .await
        .expect("Failed to load deployed GPv2Settlement");
    let gp_allowance = gp_settlement
        .allowance_manager()
        .call()
        .await
        .expect("Couldn't get allowance manager address");

    // Create & Mint tokens to trade
    let token_a = deploy_mintable_token().await;
    tx!(solver, token_a.mint(solver.address(), to_wei(100_000)));
    tx!(solver, token_a.mint(trader_a.address(), to_wei(100)));

    let token_b = deploy_mintable_token().await;
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
    tx!(trader_a, token_a.approve(gp_allowance, to_wei(100)));
    tx!(trader_b, token_b.approve(gp_allowance, to_wei(100)));

    // Place Orders
    let domain_separator = DomainSeparator(
        gp_settlement
            .domain_separator()
            .call()
            .await
            .expect("Couldn't query domain separator"),
    );
    let db = Database::new("postgresql://").unwrap();
    db.clear().await.unwrap();
    let event_updater = EventUpdater::new(gp_settlement.clone(), db.clone(), None);

    let current_block_stream = current_block_stream(web3.clone()).await.unwrap();
    let pair_provider = Arc::new(UniswapPairProvider {
        factory: uniswap_factory.clone(),
        chain_id,
    });
    let pool_fetcher = CachedPoolFetcher::new(
        Box::new(PoolFetcher {
            pair_provider,
            web3: web3.clone(),
        }),
        current_block_stream,
    );
    let price_estimator = Arc::new(UniswapPriceEstimator::new(
        Box::new(pool_fetcher),
        HashSet::new(),
    ));
    let native_token = token_a.address();
    let fee_calculator = Arc::new(MinFeeCalculator::new(
        price_estimator.clone(),
        Box::new(web3.clone()),
        native_token,
        db.clone(),
        1.0,
    ));
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        db.clone(),
        event_updater,
        Box::new(Web3BalanceFetcher::new(
            web3.clone(),
            gp_allowance,
            gp_settlement.address(),
        )),
        fee_calculator.clone(),
        HashSet::new(),
    ));

    orderbook::serve_task(
        db.clone(),
        orderbook.clone(),
        fee_calculator,
        price_estimator.clone(),
        API_HOST[7..].parse().expect("Couldn't parse API address"),
    );
    let client = reqwest::Client::new();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
        .with_valid_to(u32::max_value())
        .with_kind(OrderKind::Sell)
        .with_signing_scheme(SigningScheme::Eip712)
        .sign_with(
            &domain_separator,
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
        .with_valid_to(u32::max_value())
        .with_kind(OrderKind::Sell)
        .with_signing_scheme(SigningScheme::EthSign)
        .sign_with(
            &domain_separator,
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
        gp_settlement.clone(),
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
        gp_settlement.clone(),
        liquidity_collector,
        price_estimator,
        vec![Box::new(solver)],
        Box::new(web3.clone()),
        Duration::from_secs(1),
        Duration::from_secs(30),
        native_token,
        Duration::from_secs(0),
        Arc::new(NoopMetrics::default()),
        web3.clone(),
        network_id,
        1,
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
    orderbook.run_maintenance(&gp_settlement).await.unwrap();

    let orders = create_orderbook_api(&web3).get_orders().await.unwrap();
    assert!(orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}

fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
}

fn create_orderbook_api(web3: &Web3) -> OrderBookApi {
    let native_token = WETH9::at(web3, H160([0x42; 20]));
    solver::orderbook::OrderBookApi::new(
        reqwest::Url::from_str(API_HOST).unwrap(),
        std::time::Duration::from_secs(10),
        native_token,
    )
}
