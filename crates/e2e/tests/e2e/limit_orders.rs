use crate::services::{deploy_mintable_token, to_wei, OrderbookServices, API_HOST};
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use hex_literal::hex;
use model::{
    order::{Order, OrderBuilder, OrderClass, OrderKind},
    signature::EcdsaSigningScheme,
};
use secp256k1::SecretKey;
use shared::{ethrpc::Web3, http_client::HttpClientFactory};
use web3::signing::SecretKeyRef;

const TRADER_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn single_limit_order() {
    crate::local_node::test(single_limit_order_test).await;
}

#[tokio::test]
#[ignore]
async fn too_many_limit_orders() {
    crate::local_node::test(too_many_limit_orders_test).await;
}

async fn single_limit_order_test(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_account = Account::Offline(PrivateKey::from_raw(TRADER_PK).unwrap(), None);

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
        token_b.mint(contracts.gp_settlement.address(), to_wei(100))
    );

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
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
        token_a.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        contracts.uniswap_router.add_liquidity(
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
            token.approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx_value!(solver_account, to_wei(100_000), contracts.weth.deposit());
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
    }

    // Approve GPv2 for trading
    tx!(
        trader_account,
        token_a.approve(contracts.allowance, to_wei(100))
    );

    // Place Orders
    let _services = OrderbookServices::new(&web3, &contracts, true).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(1200))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);
    let order_id: String = placement.json().await.unwrap();

    let order: Order = client
        .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_id}"))
        .json(&order)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(order.metadata.class, OrderClass::Limit);

    // TODO #643 Extend this to actually simulate driving the solution. Look at other E2E tests for
    // an example of how this can be done.
}

async fn too_many_limit_orders_test(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_account = Account::Offline(PrivateKey::from_raw(TRADER_PK).unwrap(), None);

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
        token_b.mint(contracts.gp_settlement.address(), to_wei(100))
    );

    // Approve GPv2 for trading
    tx!(
        trader_account,
        token_a.approve(contracts.allowance, to_wei(100))
    );

    // Place Orders
    let _services = OrderbookServices::new(&web3, &contracts, true).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(1200))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);

    // Attempt to place another order, but the orderbook is configured to allow only one limit
    // order per user.
    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(1200))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 400);
    assert!(placement
        .text()
        .await
        .unwrap()
        .contains("TooManyLimitOrders"));
}
