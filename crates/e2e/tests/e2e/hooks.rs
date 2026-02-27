use {
    alloy::{
        primitives::{Address, Bytes, U256},
        providers::Provider,
    },
    app_data::Hook,
    e2e::setup::{
        OnchainComponents, Services, TIMEOUT, onchain_components, run_test, safe::Safe,
        wait_for_condition,
    },
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::{EcdsaSigningScheme, Signature, hashed_eip712_message},
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    reqwest::StatusCode,
    serde_json::json,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_allowance() {
    run_test(allowance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_signature() {
    run_test(signature).await;
}

#[tokio::test]
#[ignore]
async fn local_node_partial_fills() {
    run_test(partial_fills).await;
}

#[tokio::test]
#[ignore]
async fn local_node_gas_limit() {
    run_test(gas_limit).await;
}

#[tokio::test]
#[ignore]
async fn local_node_quote_verification() {
    run_test(quote_verification).await;
}

async fn gas_limit(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let cow = onchain
        .deploy_cow_weth_pool(1_000_000u64.eth(), 1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts and approve relayer
    cow.fund(trader.address(), 5u64.eth()).await;
    cow.approve(onchain.contracts().allowance, 5u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: *cow.address(),
        sell_amount: 4u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [Hook {
                            target: trader.address(),
                            call_data: Default::default(),
                            gas_limit: 8_000_000,
                        }],
                        "post": [],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let error = services.create_order(&order).await.unwrap_err();
    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    assert!(error.1.contains("TooMuchGas"));
}

async fn allowance(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let cow = onchain
        .deploy_cow_weth_pool(1_000_000u64.eth(), 1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts
    cow.fund(trader.address(), 5u64.eth()).await;

    // Sign a permit pre-interaction for trading.
    let permit = cow
        .permit(&trader, onchain.contracts().allowance, 5u64.eth())
        .await;
    // Setup a malicious interaction for setting approvals to steal funds from
    // the settlement contract.
    let steal_cow = {
        let tx = cow
            .approve(trader.address(), U256::MAX)
            .from(solver.address());
        Hook {
            target: *cow.address(),
            call_data: tx.calldata().to_vec(),
            gas_limit: tx.estimate_gas().await.unwrap(),
        }
    };
    let steal_weth = {
        let approve = onchain
            .contracts()
            .weth
            .approve(trader.address(), U256::MAX);
        Hook {
            target: *onchain.contracts().weth.address(),
            call_data: approve.calldata().to_vec(),
            gas_limit: approve.estimate_gas().await.unwrap(),
        }
    };

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: *cow.address(),
        sell_amount: 5u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [permit, steal_cow],
                        "post": [steal_weth],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    let balance = cow.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(balance, 5u64.eth());

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        cow.balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let balance = onchain
        .contracts()
        .weth
        .balanceOf(trader.address())
        .call()
        .await
        .unwrap();
    assert!(balance >= order.buy_amount);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();

    // Check malicious custom interactions did not work.
    let allowance = cow
        .allowance(
            *onchain.contracts().gp_settlement.address(),
            trader.address(),
        )
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::ZERO);
    let allowance = onchain
        .contracts()
        .weth
        .allowance(
            *onchain.contracts().gp_settlement.address(),
            trader.address(),
        )
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::ZERO);

    // Note that the allowances were set with the `HooksTrampoline` contract!
    // This is OK since the `HooksTrampoline` contract is not used for holding
    // any funds.
    let allowance = cow
        .allowance(*onchain.contracts().hooks.address(), trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::MAX);
    let allowance = onchain
        .contracts()
        .weth
        .allowance(*onchain.contracts().hooks.address(), trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::MAX);
}

async fn signature(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let chain_id = U256::from(web3.provider.get_chain_id().await.unwrap());

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let safe_infra = onchain_components::safe::Infrastructure::new(web3.provider.clone()).await;

    // Prepare the Safe creation transaction, but don't execute it! This will
    // be executed as a pre-hook.
    let safe_creation_builder = safe_infra.factory.createProxy(
        *safe_infra.singleton.address(),
        safe_infra
            .singleton
            .setup(
                vec![trader.address()], // owners
                U256::ONE,              // threshold
                Address::default(),     // delegate call
                Bytes::default(),       // delegate call bytes
                *safe_infra.fallback.address(),
                Address::default(), // relayer payment token
                U256::ZERO,         // relayer payment amount
                Address::default(), // relayer address
            )
            .calldata()
            .clone(),
    );
    let safe_creation =
        onchain_components::alloy::hook_for_transaction(safe_creation_builder.clone())
            .await
            .unwrap();

    // Create a contract instance at the would-be address of the Safe we are
    // creating with the pre-hook.
    let safe_address = safe_creation_builder.clone().call().await.unwrap();
    let safe = Safe::deployed(
        chain_id,
        contracts::GnosisSafe::Instance::new(safe_address, web3.provider.clone()),
        trader.clone(),
    );

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    token.mint(safe.address(), 5u64.eth()).await;

    // Sign an approval transaction for trading. This will be at nonce 0 because
    // it is the first transaction evah!
    let approval_call_data = token
        .approve(onchain.contracts().allowance, 5u64.eth())
        .calldata()
        .to_vec();
    let approval_builder = safe.sign_transaction(*token.address(), approval_call_data, U256::ZERO);
    let call_data = approval_builder.calldata().to_vec();
    let target = approval_builder
        .into_transaction_request()
        .to
        .unwrap()
        .into_to()
        .unwrap();
    let approval = Hook {
        target,
        call_data,
        // The contract isn't deployed, so we can't estimate this. Instead, we
        // just guess an amount that should be high enough.
        gas_limit: 100_000,
    };

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Place Orders
    let mut order = OrderCreation {
        from: Some(safe.address()),
        // Quotes for trades where the pre-interactions deploy a contract
        // at the `from` address currently can't be verified.
        // To not throw an error because we can't get a verifiable quote
        // we make the order partially fillable and sell slightly more than
        // `from` currently has.
        sell_amount: 6u64.eth(),
        partially_fillable: true,
        sell_token: *token.address(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [safe_creation, approval],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    };
    order.signature = Signature::Eip1271(safe.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));

    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    let balance = token.balanceOf(safe.address()).call().await.unwrap();
    assert_eq!(balance, 5u64.eth());

    // Check that the Safe really hasn't been deployed yet.
    let code = web3.provider.get_code_at(safe.address()).await.unwrap();
    assert_eq!(code.0.len(), 0);

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        token
            .balanceOf(safe.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let balance = onchain
        .contracts()
        .weth
        .balanceOf(safe.address())
        .call()
        .await
        .unwrap();
    assert!(balance >= order.buy_amount);

    // Check Safe was deployed
    let code = web3.provider.get_code_at(safe.address()).await.unwrap();
    assert_ne!(code.0.len(), 0);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();
}

async fn partial_fills(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(3u64.eth()).await;

    let counter = contracts::test::Counter::Instance::deploy(web3.provider.clone())
        .await
        .unwrap();

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_token = onchain.contracts().weth.clone();
    sell_token
        .approve(onchain.contracts().allowance, 2u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    sell_token
        .deposit()
        .from(trader.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    let balance_before_first_trade = sell_token.balanceOf(trader.address()).call().await.unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let pre_inc =
        counter.setCounterToBalance("pre".to_string(), *sell_token.address(), trader.address());
    let pre_hook = Hook {
        target: *counter.address(),
        call_data: pre_inc.calldata().to_vec(),
        gas_limit: pre_inc.estimate_gas().await.unwrap(),
    };

    let post_inc =
        counter.setCounterToBalance("post".to_string(), *sell_token.address(), trader.address());
    let post_hook = Hook {
        target: *counter.address(),
        call_data: post_inc.calldata().to_vec(),
        gas_limit: post_inc.estimate_gas().await.unwrap(),
    };

    tracing::info!("Placing order");
    let order = OrderCreation {
        sell_token: *sell_token.address(),
        sell_amount: 2u64.eth(),
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [pre_hook],
                        "post": [post_hook],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    tracing::info!("Waiting for first trade.");
    let trade_happened = || async {
        sell_token
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
    assert_eq!(
        counter.counters("pre".to_string()).call().await.unwrap(),
        balance_before_first_trade
    );
    let post_balance_after_first_trade =
        sell_token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(
        counter.counters("post".to_string()).call().await.unwrap(),
        post_balance_after_first_trade
    );

    tracing::info!("Fund remaining sell balance.");
    sell_token
        .deposit()
        .from(trader.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Waiting for second trade.");
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
    assert_eq!(
        counter.counters("pre".to_string()).call().await.unwrap(),
        balance_before_first_trade
    );
    assert_eq!(
        counter.counters("post".to_string()).call().await.unwrap(),
        sell_token.balanceOf(trader.address()).call().await.unwrap()
    );
}

/// Checks that quotes can be verified which need the pre-hooks
/// to run before the requested trade could be executed.
async fn quote_verification(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let chain_id = U256::from(web3.provider.get_chain_id().await.unwrap());

    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let safe_infra = onchain_components::safe::Infrastructure::new(web3.provider.clone()).await;

    // Prepare the Safe creation transaction, but don't execute it! This will
    // be executed as a pre-hook.
    let safe_creation_builder = safe_infra.factory.createProxy(
        *safe_infra.singleton.address(),
        safe_infra
            .singleton
            .setup(
                vec![trader.address()], // owners
                U256::ONE,              // threshold
                Address::default(),     // delegate call
                Bytes::default(),       // delegate call bytes
                *safe_infra.fallback.address(),
                Address::default(), // relayer payment token
                U256::ZERO,         // relayer payment amount
                Address::default(), // relayer address
            )
            .calldata()
            .clone(),
    );
    let safe_address = safe_creation_builder.clone().call().await.unwrap();
    safe_creation_builder.send_and_watch().await.unwrap();

    let safe = Safe::deployed(
        chain_id,
        contracts::GnosisSafe::Instance::new(safe_address, web3.provider.clone()),
        trader.clone(),
    );

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    token.mint(safe.address(), 5u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 5u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Sign transaction transferring 5 token from the safe to the trader
    // to fund the trade in a pre-hook.
    let transfer_builder = safe.sign_transaction(
        *token.address(),
        token
            .transfer(trader.address(), 5u64.eth())
            .calldata()
            .to_vec(),
        U256::ZERO,
    );
    let call_data = transfer_builder.calldata().to_vec();
    let target = transfer_builder
        .into_transaction_request()
        .to
        .unwrap()
        .into_to()
        .unwrap();
    let transfer = Hook {
        target,
        call_data,
        gas_limit: 100_000,
    };

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let quote = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *token.address(),
            buy_token: *onchain.contracts().weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(5u64.eth()).unwrap(),
                },
            },
            app_data: OrderCreationAppData::Full {
                full: json!({
                    "metadata": {
                        "hooks": {
                            "pre": [transfer],
                        },
                    },
                })
                .to_string(),
            },
            ..Default::default()
        })
        .await
        .unwrap();

    // quote can be verified although the trader only get the necessary
    // sell tokens with a pre-hook
    assert!(quote.verified);
}
