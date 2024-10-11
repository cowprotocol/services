use {
    e2e::{setup::*, tx, tx_value},
    ethcontract::prelude::U256,
    model::{
        order::OrderCreationAppData,
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, SellAmount},
    },
    number::nonzero::U256 as NonZeroU256,
    serde_json::json,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_uses_stale_liquidity() {
    run_test(uses_stale_liquidity).await;
}

// Test that quoting works as expected, specifically, that we can quote for a
// token pair and additional gas from ERC-1271 and hooks are included in the
// quoted fee amount.
async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    tracing::info!("Quoting order");
    let request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(to_wei(1)).unwrap(),
            },
        },
        ..Default::default()
    };

    let with_eip1271 = services
        .submit_quote(&OrderQuoteRequest {
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: 50_000,
            },
            ..request.clone()
        })
        .await
        .unwrap();

    let with_hooks = services
        .submit_quote(&OrderQuoteRequest {
            app_data: OrderCreationAppData::Full {
                full: serde_json::to_string(&json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                            "post": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                        },
                    },
                }))
                .unwrap(),
            },
            ..request.clone()
        })
        .await
        .unwrap();

    let with_both = services
        .submit_quote(&OrderQuoteRequest {
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: 50_000,
            },
            app_data: OrderCreationAppData::Full {
                full: serde_json::to_string(&json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                            "post": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                        },
                    },
                }))
                .unwrap(),
            },
            ..request.clone()
        })
        .await
        .unwrap();

    let base = services.submit_quote(&request).await.unwrap();

    tracing::debug!(
        ?with_eip1271,
        ?with_hooks,
        ?with_both,
        ?base,
        "Computed quotes."
    );

    assert!(base.quote.fee_amount < with_eip1271.quote.fee_amount);
    assert!(base.quote.fee_amount < with_hooks.quote.fee_amount);

    assert!(with_both.quote.fee_amount > with_eip1271.quote.fee_amount);
    assert!(with_both.quote.fee_amount > with_hooks.quote.fee_amount);

    // TODO: test verified quotes, requires state overrides support.
}

async fn uses_stale_liquidity(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(2)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(1))
    );
    tx_value!(
        trader.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let quote = OrderQuoteRequest {
        from: trader.address(),
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::new(to_wei(1)).unwrap(),
            },
        },
        ..Default::default()
    };

    tracing::info!("performining initial quote");
    let first = services.submit_quote(&quote).await.unwrap();

    // Now, we want to manually unbalance the pools and assert that the quote
    // doesn't change (as the price estimation will use stale pricing data).
    onchain
        .mint_token_to_weth_uni_v2_pool(&token, to_wei(1_000))
        .await;

    tracing::info!("performining second quote, which should match first");
    let second = services.submit_quote(&quote).await.unwrap();
    assert_eq!(first.quote.buy_amount, second.quote.buy_amount);

    tracing::info!("waiting for liquidity state to update");
    wait_for_condition(TIMEOUT, || async {
        // Mint blocks until we evict the cached liquidty and fetch the new state.
        onchain.mint_block().await;
        let Ok(next) = services.submit_quote(&quote).await else {
            return false;
        };
        next.quote.buy_amount != first.quote.buy_amount
    })
    .await
    .unwrap();
}
