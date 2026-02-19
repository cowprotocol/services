use {
    ::alloy::{
        primitives::{Address, B256, U256},
        providers::{Provider, ext::TxPoolApi},
        rpc::{
            client::PollerStream,
            types::{Transaction, TransactionReceipt},
        },
    },
    e2e::setup::*,
    ethrpc::alloy::{CallBuilderExt, EvmProviderExt},
    futures::StreamExt,
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, testing::ApproxEq, units::EthUnit},
    shared::web3::Web3,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_on_expiry() {
    run_test(test_cancel_on_expiry).await;
}

#[tokio::test]
#[ignore]
async fn local_node_execute_same_sell_and_buy_token() {
    run_test(test_execute_same_sell_and_buy_token).await;
}

#[tokio::test]
#[ignore]
async fn local_node_submit_same_sell_and_buy_token_order_without_quote() {
    run_test(test_submit_same_sell_and_buy_token_order_without_quote).await;
}

async fn test_cancel_on_expiry(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let nonce = solver.nonce(&web3).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Placing order");
    let balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(balance, U256::ZERO);
    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 2u64.eth(),
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    // Start tracking confirmed blocks so we can find the transaction later
    let block_stream = web3
        .provider
        .watch_blocks()
        .await
        .expect("must be able to create blocks filter")
        .into_stream();

    // Wait for settlement tx to appear in txpool
    wait_for_condition(TIMEOUT, || async {
        get_pending_tx(solver.address(), &web3).await.is_some()
    })
    .await
    .unwrap();

    // Restart mining, but with blocks that are too small to fit the settlement
    web3.provider
        .evm_set_block_gas_limit(100_000)
        .await
        .expect("Must be able to set block gas limit");
    web3.provider
        .evm_set_interval_mining(1)
        .await
        .expect("Must be able to set mining interval");

    // Wait for cancellation tx to appear
    wait_for_condition(TIMEOUT, || async { solver.nonce(&web3).await == nonce + 1 })
        .await
        .unwrap();

    // Check that it's actually a cancellation
    let tx = tokio::time::timeout(
        TIMEOUT,
        get_confirmed_transaction(solver.address(), &web3, block_stream),
    )
    .await
    .unwrap();
    assert_eq!(tx.to, Some(solver.address()))
}

