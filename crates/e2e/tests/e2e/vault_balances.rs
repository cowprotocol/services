use {
    crate::{
        onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
        services::{solvable_orders, wait_for_condition, API_HOST},
    },
    ethcontract::{
        prelude::{Account, PrivateKey, U256},
        transaction::TransactionBuilder,
    },
    model::{
        order::{OrderBuilder, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const TRADER: [u8; 32] = [1; 32];
const SOLVER: [u8; 32] = [2; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_vault_balances() {
    crate::local_node::test(vault_balances).await;
}

async fn vault_balances(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER).unwrap(), None);
    let trader = Account::Offline(PrivateKey::from_raw(TRADER).unwrap(), None);
    for account in [&trader, &solver] {
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
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    token.mint(trader.address(), to_wei(10)).await;
    let token = token.contract;

    // Approve GPv2 for trading
    tx!(
        trader,
        token.approve(contracts.balancer_vault.address(), to_wei(10))
    );
    tx!(
        trader,
        contracts
            .balancer_vault
            .set_relayer_approval(trader.address(), contracts.allowance, true)
    );

    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    // Place Orders
    let order = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(9))
        .with_sell_token_balance(SellTokenSource::External)
        .with_fee_amount(to_wei(1))
        .with_buy_token(contracts.weth.address())
        .with_buy_amount(to_wei(8))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);
    let balance_before = contracts
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 1
    })
    .await
    .unwrap();
    crate::services::start_old_driver(&contracts, &SOLVER, &[]);
    crate::services::wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();

    // Check matching
    let balance = token
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch token balance");
    assert_eq!(balance, U256::zero());

    let balance_after = contracts
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(8));
}
