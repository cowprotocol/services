use contracts::{ERC20Mintable, GPv2Settlement, UniswapV2Factory, UniswapV2Router02};
use ethcontract::prelude::{Account, Address, Http, PrivateKey, Web3, U256};
use hex_literal::hex;
use model::{DomainSeparator, OrderCreationBuilder, OrderKind};
use orderbook::orderbook::OrderBook;
use secp256k1::SecretKey;
use serde_json::json;
use std::{str::FromStr, sync::Arc};
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
    tracing_setup::initialize("warn,orderbook=debug,solver=debug");
    let http = Http::new(NODE_HOST).expect("transport failure");
    let web3 = Web3::new(http);

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
    let gp_settlement = GPv2Settlement::deployed(&web3)
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
    orderbook::serve_task(
        Arc::new(OrderBook::new(domain_separator)),
        API_HOST[7..].parse().expect("Couldn't parse API address"),
    );
    let client = reqwest::Client::new();

    let order_a = OrderCreationBuilder::default()
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
        .build();
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order_a).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    let order_b = OrderCreationBuilder::default()
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
        .build();
    let placement = client
        .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
        .body(json!(order_b).to_string())
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    // Drive solution
    let orderbook_api = solver::orderbook::OrderBookApi::new(
        reqwest::Url::from_str(API_HOST).unwrap(),
        std::time::Duration::from_secs(10),
    );
    let mut driver = solver::driver::Driver::new(gp_settlement, uniswap_router, orderbook_api);
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .expect("Couldn't fetch TokenB's balance");
    assert_eq!(balance, to_wei(80));

    let balance = token_a
        .balance_of(trader_b.address())
        .call()
        .await
        .expect("Couldn't fetch TokenA's balance");
    assert_eq!(balance, 62500000000000000000u128.into());
}

fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
}
