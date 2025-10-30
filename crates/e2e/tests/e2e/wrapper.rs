use {
    ::alloy::{
        primitives::{Address, address},
        providers::ext::{AnvilApi, ImpersonateConfig},
    },
    app_data::{AppDataHash, hash_full_app_data},
    contracts::alloy::ERC20,
    e2e::setup::*,
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

/// Test that orders can be placed with wrapper contracts specified in the app
/// data. This replicates the functionality of the playground/test_wrapper.sh
/// script as a proper E2E test.

/// The block number from which we will fetch state for the forked test.
const FORK_BLOCK_MAINNET: u64 = 23688436;
/// EmptyWrapper contract address deployed on mainnet.
const EMPTY_WRAPPER_MAINNET: Address = address!("751871E9cA28B441Bb6d3b7C4255cf2B5873d56a");

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_wrapper() {
    run_forked_test_with_block_number(
        forked_mainnet_wrapper_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn forked_mainnet_wrapper_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(2)).await;

    let token_weth = onchain.contracts().weth.clone();
    let token_usdc = ERC20::Instance::new(
        address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        web3.alloy.clone(),
    );

    // Authorize the empty wrapper as a solver
    web3.alloy
        .anvil_send_impersonated_transaction_with_config(
            onchain
                .contracts()
                .gp_authenticator
                .addSolver(EMPTY_WRAPPER_MAINNET)
                .from(
                    onchain
                        .contracts()
                        .gp_authenticator
                        .manager()
                        .call()
                        .await
                        .unwrap(),
                )
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: Some(to_wei(1).into_alloy()),
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Trader deposits ETH to get WETH
    web3.alloy
        .anvil_send_impersonated_transaction_with_config(
            token_weth
                .deposit()
                .value(to_wei(1).into_alloy())
                .from(trader.address().into_alloy())
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Approve GPv2 for trading
    token_weth
        .approve(
            onchain.contracts().allowance.into_alloy(),
            to_wei(1).into_alloy(),
        )
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // Start services
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    // Create app data with the deployed EmptyWrapper contract
    let app_data = json!({
        "version": "0.9.0",
        "metadata": {
            "wrappers": [{
                "address": format!("{:?}", EMPTY_WRAPPER_MAINNET),
                "data": "0xbeef",
                "isOmittable": false,
            }]
        }
    })
    .to_string();

    let app_data_hash = AppDataHash(hash_full_app_data(app_data.as_bytes()));

    // Warm up co-located driver by quoting the order
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: token_weth.address().into_legacy(),
            buy_token: token_usdc.address().into_legacy(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            app_data: OrderCreationAppData::Both {
                full: app_data.clone(),
                expected: app_data_hash,
            },
            ..Default::default()
        })
        .await;

    tracing::info!("Creating order with wrapper in app data");
    let order = OrderCreation {
        app_data: OrderCreationAppData::Both {
            full: app_data.clone(),
            expected: app_data_hash,
        },
        sell_token: token_weth.address().into_legacy(),
        sell_amount: to_wei(1),
        buy_token: token_usdc.address().into_legacy(),
        buy_amount: 1.into(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let sell_token_balance_before = token_weth
        .balanceOf(trader.address().into_alloy())
        .call()
        .await
        .unwrap();
    let buy_token_balance_before = token_usdc
        .balanceOf(trader.address().into_alloy())
        .call()
        .await
        .unwrap();

    // Create the order
    let order_uid = services.create_order(&order).await.unwrap();
    tracing::info!("Order created with UID: {:?}", order_uid);

    // Verify the order was created with correct app data
    let created_order = services.get_order(&order_uid).await.unwrap();
    assert_eq!(created_order.data.app_data, app_data_hash);
    assert_eq!(
        created_order.metadata.full_app_data.as_deref(),
        Some(app_data.as_str())
    );

    // Verify app data can be retrieved
    let retrieved_app_data = services.get_app_data(app_data_hash).await.unwrap();
    assert_eq!(retrieved_app_data, app_data);

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let sell_token_balance_after = token_weth
            .balanceOf(trader.address().into_alloy())
            .call()
            .await
            .unwrap();
        let buy_token_balance_after = token_usdc
            .balanceOf(trader.address().into_alloy())
            .call()
            .await
            .unwrap();

        (sell_token_balance_before > sell_token_balance_after)
            && (buy_token_balance_after > buy_token_balance_before)
    })
    .await
    .unwrap();

    tracing::info!("Order with wrapper successfully traded on forked mainnet");
}
