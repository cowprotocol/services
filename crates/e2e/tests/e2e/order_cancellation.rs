use {
    ::alloy::primitives::U256,
    autopilot::config::Configuration,
    database::order_events::OrderEventLabel,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{
            CancellationPayload,
            OrderCancellation,
            OrderCancellations,
            OrderCreation,
            OrderCreationAppData,
            OrderStatus,
            OrderUid,
            SignedOrderCancellations,
        },
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::{EcdsaSignature, EcdsaSigningScheme},
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    serde_json::json,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_order_cancellation() {
    run_test(order_cancellation).await;
}

async fn order_cancellation(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;

    // Approve GPv2 for trading

    token
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver,
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    let config_file = Configuration::default().to_temp_path();
    services
        .start_autopilot(
            None,
            vec![
                format!("--config={}", config_file.path().display()),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    onchain.mint_block().await;

    let place_order = |salt: u8| {
        let services = &services;
        let onchain = &onchain;
        let trader = &trader;

        let request = OrderQuoteRequest {
            from: trader.address(),
            sell_token: *token.address(),
            buy_token: *onchain.contracts().weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: NonZeroU256::try_from(1u64.eth()).unwrap(),
                },
            },
            app_data: OrderCreationAppData::Full {
                full: json!({"salt": salt}).to_string(),
            },
            ..Default::default()
        };
        async move {
            let quote = services.submit_quote(&request).await.unwrap().quote;

            let order = OrderCreation {
                kind: quote.kind,
                sell_token: quote.sell_token,
                sell_amount: quote.sell_amount,
                fee_amount: U256::ZERO,
                buy_token: quote.buy_token,
                buy_amount: ((quote.buy_amount * U256::from(99)) / U256::from(100)),
                valid_to: quote.valid_to,
                app_data: quote.app_data,
                ..Default::default()
            }
            .sign(
                EcdsaSigningScheme::Eip712,
                &onchain.contracts().domain_separator,
                &trader.signer,
            );
            services.create_order(&order).await.unwrap()
        }
    };

    let cancel_order = |order_uid: OrderUid| {
        let client = services.client();
        let cancellation = OrderCancellation::for_order(
            order_uid,
            &onchain.contracts().domain_separator,
            &trader.signer,
        );

        async move {
            let cancellation = client
                .delete(format!("{API_HOST}{ORDERS_ENDPOINT}/{order_uid}"))
                .json(&CancellationPayload {
                    signature: cancellation.signature,
                    signing_scheme: cancellation.signing_scheme,
                })
                .send()
                .await
                .unwrap();

            assert_eq!(cancellation.status(), 200);
        }
    };

    let cancel_orders = |order_uids: Vec<OrderUid>| {
        let client = services.client();
        let cancellations = OrderCancellations { order_uids };
        let signing_scheme = EcdsaSigningScheme::Eip712;
        let signature = EcdsaSignature::sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &cancellations.hash_struct(),
            &trader.signer,
        );

        let signed_cancellations = SignedOrderCancellations {
            data: cancellations,
            signature,
            signing_scheme,
        };

        async move {
            let cancellation = client
                .delete(format!("{API_HOST}{ORDERS_ENDPOINT}"))
                .json(&signed_cancellations)
                .send()
                .await
                .unwrap();

            assert_eq!(cancellation.status(), 200);
        }
    };

    // Place 3 orders.
    let order_uids = vec![
        place_order(0).await,
        place_order(1).await,
        place_order(2).await,
    ];
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        services.get_auction().await.auction.orders.len() == 3
    })
    .await
    .unwrap();
    for order_uid in &order_uids {
        assert_eq!(
            services.get_order(order_uid).await.unwrap().metadata.status,
            OrderStatus::Open,
        );
    }
    for uid in &order_uids {
        let events = crate::database::events_of_order(services.db(), uid).await;
        assert_eq!(events.first().unwrap().label, OrderEventLabel::Created);
    }
    for uid in &order_uids {
        let order_status = services.get_order_status(uid).await.unwrap();
        assert!(matches!(
            order_status,
            orderbook::dto::order::Status::Active
        ));
    }

    // Cancel one of them.
    cancel_order(order_uids[0]).await;
    wait_for_condition(TIMEOUT, || async {
        // continue minting another block to make sure the autopilot eventually
        // refreshes its cache
        onchain.mint_block().await;
        services.get_auction().await.auction.orders.len() == 2
    })
    .await
    .unwrap();
    assert_eq!(
        services
            .get_order(&order_uids[0])
            .await
            .unwrap()
            .metadata
            .status,
        OrderStatus::Cancelled,
    );
    let events = crate::database::events_of_order(services.db(), &order_uids[0]).await;
    assert!(
        events
            .iter()
            .any(|event| event.label == OrderEventLabel::Cancelled)
    );

    // Cancel the other two.
    cancel_orders(vec![order_uids[1], order_uids[2]]).await;
    wait_for_condition(TIMEOUT, || async {
        // continue minting another block to make sure the autopilot eventually
        // refreshes its cache
        onchain.mint_block().await;
        services.get_auction().await.auction.orders.is_empty()
    })
    .await
    .unwrap();
    assert_eq!(
        services
            .get_order(&order_uids[1])
            .await
            .unwrap()
            .metadata
            .status,
        OrderStatus::Cancelled,
    );
    assert_eq!(
        services
            .get_order(&order_uids[2])
            .await
            .unwrap()
            .metadata
            .status,
        OrderStatus::Cancelled,
    );

    for uid in &order_uids {
        let events = crate::database::events_of_order(services.db(), uid).await;
        assert!(
            events
                .iter()
                .any(|event| event.label == OrderEventLabel::Cancelled)
        );
    }
}
