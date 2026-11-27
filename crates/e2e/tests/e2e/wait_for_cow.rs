use {
    ::alloy::primitives::{Address, U256},
    autopilot::infra::persistence::dto::order::FeePolicy,
    configs::{
        autopilot::{Configuration as AutopilotConfiguration, run_loop::RunLoopConfig},
        canonical_batch::CanonicalBatchInterval,
        orderbook::{
            Configuration as OrderbookConfiguration,
            order_validation::OrderValidationConfig,
        },
        test_util::TestDefault,
    },
    database::fee_policies::{FeePolicyKind, FeePolicyOrigin},
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        DomainSeparator,
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus, OrderUid},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
        time::now_in_epoch_seconds,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    orderbook::dto::order::Status,
    reqwest::StatusCode,
    std::time::Duration,
};

/// Seconds between two canonical batches in this test. Long enough to place
/// the orders and see a regular order trade before the batch.
const INTERVAL: u32 = 20;

const WAIT_FOR_COW: &str = r#"{"metadata":{"enableWaitForCow":true}}"#;

/// Signs an order selling 2 units of `sell_token`.
fn order(
    domain_separator: &DomainSeparator,
    trader: &TestAccount,
    sell_token: Address,
    buy_token: Address,
    buy_amount: U256,
    valid_to: u32,
    app_data: &str,
) -> OrderCreation {
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
    .sign(EcdsaSigningScheme::Eip712, domain_separator, &trader.signer)
}
/// The shape of the prod rules: every order pays a surplus or price
/// improvement fee, and all but wait-for-CoW orders a 1% volume fee on top.
const FEE_RULES: &str = r#"
[[rule]]
name = "Wait for CoW: no volume fee"
when = "order.waitForCow"
fee = "order.inMarket ? price_improvement(50, 98) : surplus(50, 98)"

[[rule]]
name = "Default"
when = "true"
fee = "[order.inMarket ? price_improvement(50, 98) : surplus(50, 98), volume(100)]"
"#;

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
                fee_engine: fee_engine_config(FEE_RULES),
                ..AutopilotConfiguration::test("test_solver", solver.address())
            },
            OrderbookConfiguration {
                order_validation: OrderValidationConfig {
                    canonical_batch_interval: Some(interval),
                    ..Default::default()
                },
                fee_engine: fee_engine_config(FEE_RULES),
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
    let min_validity = OrderValidationConfig::default()
        .min_order_validity_period
        .as_secs();
    assert_eq!(
        schedule["nextBatchValidTo"].as_u64().unwrap(),
        schedule["nextBatch"].as_u64().unwrap() + u64::from(INTERVAL) + min_validity
    );

    let domain_separator = &onchain.contracts().domain_separator;
    let (token_a, token_b) = (*token_a.address(), *token_b.address());

    tracing::info!("A wait-for-CoW quote has no volume fee.");
    let quoted_volume_fee = |app_data: &str| {
        let request = OrderQuoteRequest {
            sell_token: token_a,
            buy_token: token_b,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(2u64.eth()).unwrap(),
                },
            },
            app_data: OrderCreationAppData::Full {
                full: app_data.to_string(),
            },
            ..Default::default()
        };
        let services = &services;
        async move {
            services
                .submit_quote(&request)
                .await
                .unwrap()
                .protocol_fee_bps
        }
    };
    assert_eq!(quoted_volume_fee(WAIT_FOR_COW).await, None);
    assert_eq!(quoted_volume_fee("{}").await.as_deref(), Some("100"));

    tracing::info!("Waiting until right after a batch time.");
    // The rest of the test relies on starting early in an interval. Sleeping
    // whole seconds from inside the current second wakes up inside the second
    // right after a batch time.
    let now = now_in_epoch_seconds();
    let after_batch = interval.next_batch(now - 1) + 1;
    tokio::time::sleep(Duration::from_secs((after_batch - now).into())).await;
    let placed_at = now_in_epoch_seconds();
    let batch = placed_at - (placed_at % INTERVAL) + INTERVAL;

    tracing::info!("A wait-for-CoW order that would expire before its batch is rejected.");
    // Valid for longer than the minimum validity (60s) from now, but not from
    // the next batch time, which is almost a whole interval away.
    let (status, body) = services
        .create_order(&order(
            domain_separator,
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
            domain_separator,
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
            domain_separator,
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
            domain_separator,
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
            domain_separator,
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
    // The batch is up to an interval away.
    let until_after_batch = Duration::from_secs(INTERVAL.into()) + TIMEOUT;
    wait_for_condition(until_after_batch, || async {
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

    tracing::info!("Only the regular order paid a protocol volume fee.");
    wait_for_condition(TIMEOUT, || async {
        for uid in [regular, cow_a, cow_b] {
            if stored_protocol_fees(&services, &uid).await.is_empty() {
                return false;
            }
        }
        true
    })
    .await
    .unwrap();
    let regular_fees = stored_protocol_fees(&services, &regular).await;
    assert_eq!(regular_fees.len(), 2, "{regular_fees:?}");
    assert_eq!(regular_fees[1], (FeePolicyKind::Volume, Some(0.01)));
    for uid in [cow_a, cow_b] {
        let fees = stored_protocol_fees(&services, &uid).await;
        assert_eq!(fees.len(), 1, "{fees:?}");
        assert_ne!(fees[0].0, FeePolicyKind::Volume);
    }

    tracing::info!(
        "The order that wasn't filled in its batch is a regular order afterwards, still without a \
         volume fee."
    );
    let next_batch = batch + INTERVAL;
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        assert!(
            now_in_epoch_seconds() < next_batch,
            "the unfilled order didn't enter an auction before the next batch"
        );
        let auction = services.get_auction().await;
        if auction.id <= competition.auction_id {
            return false;
        }
        let Some(order) = auction
            .auction
            .orders
            .iter()
            .find(|order| order.uid == unfillable)
        else {
            return false;
        };
        let fees = &order.protocol_fees;
        assert_eq!(fees.len(), 1, "{fees:?}");
        assert!(!matches!(fees[0], FeePolicy::Volume { .. }), "{fees:?}");
        true
    })
    .await
    .unwrap();
    assert_ne!(
        services.get_order_status(&unfillable).await.unwrap(),
        Status::Scheduled
    );
}

/// The kind and volume factor of each protocol fee stored for the order in the
/// latest auction where a solution included it.
async fn stored_protocol_fees(
    services: &Services<'_>,
    uid: &OrderUid,
) -> Vec<(FeePolicyKind, Option<f64>)> {
    crate::database::latest_fee_policies_of_order(services.db(), uid)
        .await
        .into_iter()
        .filter(|policy| policy.origin == Some(FeePolicyOrigin::Protocol))
        .map(|policy| (policy.kind, policy.volume_factor))
        .collect()
}
