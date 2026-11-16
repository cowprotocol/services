use {
    ::alloy::primitives::{Address, U256},
    configs::{
        autopilot::{Configuration as AutopilotConfiguration, run_loop::RunLoopConfig},
        canonical_batch::CanonicalBatchInterval,
        orderbook::{
            Configuration as OrderbookConfiguration,
            order_validation::OrderValidationConfig,
        },
        test_util::TestDefault,
    },
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus, OrderUid},
        signature::EcdsaSigningScheme,
        time::now_in_epoch_seconds,
    },
    number::units::EthUnit,
    orderbook::dto::order::Status,
    reqwest::StatusCode,
    std::time::Duration,
};

/// Seconds between two canonical batches in this test. Long enough to place
/// the orders and see a regular order trade before the batch.
const INTERVAL: u32 = 20;

const WAIT_FOR_COW: &str = r#"{"metadata":{"enableWaitForCow":true}}"#;

#[tokio::test]
#[ignore]
async fn local_node_wait_for_cow() {
    run_test(wait_for_cow_test).await;
}

/// Orders placed with `enableWaitForCow` stay out of auctions until the next
/// canonical batch. All orders waiting for a batch enter the same auction,
/// regular orders keep trading before it, and an order that isn't filled in
/// its batch trades like a regular order afterwards.
async fn wait_for_cow_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(10u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    for (trader, token) in [(&trader_a, &token_a), (&trader_b, &token_b)] {
        token.mint(trader.address(), 10u64.eth()).await;
        token
            .approve(onchain.contracts().allowance, 10u64.eth())
            .from(trader.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    let interval = CanonicalBatchInterval::try_from(Duration::from_secs(INTERVAL.into())).unwrap();
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            AutopilotConfiguration {
                run_loop: RunLoopConfig {
                    canonical_batch_interval: Some(interval),
                    ..RunLoopConfig::test_default()
                },
                ..AutopilotConfiguration::test("test_solver", solver.address())
            },
            OrderbookConfiguration {
                order_validation: OrderValidationConfig {
                    canonical_batch_interval: Some(interval),
                    ..Default::default()
                },
                ..OrderbookConfiguration::test_default()
            },
            solver,
        )
        .await;

    tracing::info!("The orderbook serves the canonical batch schedule.");
    let schedule = services.get_canonical_batch_schedule().await.unwrap();
    assert_eq!(schedule["intervalSeconds"], INTERVAL);
    assert_eq!(
        schedule["nextBatch"].as_u64().unwrap() % u64::from(INTERVAL),
        0
    );
    assert!(schedule["secondsUntilNextBatch"].as_u64().unwrap() < u64::from(INTERVAL));

    let order = |trader: &TestAccount,
                 sell_token: Address,
                 buy_token: Address,
                 buy_amount: U256,
                 valid_to: u32,
                 app_data: &str| {
        OrderCreation {
            sell_token,
            sell_amount: 2u64.eth(),
            buy_token,
            buy_amount,
            valid_to,
            kind: OrderKind::Sell,
            app_data: OrderCreationAppData::Full {
                full: app_data.to_string(),
            },
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        )
    };
    let (token_a, token_b) = (*token_a.address(), *token_b.address());

    tracing::info!("Waiting until right after a batch time.");
    while now_in_epoch_seconds() % INTERVAL != 1 {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let placed_at = now_in_epoch_seconds();
    let batch = placed_at - placed_at % INTERVAL + INTERVAL;

    tracing::info!("A wait-for-CoW order that would expire before its batch is rejected.");
    // Valid for longer than the minimum validity (60s) from now, but not from
    // the next batch time, which is almost a whole interval away.
    let (status, body) = services
        .create_order(&order(
            &trader_a,
            token_a,
            token_b,
            U256::ONE,
            placed_at + 70,
            WAIT_FOR_COW,
        ))
        .await
        .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body.contains("InsufficientValidToForCanonicalBatch"),
        "{body}"
    );

    tracing::info!("Placing wait-for-CoW orders and a regular order.");
    let valid_to = placed_at + 600;
    let cow_a = services
        .create_order(&order(
            &trader_a,
            token_a,
            token_b,
            1u64.eth(),
            valid_to,
            WAIT_FOR_COW,
        ))
        .await
        .unwrap();
    let cow_b = services
        .create_order(&order(
            &trader_b,
            token_b,
            token_a,
            1u64.eth(),
            valid_to,
            WAIT_FOR_COW,
        ))
        .await
        .unwrap();
    // Asks for far more than the pools can give.
    let unfillable = services
        .create_order(&order(
            &trader_a,
            token_a,
            token_b,
            100u64.eth(),
            valid_to,
            WAIT_FOR_COW,
        ))
        .await
        .unwrap();
    let regular = services
        .create_order(&order(
            &trader_b,
            token_b,
            token_a,
            1u64.eth(),
            valid_to,
            "{}",
        ))
        .await
        .unwrap();
    let waiting = [cow_a, cow_b, unfillable];

    tracing::info!("The autopilot schedules the wait-for-CoW orders for the next batch.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        for uid in waiting {
            if services
                .get_order(&uid)
                .await
                .unwrap()
                .metadata
                .valid_from
                .is_none()
            {
                return false;
            }
        }
        true
    })
    .await
    .unwrap();
    for uid in waiting {
        assert_eq!(
            services.get_order(&uid).await.unwrap().metadata.valid_from,
            Some(batch)
        );
    }

    let is_traded = |uid: OrderUid| {
        let services = &services;
        async move { services.get_order(&uid).await.unwrap().metadata.status == OrderStatus::Fulfilled }
    };

    tracing::info!("Before the batch, only the regular order trades.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        is_traded(regular).await
    })
    .await
    .unwrap();
    assert!(
        now_in_epoch_seconds() < batch,
        "the regular order took too long to trade"
    );
    let auction = services.get_auction().await;
    for uid in waiting {
        assert!(!is_traded(uid).await);
        assert_eq!(
            services.get_order_status(&uid).await.unwrap(),
            Status::Scheduled
        );
        assert!(!auction.auction.orders.iter().any(|order| order.uid == uid));
    }

    tracing::info!("At the batch, the wait-for-CoW orders that can trade do.");
    wait_for_condition(Duration::from_secs(INTERVAL.into()) + TIMEOUT, || async {
        onchain.mint_block().await;
        if now_in_epoch_seconds() < batch {
            assert!(
                !is_traded(cow_a).await && !is_traded(cow_b).await,
                "a wait-for-CoW order traded before its batch"
            );
            return false;
        }
        is_traded(cow_a).await && is_traded(cow_b).await
    })
    .await
    .unwrap();

    tracing::info!("The wait-for-CoW orders were in the same auction.");
    let first_trade = services
        .get_trades(&cow_a)
        .await
        .unwrap()
        .into_iter()
        .chain(services.get_trades(&cow_b).await.unwrap())
        .min_by_key(|trade| trade.block_number)
        .unwrap();
    let tx_hash = first_trade.tx_hash.unwrap();
    wait_for_condition(TIMEOUT, || async {
        services.get_solver_competition(tx_hash).await.is_ok()
    })
    .await
    .unwrap();
    let competition = services.get_solver_competition(tx_hash).await.unwrap();
    for uid in waiting {
        assert!(
            competition.auction.orders.contains(&uid),
            "{uid} is missing from the auction"
        );
    }

    tracing::info!("The order that wasn't filled in its batch is a regular order afterwards.");
    let next_batch = batch + INTERVAL;
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        assert!(
            now_in_epoch_seconds() < next_batch,
            "the unfilled order didn't enter an auction before the next batch"
        );
        let auction = services.get_auction().await;
        auction.id > competition.auction_id
            && auction
                .auction
                .orders
                .iter()
                .any(|order| order.uid == unfillable)
    })
    .await
    .unwrap();
    assert_ne!(
        services.get_order_status(&unfillable).await.unwrap(),
        Status::Scheduled
    );
}
