use crate::{
    onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
    services::{
        create_orderbook_api, setup_naive_solver_uniswapv2_driver, wait_for_solvable_orders,
        OrderbookServices, API_HOST,
    },
    tx,
};
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use model::{
    order::{OrderBuilder, OrderKind, BUY_ETH_ADDRESS},
    signature::EcdsaSigningScheme,
};
use secp256k1::SecretKey;
use serde_json::json;
use shared::{ethrpc::Web3, http_client::HttpClientFactory, maintenance::Maintaining};
use web3::signing::SecretKeyRef;

const TRADER_BUY_ETH_A_PK: [u8; 32] = [1; 32];
const TRADER_BUY_ETH_B_PK: [u8; 32] = [2; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_eth_integration() {
    crate::local_node::test(eth_integration).await;
}

async fn eth_integration(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_buy_eth_a =
        Account::Offline(PrivateKey::from_raw(TRADER_BUY_ETH_A_PK).unwrap(), None);
    let trader_buy_eth_b =
        Account::Offline(PrivateKey::from_raw(TRADER_BUY_ETH_B_PK).unwrap(), None);

    // Create & mint tokens to trade, pools for fee connections
    let token = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;

    token.mint(trader_buy_eth_a.address(), to_wei(51)).await;
    token.mint(trader_buy_eth_b.address(), to_wei(51)).await;
    let token = token.contract;

    // Approve GPv2 for trading
    tx!(
        trader_buy_eth_a,
        token.approve(contracts.allowance, to_wei(51))
    );
    tx!(
        trader_buy_eth_b,
        token.approve(contracts.allowance, to_wei(51))
    );

    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, false).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Test quote
    let client_ref = &client;
    let quote = |sell_token, buy_token| async move {
        let body = json!({
                "sellToken": sell_token,
                "buyToken": buy_token,
                "from": Address::default(),
                "kind": "sell",
                "sellAmountAfterFee": to_wei(42).to_string(),
        });
        client_ref
            .post(&format!("{}{}", API_HOST, "/api/v1/quote",))
            .json(&body)
            .send()
            .await
            .unwrap()
    };
    let response = quote(token.address(), BUY_ETH_ADDRESS).await;
    if response.status() != 200 {
        tracing::error!("{}", response.text().await.unwrap());
        panic!("bad status");
    }
    // Eth is only supported as the buy token
    let response = quote(BUY_ETH_ADDRESS, token.address()).await;
    if response.status() != 400 {
        tracing::error!("{}", response.text().await.unwrap());
        panic!("bad status");
    }

    // Place Orders
    assert_ne!(contracts.weth.address(), BUY_ETH_ADDRESS);
    let order_buy_eth_a = OrderBuilder::default()
        .with_kind(OrderKind::Buy)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(to_wei(1))
        .with_buy_token(BUY_ETH_ADDRESS)
        .with_buy_amount(to_wei(49))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_BUY_ETH_A_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_buy_eth_a)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);
    let order_buy_eth_b = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(to_wei(1))
        .with_buy_token(BUY_ETH_ADDRESS)
        .with_buy_amount(to_wei(49))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_BUY_ETH_B_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_buy_eth_b)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    wait_for_solvable_orders(&client, 2).await.unwrap();

    // Drive solution
    let mut driver = setup_naive_solver_uniswapv2_driver(
        &web3,
        &contracts,
        base_tokens,
        block_stream,
        solver_account,
    )
    .await;
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
        U256::from(49_800_747_827_208_136_744_u128)
    );

    // Drive orderbook in order to check that all orders were settled
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    let auction = create_orderbook_api().get_auction().await.unwrap();
    assert!(auction.auction.orders.is_empty());
}
