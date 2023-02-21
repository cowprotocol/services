use {
    crate::{
        onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
        services::{solvable_orders, wait_for_condition, API_HOST},
        tx,
    },
    ethcontract::{
        prelude::{Account, Address, PrivateKey, U256},
        transaction::TransactionBuilder,
    },
    model::{
        order::{OrderBuilder, OrderKind, BUY_ETH_ADDRESS},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const TRADER_A_PK: [u8; 32] = [1; 32];
const TRADER_B_PK: [u8; 32] = [2; 32];
const SOLVER_PK: [u8; 32] = [3; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_eth_integration() {
    crate::local_node::test(eth_integration).await;
}

async fn eth_integration(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PK).unwrap(), None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);
    for account in [&trader_a, &trader_b, &solver] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(1))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();

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

    token.mint(trader_a.address(), to_wei(51)).await;
    token.mint(trader_b.address(), to_wei(51)).await;
    let token = token.contract;

    // Approve GPv2 for trading
    tx!(trader_a, token.approve(contracts.allowance, to_wei(51)));
    tx!(trader_b, token.approve(contracts.allowance, to_wei(51)));

    let trader_a_eth_balance_before = web3.eth().balance(trader_a.address(), None).await.unwrap();
    let trader_b_eth_balance_before = web3.eth().balance(trader_b.address(), None).await.unwrap();

    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

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
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_A_PK).unwrap()),
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
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_B_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_buy_eth_b)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    tracing::info!("Waiting for trade.");
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();
    crate::services::start_old_driver(&contracts, &SOLVER_PK, &[]);
    let trade_happened = || async {
        let balance_a = web3.eth().balance(trader_a.address(), None).await.unwrap();
        let balance_b = web3.eth().balance(trader_b.address(), None).await.unwrap();
        balance_a != trader_a_eth_balance_before && balance_b != trader_b_eth_balance_before
    };
    wait_for_condition(Duration::from_secs(10), trade_happened)
        .await
        .unwrap();

    // Check matching
    let trader_a_eth_balance_after = web3.eth().balance(trader_a.address(), None).await.unwrap();
    let trader_b_eth_balance_after = web3.eth().balance(trader_b.address(), None).await.unwrap();
    assert_eq!(
        trader_a_eth_balance_after - trader_a_eth_balance_before,
        to_wei(49)
    );
    assert_eq!(
        trader_b_eth_balance_after - trader_b_eth_balance_before,
        49_800_747_827_208_136_744_u128.into()
    );
}