async fn test_submit_same_sell_and_buy_token_order_without_quote(web3: Web3) {
    use {
        autopilot::config::{
            Configuration,
            solver::{Account, Solver},
        },
        std::str::FromStr,
        url::Url,
    };

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::new(
            "test_solver".to_string(),
            Url::from_str("http://localhost:11088/test_solver").unwrap(),
            Account::Address(solver.address()),
        )],
        ..Default::default()
    }
    .to_cli_args();
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec!["--same-tokens-policy=allow-sell".to_string()],
                autopilot: vec![config_arg],
            },
            solver.clone(),
        )
        .await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    let initial_balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(initial_balance, 10u64.eth());

    let sell_amount = 1u64.eth(); // Sell 1 eth
    let buy_amount = 0.99.eth(); // For 0.99 ETH, for order to execute

    tracing::info!("Placing order");
    let order = OrderCreation {
        sell_token: *token.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();
    // Start tracking confirmed blocks so we can find the transaction later
    let block_stream = web3
        .provider
        .watch_blocks()
        .await
        .expect("must be able to create blocks filter")
        .into_stream();

    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;

    // Wait for settlement tx to appear in txpool
    wait_for_condition(TIMEOUT, || async {
        get_pending_tx(solver.address(), &web3).await.is_some()
    })
    .await
    .unwrap();

    // Continue mining to confirm the settlement
    web3.provider
        .evm_set_automine(true)
        .await
        .expect("Must be able to enable automine");

    // Wait for the settlement to be confirmed on chain
    let tx = tokio::time::timeout(
        Duration::from_secs(5),
        get_confirmed_transaction(solver.address(), &web3, block_stream),
    )
    .await
    .unwrap();

    // Verify the transaction is to the settlement contract (not a cancellation)
    assert_eq!(tx.to, Some(*onchain.contracts().gp_settlement.address()));

    // Verify that the balance changed (settlement happened on chain)
    let trade_happened = || async {
        let balance = token.balanceOf(trader.address()).call().await.unwrap();
        // Balance should change due to fees even if sell token == buy token
        balance != initial_balance
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let final_balance = token.balanceOf(trader.address()).call().await.unwrap();
    tracing::info!(?initial_balance, ?final_balance, "Trade completed");

    // Verify that the balance changed (settlement happened on chain)
    assert!(
        final_balance < initial_balance,
        "Final balance should be smaller than initial balance due to fees"
    );
}

async fn test_execute_same_sell_and_buy_token(web3: Web3) {
    use {
        autopilot::config::{
            Configuration,
            solver::{Account, Solver},
        },
        std::str::FromStr,
        url::Url,
    };

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::new(
            "test_solver".to_string(),
            Url::from_str("http://localhost:11088/test_solver").unwrap(),
            Account::Address(solver.address()),
        )],
        ..Default::default()
    }
    .to_cli_args();
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec!["--same-tokens-policy=allow-sell".to_string()],
                autopilot: vec![config_arg],
            },
            solver.clone(),
        )
        .await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Quoting");
    let quote_sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *token.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(quote_sell_amount).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    tracing::info!(?quote_response);
    assert!(quote_response.id.is_some());
    assert!(quote_response.verified);
    assert!(quote_response.quote.buy_amount < quote_sell_amount);

    {
        // check that a different receiver does not affect the quoted amount
        let quote_request_different_receiver = OrderQuoteRequest {
            receiver: Some(Address::repeat_byte(0x01)),
            ..quote_request
        };
        let quote_response_different_receiver = services
            .submit_quote(&quote_request_different_receiver)
            .await
            .unwrap();

        tracing::info!(?quote_response_different_receiver);
        assert!(quote_response_different_receiver.id.is_some());
        assert!(quote_response_different_receiver.verified);
        assert!(
            quote_response_different_receiver
                .quote
                .buy_amount
                .is_approx_eq(&quote_response.quote.buy_amount, Some(0.0001))
        );
        assert!(
            quote_response_different_receiver
                .quote
                .sell_amount
                .is_approx_eq(&quote_response.quote.sell_amount, Some(0.0001))
        );
    }

    let quote_metadata =
        crate::database::quote_metadata(services.db(), quote_response.id.unwrap()).await;
    assert!(quote_metadata.is_some());
    tracing::debug!(?quote_metadata);

    tracing::info!("Placing order");
    let initial_balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(initial_balance, 10u64.eth());

    let order = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: *token.address(),
        buy_token: *token.address(),
        quote_id: quote_response.id,
        sell_amount: quote_sell_amount,
        buy_amount: quote_response.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    assert!(services.create_order(&order).await.is_ok());

    // Start tracking confirmed blocks so we can find the transaction later
    let block_stream = web3
        .provider
        .watch_blocks()
        .await
        .expect("must be able to create blocks filter")
        .into_stream();

    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;

    // Wait for settlement tx to appear in txpool
    wait_for_condition(TIMEOUT, || async {
        get_pending_tx(solver.address(), &web3).await.is_some()
    })
    .await
    .unwrap();

    // Continue mining to confirm the settlement
    web3.provider
        .evm_set_automine(true)
        .await
        .expect("Must be able to enable automine");

    // Wait for the settlement to be confirmed on chain
    let tx = tokio::time::timeout(
        Duration::from_secs(5),
        get_confirmed_transaction(solver.address(), &web3, block_stream),
    )
    .await
    .unwrap();

    // Verify the transaction is to the settlement contract (not a cancellation)
    assert_eq!(tx.to, Some(*onchain.contracts().gp_settlement.address()));

    // Verify that the balance changed (settlement happened on chain)
    let trade_happened = || async {
        let balance = token.balanceOf(trader.address()).call().await.unwrap();
        // Balance should change due to fees even if sell token == buy token
        balance != initial_balance
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let final_balance = token.balanceOf(trader.address()).call().await.unwrap();
    tracing::info!(?initial_balance, ?final_balance, "Trade completed");

    // Verify that the balance changed (settlement happened on chain)
    assert!(
        final_balance < initial_balance,
        "Final balance should be smaller than initial balance due to fees"
    );
}

async fn get_pending_tx(account: Address, web3: &Web3) -> Option<Transaction> {
    let txpool = web3
        .provider
        .txpool_content()
        .await
        .expect("must be able to inspect mempool");
    txpool.pending.get(&account)?.values().next().cloned()
}

async fn get_confirmed_transaction(
    account: Address,
    web3: &Web3,
    block_hash_stream: PollerStream<Vec<B256>>,
) -> TransactionReceipt {
    let mut block_hash_stream = Box::pin(block_hash_stream);
    loop {
        let block_hashes = block_hash_stream.next().await.unwrap();
        for block_hash in block_hashes {
            let transaction_senders = web3
                .provider
                .get_block_receipts(block_hash.into())
                .await
                .unwrap()
                .into_iter()
                .flatten();

            for tx in transaction_senders {
                if tx.from == account {
                    return tx;
                }
            }
        }
    }
}
