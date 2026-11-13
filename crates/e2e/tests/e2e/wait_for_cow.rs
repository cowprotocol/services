use {
    crate::database::events_of_order,
    ::alloy::primitives::{Address, U256},
    configs::{
        autopilot::{Configuration as AutopilotConfiguration, run_loop::RunLoopConfig},
        orderbook::Configuration as OrderbookConfiguration,
        test_util::TestDefault,
    },
    database::order_events::{OrderEventLabel, OrderFilterReason},
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
/// the orders and see a regular order trade between two batches.
const INTERVAL: u32 = 20;
/// Orders created less than this many seconds before a batch time join the
/// batch after it.
const CUTOFF: u32 = 2;
/// How long after its batch time a batch auction may start in this test.
const BATCH_START_SLACK: u32 = 5;

const WAIT_FOR_COW: &str = r#"{"metadata":{"enableWaitForCow":true}}"#;

#[tokio::test]
#[ignore]
async fn local_node_wait_for_cow() {
    run_test(wait_for_cow_test).await;
}

/// Orders placed with `enableWaitForCow` only trade in canonical batches. All
/// orders waiting for a batch enter the same auction, regular orders keep
/// trading in between, and an order that isn't filled in its batch waits for
/// the next one.
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

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            AutopilotConfiguration {
                run_loop: RunLoopConfig {
                    canonical_batch_interval: Some(Duration::from_secs(INTERVAL.into())),
                    canonical_batch_cutoff: Duration::from_secs(CUTOFF.into()),
                    ..RunLoopConfig::test_default()
                },
                ..AutopilotConfiguration::test("test_solver", solver.address())
            },
            OrderbookConfiguration::test_default(),
            solver,
        )
        .await;

    tracing::info!("The orderbook serves the schedule the autopilot published.");
    wait_for_condition(TIMEOUT, || async {
        services.get_canonical_batch_schedule().await.is_ok()
    })
    .await
    .unwrap();
    let schedule = services.get_canonical_batch_schedule().await.unwrap();
    assert_eq!(schedule["intervalSeconds"], INTERVAL);
    assert_eq!(schedule["cutoffSeconds"], CUTOFF);

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

    tracing::info!("A wait-for-CoW order that would expire before its first batch is rejected.");
    // Long enough for the minimum validity (60s), but not for the first batch
    // (more than `CUTOFF` away) plus an interval plus the minimum validity.
    let now = now_in_epoch_seconds();
    let (status, body) = services
        .create_order(&order(
            &trader_a,
            token_a,
            token_b,
            U256::ONE,
            now + 70,
            WAIT_FOR_COW,
        ))
        .await
        .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body.contains("InsufficientValidToForCanonicalBatch"),
        "{body}"
    );

    tracing::info!("Placing orders right after a batch time.");
    while now_in_epoch_seconds() % INTERVAL != 1 {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let placed_at = now_in_epoch_seconds();
    let first_batch = placed_at - placed_at % INTERVAL + INTERVAL;
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
    assert!(
        now_in_epoch_seconds() < first_batch - CUTOFF,
        "placing the orders took too long"
    );

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
        now_in_epoch_seconds() < first_batch,
        "the regular order took too long to trade"
    );
    let auction = services.get_auction().await;
    for uid in [cow_a, cow_b, unfillable] {
        assert!(!is_traded(uid).await);
        assert_eq!(
            services.get_order_status(&uid).await.unwrap(),
            Status::Scheduled
        );
        assert!(!auction.auction.orders.iter().any(|order| order.uid == uid));
    }

    tracing::info!("At the batch, the wait-for-CoW orders that can trade do.");
    wait_for_condition(
        Duration::from_secs((2 * INTERVAL).into()) + TIMEOUT,
        || async {
            onchain.mint_block().await;
            if now_in_epoch_seconds() < first_batch {
                assert!(
                    !is_traded(cow_a).await && !is_traded(cow_b).await,
                    "a wait-for-CoW order traded before its batch"
                );
                return false;
            }
            is_traded(cow_a).await && is_traded(cow_b).await
        },
    )
    .await
    .unwrap();

    tracing::info!("All wait-for-CoW orders were in the same batch auction.");
    let mut trades = Vec::new();
    for uid in [cow_a, cow_b] {
        trades.extend(services.get_trades(&uid).await.unwrap());
    }
    let first_trade = trades
        .iter()
        .min_by_key(|trade| trade.block_number)
        .unwrap();
    let tx_hash = first_trade.tx_hash.unwrap();
    wait_for_condition(TIMEOUT, || async {
        services.get_solver_competition(tx_hash).await.is_ok()
    })
    .await
    .unwrap();
    let competition = services.get_solver_competition(tx_hash).await.unwrap();
    for uid in [cow_a, cow_b, unfillable] {
        assert!(
            competition.auction.orders.contains(&uid),
            "{uid} is missing from the batch auction"
        );
    }

    tracing::info!("The order that couldn't trade waits for the next batch.");
    let second_batch = first_batch + INTERVAL;
    wait_for_condition(Duration::from_secs(INTERVAL.into()) + TIMEOUT, || async {
        onchain.mint_block().await;
        let now = now_in_epoch_seconds();
        if (first_batch + BATCH_START_SLACK..second_batch).contains(&now) {
            assert_eq!(
                services.get_order_status(&unfillable).await.unwrap(),
                Status::Scheduled
            );
        }
        events_of_order(services.db(), &unfillable)
            .await
            .iter()
            .any(|event| {
                event.label == OrderEventLabel::Ready
                    && event.timestamp.timestamp() >= i64::from(second_batch)
            })
    })
    .await
    .unwrap();

    // It was only ever in an auction right after one of its batch times, and
    // was waiting in between.
    let events = events_of_order(services.db(), &unfillable).await;
    let since = |batch: u32| i64::from(batch)..i64::from(batch + BATCH_START_SLACK);
    let ready: Vec<_> = events
        .iter()
        .filter(|event| event.label == OrderEventLabel::Ready)
        .map(|event| event.timestamp.timestamp())
        .collect();
    assert!(
        ready
            .iter()
            .all(|time| since(first_batch).contains(time) || since(second_batch).contains(time)),
        "in an auction outside its batches: {events:?}"
    );
    assert!(
        ready.iter().any(|time| since(first_batch).contains(time)),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.label == OrderEventLabel::Filtered
                && event.reason == Some(OrderFilterReason::WaitingForCanonicalBatch)
                && (i64::from(first_batch)..i64::from(second_batch))
                    .contains(&event.timestamp.timestamp())
        }),
        "{events:?}"
    );
}
