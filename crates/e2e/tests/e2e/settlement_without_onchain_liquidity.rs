use {
    crate::{
        onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
        services::{solvable_orders, wait_for_condition, API_HOST},
    },
    ethcontract::{
        prelude::{Account, PrivateKey, U256},
        transaction::TransactionBuilder,
    },
    hex_literal::hex,
    model::{
        order::{OrderBuilder, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const TRADER_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const SOLVER_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000003");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_onchain_settlement_without_liquidity() {
    crate::local_node::test(onchain_settlement_without_liquidity).await;
}

async fn onchain_settlement_without_liquidity(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PK).unwrap(), None);
    let trader = Account::Offline(PrivateKey::from_raw(TRADER_PK).unwrap(), None);
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
    let token_a = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    let token_b = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader.address(), to_wei(10)).await;
    token_b
        .mint(contracts.gp_settlement.address(), to_wei(100))
        .await;
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    let token_a = token_a.contract;
    let token_b = token_b.contract;
    let settlement_contract_balance_before = token_b
        .balance_of(contracts.gp_settlement.address())
        .call()
        .await
        .unwrap();

    // Create and fund Uniswap pool
    tx!(
        solver,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver,
        token_a.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        token_b.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        contracts.uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader, token_a.approve(contracts.allowance, to_wei(10)));

    // Place Orders
    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(9))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(5))
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
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 1
    })
    .await
    .unwrap();
    crate::services::start_old_driver(
        &contracts,
        &SOLVER_PK,
        &[format!(
            "--market-makable-tokens={:?},{:?}",
            token_a.address(),
            token_b.address()
        )],
    );
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();

    // Check that trader traded.
    let balance = token_a
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch trader TokenA's balance");
    assert_eq!(balance, U256::from(0_u128));

    let balance = token_b
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch trader TokenB's balance");
    assert!(balance > U256::zero());

    // Check that settlement buffers were traded.
    let settlement_contract_balance_after = token_b
        .balance_of(contracts.gp_settlement.address())
        .call()
        .await
        .unwrap();
    // Would fail if the settlement didn't internalize the uniswap interaction.
    assert!(settlement_contract_balance_before > settlement_contract_balance_after);
}
