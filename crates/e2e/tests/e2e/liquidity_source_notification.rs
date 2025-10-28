use {
    alloy::primitives::Bytes,
    chrono::Utc,
    contracts::{
        ERC20,
        alloy::{ILiquoriceSettlement, InstanceExt},
    },
    driver::{domain::eth::H160, infra},
    e2e::{
        api,
        nodes::forked_node::ForkedNodeApi,
        setup::{
            OnchainComponents,
            Services,
            TIMEOUT,
            colocation::{self, SolverEngine},
            mock::Mock,
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
        },
        tx,
    },
    ethcontract::prelude::U256,
    ethrpc::{
        Web3,
        alloy::conversions::{IntoAlloy, IntoLegacy},
    },
    hex_literal::hex,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    solvers_dto::solution::Solution,
    std::collections::HashMap,
    web3::signing::SecretKeyRef,
};

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 23326100;
pub const USDT_WHALE: H160 = H160(hex!("6AC38D1b2f0c0c3b9E816342b1CA14d91D5Ff60B"));
pub const USDC_WHALE: H160 = H160(hex!("01b8697695eab322a339c4bf75740db75dc9375e"));

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
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    // Define trade params
    let trade_amount = to_wei_with_exp(5, 8);

    // Create parties accounts
    // solver - represents both baseline solver engine for quoting and liquorice
    // solver engine for solving
    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    // trader - the account that will place CoW order
    // liquorice_maker - the account that will place Liquorice order to fill CoW
    // order with
    let [trader, liquorice_maker] = onchain.make_accounts(to_wei(1)).await;

    // Access trade tokens contracts
    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    // CoW onchain setup
    {
        // Fund trader
        let usdc_whale = forked_node_api.impersonate(&USDC_WHALE).await.unwrap();
        tx!(
            usdc_whale,
            token_usdc.transfer(trader.address(), trade_amount)
        );

        // Fund solver
        tx!(
            usdc_whale,
            token_usdc.transfer(solver.address(), trade_amount)
        );

        // Trader gives approval to the CoW allowance contract
        tx!(
            trader.account(),
            token_usdc.approve(onchain.contracts().allowance, U256::MAX)
        );
    }

    // Liquorice onchain setup

    // Liquorice settlement contract through which we will trade with the
    // `liquorice_maker`
    let liquorice_settlement = ILiquoriceSettlement::Instance::deployed(&web3.alloy)
        .await
        .unwrap();
    let liquorice_balance_manager_address = liquorice_settlement
        .BALANCE_MANAGER()
        .call()
        .await
        .expect("no balance manager found")
        .into_legacy();

    // Fund `liquorice_maker`
    {
        let usdt_whale = forked_node_api.impersonate(&USDT_WHALE).await.unwrap();
        tx!(
            usdt_whale,
            token_usdt.transfer(liquorice_maker.address(), trade_amount)
        );
    }

    // Maker gives approval to the Liquorice balance manager contract
    tx!(
        liquorice_maker.account(),
        token_usdt.approve(liquorice_balance_manager_address, U256::MAX)
    );

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
    services
        .start_autopilot(
            None,
            vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                format!(
                    "--drivers=liquorice_solver|http://localhost:11088/liquorice_solver|{}",
                    const_hex::encode(solver.address())
                ),
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
            sell_token: token_usdc.address(),
            sell_amount: trade_amount,
            buy_token: token_usdt.address(),
            buy_amount: trade_amount,
            valid_to: model::time::now_in_epoch_seconds() + 300,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        );
        services.create_order(&order).await.unwrap()
    };

    // Prepare Liquorice solution

    // Create Liquorice order
    let liquorice_order = api::liquorice::onchain::order::Single {
        rfq_id: "c99d2e3f-702b-49c9-8bb8-43775770f2f3".to_string(),
        nonce: U256::from(0),
        trader: onchain.contracts().gp_settlement.address().into_legacy(),
        effective_trader: onchain.contracts().gp_settlement.address().into_legacy(),
        base_token: token_usdc.address(),
        quote_token: token_usdt.address(),
        base_token_amount: trade_amount,
        quote_token_amount: trade_amount,
        min_fill_amount: U256::from(1),
        quote_expiry: U256::from(Utc::now().timestamp() as u64 + 10),
        recipient: liquorice_maker.address(),
    };

    // Create calldata
    let liquorice_solution_calldata = {
        // Create Liquorice order signature
        let liquorice_order_signature = liquorice_order.sign(
            &api::liquorice::onchain::DomainSeparator::new(
                1,
                liquorice_settlement.address().into_legacy(),
            ),
            liquorice_order.hash(),
            &liquorice_maker,
        );

        // Create Liquorice settlement calldata
        liquorice_settlement
            .settleSingle(
                liquorice_maker.address().into_alloy(),
                ILiquoriceSettlement::ILiquoriceSettlement::Single {
                    rfqId: liquorice_order.rfq_id.clone(),
                    nonce: liquorice_order.nonce.into_alloy(),
                    trader: liquorice_order.trader.into_alloy(),
                    effectiveTrader: liquorice_order.effective_trader.into_alloy(),
                    baseToken: liquorice_order.base_token.into_alloy(),
                    quoteToken: liquorice_order.quote_token.into_alloy(),
                    baseTokenAmount: liquorice_order.base_token_amount.into_alloy(),
                    quoteTokenAmount: liquorice_order.quote_token_amount.into_alloy(),
                    minFillAmount: liquorice_order.min_fill_amount.into_alloy(),
                    quoteExpiry: liquorice_order.quote_expiry.into_alloy(),
                    recipient: liquorice_order.recipient.into_alloy(),
                },
                ILiquoriceSettlement::Signature::TypedSignature {
                    signatureType: liquorice_order_signature.signature_type,
                    transferCommand: liquorice_order_signature.transfer_command,
                    signatureBytes: Bytes::from(liquorice_order_signature.signature_bytes.0),
                },
                liquorice_order.quote_token_amount.into_alloy(),
                // Taker signature is not used in this use case
                ILiquoriceSettlement::Signature::TypedSignature {
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
            (token_usdc.address(), to_wei(11)),
            (token_usdt.address(), to_wei(10)),
        ]),
        trades: vec![solvers_dto::solution::Trade::Fulfillment(
            solvers_dto::solution::Fulfillment {
                executed_amount: trade_amount,
                fee: Some(0.into()),
                order: solvers_dto::solution::OrderUid(order_id.0),
            },
        )],
        pre_interactions: vec![],
        interactions: vec![solvers_dto::solution::Interaction::Custom(
            solvers_dto::solution::CustomInteraction {
                target: liquorice_settlement.address().into_legacy(),
                calldata: liquorice_solution_calldata,
                value: 0.into(),
                allowances: vec![solvers_dto::solution::Allowance {
                    token: token_usdc.address(),
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
    }) if rfq_ids.contains(&liquorice_order.rfq_id)));
}
