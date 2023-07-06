use {
    crate::setup::*,
    ethcontract::prelude::U256,
    model::{
        order::OrderCreationAppData,
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, SellAmount},
    },
    serde_json::json,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
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
    let solver_endpoint = colocation::start_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(onchain.contracts(), &solver_endpoint, &solver);

    let services = Services::new(onchain.contracts()).await;
    services
        .start_api(vec!["--enable-custom-interactions=true".to_string()])
        .await;

    tracing::info!("Quoting order");
    let request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee { value: to_wei(1) },
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
                    "backend": {
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
                    "backend": {
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
