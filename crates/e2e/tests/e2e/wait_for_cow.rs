use {
    ::alloy::primitives::U256,
    configs::{
        autopilot::Configuration as AutopilotConfiguration,
        orderbook::{
            Configuration as OrderbookConfiguration,
            order_validation::OrderValidationConfig,
        },
        test_util::TestDefault,
    },
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    reqwest::StatusCode,
    std::time::Duration,
};

/// Batch-auction interval used by both services in this test. Short enough
/// to keep the test fast, long enough to observe the order being held out
/// of the auction across several autopilot cycles.
const INTERVAL: Duration = Duration::from_secs(20);

#[tokio::test]
#[ignore]
async fn local_node_wait_for_cow() {
    run_test(wait_for_cow_test).await;
}

/// `waitForCow` (app-data) orders are inserted without a `valid_from`. The
/// autopilot's solvable-orders cache assigns one at the next multiple of
/// `batch_auction_interval` and the order only settles after that boundary.
/// The orderbook rejects `waitForCow` orders whose `validTo` cannot outlive
/// the latest possible assignment, and app-data combining the flag with
/// `enableFastPath` or `validFrom`.
async fn wait_for_cow_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 10u64.eth()).await;
    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            AutopilotConfiguration {
                batch_auction_interval: INTERVAL,
                ..AutopilotConfiguration::test("test_solver", solver.address())
            },
            OrderbookConfiguration {
                order_validation: OrderValidationConfig {
                    batch_auction_interval: INTERVAL,
                    ..Default::default()
                },
                ..OrderbookConfiguration::test_default()
            },
            solver,
        )
        .await;

    let order = |app_data: &str, valid_to: u32| {
        OrderCreation {
            sell_token: *token_a.address(),
            sell_amount: 1u64.eth(),
            buy_token: *token_b.address(),
            buy_amount: U256::ONE,
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
    let now = model::time::now_in_epoch_seconds();

    tracing::info!("waitForCow is mutually exclusive with enableFastPath and validFrom.");
    let with_fast_path = r#"{"metadata":{"waitForCow":true,"enableFastPath":true}}"#.to_string();
    let with_valid_from = format!(
        r#"{{"metadata":{{"waitForCow":true,"validFrom":{}}}}}"#,
        now + 100
    );
    for app_data in [with_fast_path, with_valid_from] {
        let (status, body) = services
            .create_order(&order(&app_data, now + 3600))
            .await
            .unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(body.contains("InvalidAppData"), "{body}");
        assert!(body.contains("mutually exclusive"), "{body}");
    }

    tracing::info!("A validTo that cannot outlive the latest assignment is rejected.");
    // Passes the 60s minimum validity but not `now + interval + minimum`.
    let (status, body) = services
        .create_order(&order(r#"{"metadata":{"waitForCow":true}}"#, now + 70))
        .await
        .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("InvalidValidFrom"), "{body}");

    tracing::info!("The next-batch endpoint predicts the upcoming boundary.");
    let interval = u32::try_from(INTERVAL.as_secs()).unwrap();
    let (predicted, reported_interval) = services.get_wait_for_cow_next_batch().await;
    assert_eq!(reported_interval, INTERVAL.as_secs());
    assert_eq!(predicted % interval, 0, "prediction must sit on a boundary");
    let queried_at = model::time::now_in_epoch_seconds();
    assert!(
        queried_at <= predicted && predicted <= queried_at + interval,
        "prediction {predicted} is not the next boundary after {queried_at}",
    );

    tracing::info!("Placing a waitForCow order.");
    let placed_at = model::time::now_in_epoch_seconds();
    let uid = services
        .create_order(&order(r#"{"metadata":{"waitForCow":true}}"#, now + 3600))
        .await
        .unwrap();

    tracing::info!("Waiting for the autopilot to assign valid_from.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.valid_from.is_some())
    })
    .await
    .unwrap();
    let assigned_at = model::time::now_in_epoch_seconds();
    let valid_from = services
        .get_order(&uid)
        .await
        .unwrap()
        .metadata
        .valid_from
        .unwrap();

    assert_eq!(
        valid_from % interval,
        0,
        "valid_from must sit on a boundary"
    );
    assert!(
        valid_from >= placed_at,
        "valid_from {valid_from} must not predate placement at {placed_at}",
    );
    assert!(
        valid_from <= assigned_at + interval,
        "valid_from {valid_from} is more than one interval past assignment at {assigned_at}",
    );
    // The prediction holds unless the boundary passed between the query and
    // the autopilot's refresh, in which case the next one is assigned.
    assert!(
        valid_from == predicted || valid_from == predicted + interval,
        "valid_from {valid_from} does not match the predicted boundary {predicted}",
    );

    tracing::info!("The order settles only once the boundary has passed.");
    // The boundary can be up to a full interval away; allow a cycle or two
    // on top for the crossing to be picked up and settled.
    wait_for_condition(INTERVAL + TIMEOUT, || async {
        onchain.mint_block().await;
        let Ok(order) = services.get_order(&uid).await else {
            return false;
        };
        if model::time::now_in_epoch_seconds() < valid_from {
            assert_eq!(
                order.metadata.status,
                OrderStatus::Open,
                "waitForCow order settled before its batch-auction boundary",
            );
            false
        } else {
            order.metadata.status == OrderStatus::Fulfilled
        }
    })
    .await
    .unwrap();
}
