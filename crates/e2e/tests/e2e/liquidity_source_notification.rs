use {
    alloy::{
        primitives::{Address, Bytes, U256, address},
        providers::ext::{AnvilApi, ImpersonateConfig},
        signers::SignerSync,
    },
    autopilot::config::{
        Configuration,
        solver::{Account, Solver},
    },
    chrono::Utc,
    contracts::alloy::{ERC20, LiquoriceSettlement},
    driver::infra,
    e2e::{
        api,
        setup::{
            OnchainComponents,
            Services,
            TIMEOUT,
            colocation::{self, SolverEngine},
            mock::Mock,
            run_forked_test_with_block_number,
            wait_for_condition,
        },
    },
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    solvers_dto::solution::Solution,
    std::{collections::HashMap, str::FromStr},
    url::Url,
};

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 23326100;
pub const USDT_WHALE: Address = address!("6AC38D1b2f0c0c3b9E816342b1CA14d91D5Ff60B");
pub const USDC_WHALE: Address = address!("01b8697695eab322a339c4bf75740db75dc9375e");

#[tokio::test]
#[ignore]
async fn forked_node_liquidity_source_notification_mainnet() {
    run_forked_test_with_block_number(
        liquidity_source_notification,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK,
    )
    .await
}

async fn liquidity_source_notification(web3: Web3) {
    // Start onchain components
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    // Define trade params
    let trade_amount = 500u64.matom();

    // Create parties accounts
    // solver - represents both baseline solver engine for quoting and liquorice
    // solver engine for solving
    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;
    // trader - the account that will place CoW order
    // liquorice_maker - the account that will place Liquorice order to fill CoW
    // order with
    let [trader, liquorice_maker] = onchain.make_accounts(1u64.eth()).await;

    // Access trade tokens contracts
    let token_usdc = ERC20::Instance::new(
        address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        web3.provider.clone(),
    );

    let token_usdt = ERC20::Instance::new(
        address!("dac17f958d2ee523a2206206994597c13d831ec7"),
        web3.provider.clone(),
    );

    // CoW onchain setup
    // Fund trader
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            token_usdc
                .transfer(trader.address(), trade_amount)
                .from(USDC_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Fund solver
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            token_usdc
                .transfer(solver.address(), trade_amount)
                .from(USDC_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Trader gives approval to the CoW allowance contract
    token_usdc
        .approve(onchain.contracts().allowance, U256::MAX)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Liquorice onchain setup

    // Liquorice settlement contract through which we will trade with the
    // `liquorice_maker`
    let liquorice_settlement = LiquoriceSettlement::Instance::deployed(&web3.provider)
        .await
        .unwrap();

    let liquorice_balance_manager_address = liquorice_settlement
        .BALANCE_MANAGER()
        .call()
        .await
        .expect("no balance manager found");

    // Fund `liquorice_maker`
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            token_usdt
                .transfer(liquorice_maker.address(), trade_amount)
                .from(USDT_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Maker gives approval to the Liquorice balance manager contract
    token_usdt
        .approve(liquorice_balance_manager_address, U256::MAX)
        .from(liquorice_maker.address())
        .send_and_watch()
        .await
        .unwrap();

    // Liquorice API setup
    let liquorice_api = api::liquorice::server::LiquoriceApi::start().await;

    // CoW services setup
    let liquorice_solver_api_mock = Mock::default();
    let services = Services::new(&onchain).await;

    colocation::start_driver_with_config_override(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            SolverEngine {
                name: "liquorice_solver".into(),
                account: solver.clone(),
                endpoint: liquorice_solver_api_mock.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
                haircut_bps: 0,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
        Some(&format!(
            r#"
[liquidity-sources-notifier]
[liquidity-sources-notifier.liquorice]
base-url = "http://0.0.0.0:{}"
api-key = ""
http-timeout = "10s"
        "#,
            liquorice_api.port
        )),
    );
    let config_file = Configuration {
        drivers: vec![Solver::new(
            "liquorice_solver".to_string(),
            Url::from_str("http://localhost:11088/liquorice_solver").unwrap(),
            Account::Address(solver.address()),
        )],
    }
    .to_temp_path();

    services
        .start_autopilot(
            None,
            vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                format!("--config={}", config_file.path().display()),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Create CoW order
    let order_id = {
        let order = OrderCreation {
            sell_token: *token_usdc.address(),
            sell_amount: trade_amount,
            buy_token: *token_usdt.address(),
            buy_amount: trade_amount,
            valid_to: model::time::now_in_epoch_seconds() + 300,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        );
        services.create_order(&order).await.unwrap()
    };

    // Prepare Liquorice solution

    // Create Liquorice order
    let liquorice_order = LiquoriceSettlement::ILiquoriceSettlement::Single {
        rfqId: "c99d2e3f-702b-49c9-8bb8-43775770f2f3".to_string(),
        nonce: U256::from(0),
        trader: *onchain.contracts().gp_settlement.address(),
        effectiveTrader: *onchain.contracts().gp_settlement.address(),
        baseToken: *token_usdc.address(),
        quoteToken: *token_usdt.address(),
        baseTokenAmount: trade_amount,
        quoteTokenAmount: trade_amount,
        minFillAmount: U256::from(1),
        quoteExpiry: U256::from(Utc::now().timestamp() as u64 + 10),
        recipient: liquorice_maker.address(),
    };

    // Create calldata
    let liquorice_solution_calldata = {
        let liquorice_order_hash = liquorice_settlement
            .hashSingleOrder(liquorice_order.clone())
            .call()
            .await
            .unwrap();

        // Create Liquorice order signature
        let liquorice_maker_address = liquorice_maker.address();
        let signer = liquorice_maker.signer;
        let liquorice_order_signature = signer.sign_hash_sync(&liquorice_order_hash).unwrap();

        // Create Liquorice settlement calldata
        liquorice_settlement
            .settleSingle(
                liquorice_maker_address,
                liquorice_order.clone(),
                LiquoriceSettlement::Signature::TypedSignature {
                    signatureType: 3,   // EIP712
                    transferCommand: 1, // SIMPLE_TRANSFER
                    signatureBytes: liquorice_order_signature.as_bytes().into(),
                },
                liquorice_order.quoteTokenAmount,
                // Taker signature is not used in this use case
                LiquoriceSettlement::Signature::TypedSignature {
                    signatureType: 0,
                    transferCommand: 0,
                    signatureBytes: Bytes::from(vec![0u8; 65]),
                },
            )
            .calldata()
            .to_vec()
    };

    // Submit solution to CoW
    liquorice_solver_api_mock.configure_solution(Some(Solution {
        id: 1,
        prices: HashMap::from([
            (*token_usdc.address(), 11u64.eth()),
            (*token_usdt.address(), 10u64.eth()),
        ]),
        trades: vec![solvers_dto::solution::Trade::Fulfillment(
            solvers_dto::solution::Fulfillment {
                executed_amount: trade_amount,
                fee: Some(U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_id.0),
            },
        )],
        pre_interactions: vec![],
        interactions: vec![solvers_dto::solution::Interaction::Custom(
            solvers_dto::solution::CustomInteraction {
                target: *liquorice_settlement.address(),
                calldata: liquorice_solution_calldata,
                value: U256::ZERO,
                allowances: vec![solvers_dto::solution::Allowance {
                    token: *token_usdc.address(),
                    spender: liquorice_balance_manager_address,
                    amount: trade_amount,
                }],
                inputs: vec![],
                outputs: vec![],
                internalize: false,
            },
        )],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
    }));

    // Wait for trade
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let trade = services.get_trades(&order_id).await.unwrap().pop()?;
        Some(
            services
                .get_solver_competition(trade.tx_hash?)
                .await
                .is_ok(),
        )
    })
    .await
    .unwrap();

    let trade = services.get_trades(&order_id).await.unwrap().pop();
    assert!(trade.is_some());

    // Ensure that notification was delivered to Liquorice API
    wait_for_condition(TIMEOUT, || async {
        let state = liquorice_api.get_state().await;
        !state.notification_requests.is_empty()
    })
    .await
    .unwrap();

    let notification = liquorice_api
        .get_state()
        .await
        .notification_requests
        .first()
        .cloned()
        .unwrap();

    use infra::notify::liquidity_sources::liquorice::client::request::v1::intent_origin::notification::post::{Content, Settle};
    assert!(matches!(notification.content, Content::Settle(Settle {
        rfq_ids,
        ..
    }) if rfq_ids.contains(&liquorice_order.rfqId)));
}
