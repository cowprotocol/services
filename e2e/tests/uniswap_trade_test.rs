use contracts::{ERC20Mintable, UniswapV2Factory, UniswapV2Router02};
use ethcontract::{
    prelude::{Account, Address, Http, PrivateKey, Web3, U256},
    H160,
};
use hex_literal::hex;
use model::{
    order::{OrderBuilder, OrderKind},
    DomainSeparator,
};
use orderbook::{
    account_balances::Web3BalanceFetcher, database::Database, event_updater::EventUpdater,
    fee::MinFeeCalculator, orderbook::Orderbook, price_estimate::UniswapPriceEstimator,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::uniswap_pool::PoolFetcher;
use solver::{liquidity::uniswap::UniswapLiquidity, orderbook::OrderBookApi};
use std::{str::FromStr, sync::Arc, time::Duration};
use web3::signing::SecretKeyRef;

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const TRADER_B_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000002");

const NODE_HOST: &str = "http://127.0.0.1:8545";
const API_HOST: &str = "http://127.0.0.1:8080";
const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
async fn test_with_ganache() {
    shared::tracing::initialize("warn,orderbook=debug,solver=debug");
    let http = Http::new(NODE_HOST).expect("transport failure");
    let web3 = Web3::new(http);
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
            .gas(8_000_000u32.into())
            .deploy()
            .await
            .expect("MintableERC20 deployment failed")
    };

    macro_rules! tx {
        ($acc:ident, $call:expr) => {{
            const NAME: &str = stringify!($call);
            $call
                .from($acc.clone())
                .gas(8_000_000u32.into())
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
        .expect("Failed to load deployed UniswapFactory");

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
    let event_updater = EventUpdater::new(gp_settlement.clone(), db.clone());
    let price_estimator = UniswapPriceEstimator::new(Box::new(PoolFetcher {
        factory: uniswap_factory.clone(),
        web3: web3.clone(),
        chain_id,
    }));
    let fee_calcuator = Arc::new(MinFeeCalculator::new(
        Box::new(price_estimator),
        Box::new(web3.clone()),
        token_a.address(),
    ));
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        db,
        event_updater,
        Box::new(Web3BalanceFetcher::new(web3.clone(), gp_allowance)),
        fee_calcuator.clone(),
    ));

    orderbook::serve_task(
        orderbook.clone(),
        fee_calcuator,
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

    // Drive solution
    let uniswap_liquidity = UniswapLiquidity::new(
        uniswap_factory.clone(),
        uniswap_router.clone(),
        gp_settlement.clone(),
        H160::default(),
        web3.clone(),
        1,
    );
    let solver = solver::naive_solver::NaiveSolver {
        uniswap_router,
        uniswap_factory,
        gpv2_settlement: gp_settlement.clone(),
    };
    let mut driver = solver::driver::Driver::new(
        gp_settlement.clone(),
        uniswap_liquidity,
        create_orderbook_api(),
        Box::new(solver),
        Box::new(web3),
        Duration::from_secs(1),
        Duration::from_secs(30),
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

    let orders = create_orderbook_api().get_orders().await.unwrap();
    assert!(orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}

fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
}

fn create_orderbook_api() -> OrderBookApi {
    solver::orderbook::OrderBookApi::new(
        reqwest::Url::from_str(API_HOST).unwrap(),
        std::time::Duration::from_secs(10),
    )
}
