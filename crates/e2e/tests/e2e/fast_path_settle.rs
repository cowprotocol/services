use {
    crate::ethflow::ExtendedEthFlowOrder,
    ::alloy::primitives::{Address, U256},
    app_data::AppDataHash,
    configs::{
        autopilot::{
            Configuration as AutopilotConfiguration,
            fee_policy::{
                FeePoliciesConfig,
                FeePolicy as ConfigFeePolicy,
                FeePolicyKind as ConfigFeePolicyKind,
                FeePolicyOrderClass as ConfigFeePolicyOrderClass,
            },
            penalty_cap::PenaltyCapConfig,
            solver::Solver,
        },
        order_quoting::{ExternalSolver, OrderQuoting},
        test_util::TestDefault,
    },
    e2e::{assert_approximately_eq, setup::*},
    ethrpc::alloy::CallBuilderExt,
    model::{
        fee_policy::FeePolicy as TradeFeePolicy,
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        quote::{
            OrderQuoteRequest,
            OrderQuoteSide,
            PriceQuality,
            QuoteSigningScheme,
            SellAmount,
            Validity,
        },
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, u256_ext::U256Ext, units::EthUnit},
    serde_json::json,
    shared::web3::Web3,
    std::time::Duration,
};

/// Enables the fast path on the autopilot and sets
/// `fast_path_submission_deadline` to the block count that produces the
/// requested wall-clock window on the local Hardhat chain (12s blocks), so a
/// test can dial in the exclusivity it needs.
///
/// `max_partner_fee` is still mirrored onto the orderbook because its
/// placement-time limit-price check needs to size partner fees the same
/// way the autopilot would charge them at settle time.
fn with_fast_path_exclusivity(
    autopilot: AutopilotConfiguration,
    orderbook: configs::orderbook::Configuration,
    exclusivity: Duration,
) -> (AutopilotConfiguration, configs::orderbook::Configuration) {
    let max_partner_fee = autopilot
        .order_quoting
        .max_partner_fee
        .or(orderbook.order_quoting.max_partner_fee)
        .or(Some(autopilot.fee_policies.max_partner_fee));
    // Local anvil advertises the Hardhat chain-id (12s blocks).
    const HARDHAT_BLOCK_SECS: u64 = 12;
    let fast_path_submission_deadline = exclusivity.as_secs().div_ceil(HARDHAT_BLOCK_SECS).max(1);
    let autopilot = AutopilotConfiguration {
        fast_path_submission_deadline: Some(fast_path_submission_deadline),
        order_quoting: OrderQuoting {
            max_partner_fee,
            ..autopilot.order_quoting
        },
        ..autopilot
    };
    let orderbook = configs::orderbook::Configuration {
        order_quoting: OrderQuoting {
            max_partner_fee,
            ..orderbook.order_quoting
        },
        ..orderbook
    };
    (autopilot, orderbook)
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_settle() {
    run_test(fast_path_settle).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_regular_auction_fallback() {
    run_test(fast_path_regular_auction_fallback).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_volume_fees_captured() {
    run_test(fast_path_volume_fees_captured).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_ethflow_settle() {
    run_test(fast_path_ethflow_settle).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_limit_too_tight_rejected() {
    run_test(fast_path_limit_too_tight_rejected).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_records_filtered_out_solutions() {
    run_test(fast_path_records_filtered_out_solutions).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_settles_across_split_configs() {
    run_test(fast_path_settles_across_split_configs).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_penalty_cap() {
    run_test(fast_path_penalty_cap).await;
}

/// A fast-path order signed against a quoting solver that charges a solver
/// fee, at exactly the quote and with no slippage of the user's own. The fee is
/// what made that quote conservative, so the settlement has to land on the
/// signed limit and the fast path itself has to be what settles it.
async fn fast_path_settle(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    // A long fast-path exclusivity so only the fast path can settle the order
    // within the test window.
    let exclusivity = Duration::from_secs(300);
    let (autopilot_config, orderbook_config) = with_fast_path_exclusivity(
        AutopilotConfiguration::test("test_solver", solver.address()),
        configs::orderbook::Configuration::test_default(),
        exclusivity,
    );
    // The solver charges a solver fee, which is what makes the quote it
    // publishes conservative. The user signs against that quote below without
    // adding any slippage of their own, so the settlement has no room to take
    // the fee a second time.
    services
        .start_protocol_with_args_and_solver_fee(autopilot_config, orderbook_config, solver, 100)
        .await;

    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();

    tracing::info!("Quoting with enableFastPath.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: quote.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    // The fast-path handler stages its competition as soon as the order lands.
    // Holding on to its auction id is what tells a fast-path settlement apart
    // from the regular auction, which the autopilot releases the order to as
    // soon as a fast-path settle fails.
    tracing::info!("Waiting for the fast-path competition.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_latest_solver_competition()
            .await
            .is_ok_and(|competition| {
                competition
                    .solutions
                    .iter()
                    .any(|solution| solution.orders.iter().any(|order| order.id == uid))
            })
    })
    .await
    .unwrap();
    let fast_path_auction = services
        .get_latest_solver_competition()
        .await
        .unwrap()
        .auction_id;

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // A settlement under any later auction means the fast-path attempt
    // reverted and the regular auction picked the order up instead.
    let trade = services.get_trades(&uid).await.unwrap().pop().unwrap();
    let tx_hash = trade.tx_hash.expect("settled trade has a transaction");
    let settled_in = services
        .get_solver_competition(tx_hash)
        .await
        .expect("settlement has a competition")
        .auction_id;
    assert_eq!(
        settled_in, fast_path_auction,
        "order settled under auction {settled_in} but the fast path staged {fast_path_auction}",
    );

    // And the fill respects what the user signed, solver fee included.
    let filled_buy: U256 = trade.buy_amount.to_string().parse().unwrap();
    assert!(
        filled_buy >= quote.quote.buy_amount,
        "filled at {filled_buy} but the user signed for {}",
        quote.quote.buy_amount,
    );
}

/// A fast-path order settled out of competition gets its CIP-87 penalty cap
/// persisted on the promoted `competition_auctions` row, like a regular
/// auction.
async fn fast_path_penalty_cap(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    let exclusivity = Duration::from_secs(300);
    // Enable penalty caps alongside the fast path. `with_fast_path_exclusivity`
    // preserves this via its `..autopilot` spread.
    let base = AutopilotConfiguration {
        penalty_cap: Some(PenaltyCapConfig {
            default_factor: 0.0004.try_into().unwrap(),
            absolute_cap_usd: 20.,
            // WETH as the USD reference: its native price is 1 by definition, so
            // the bound is 20 ETH. The test only cares about the plumbing.
            usd_reference_token: *onchain.contracts().weth.address(),
            overrides: vec![],
        }),
        // Opt the fast path into penalties (off by default).
        fast_path_penalty_cap_enabled: true,
        ..AutopilotConfiguration::test("test_solver", solver.address())
    };
    let (autopilot_config, orderbook_config) = with_fast_path_exclusivity(
        base,
        configs::orderbook::Configuration::test_default(),
        exclusivity,
    );
    services
        .start_protocol_with_args(autopilot_config, orderbook_config, solver)
        .await;

    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();

    tracing::info!("Quoting with enableFastPath.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: quote.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let placed_at = std::time::Instant::now();
    let uid = services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // Fulfilled well within the exclusivity window ⇒ the fast-path handler
    // settled it (and therefore wrote the promoted competition row), not the
    // regular-auction fallback.
    let elapsed = placed_at.elapsed();
    assert!(
        elapsed < exclusivity / 2,
        "settled after {elapsed:?}; regular fallback would have waited out {exclusivity:?}",
    );

    let caps = crate::database::penalty_caps_of_order(services.db(), &uid).await;
    assert!(
        caps.iter().any(bigdecimal::Signed::is_positive),
        "fast-path competition row should carry a positive penalty cap, got {caps:?}",
    );

    // The settled trade exposes that same cap for accounting.
    let trade = services.get_trades(&uid).await.unwrap().remove(0);
    let cap = trade
        .penalty_cap_native
        .expect("settled fast-path trade carries the auction's penalty cap");
    assert!(!cap.is_zero());
}

/// Regression test for the quoter/solver fast-path cache split.
///
/// A quoter config (`test_quote`) and a solve config (`test_solver`) share one
/// submission address on a single driver. A fast-path quote is served by the
/// quoter, but the autopilot settles against the solve config (matched by
/// submission address). The driver keeps fast-path solutions in one store
/// shared across configs, so `test_solver` finds the quote `test_quote` cached
/// and the order settles fast. If the cache were per config (the bug), the
/// settle would miss and the driver would return `SolutionNotAvailable`.
async fn fast_path_settles_across_split_configs(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    // Long exclusivity so only the fast path can settle within the test window.
    let exclusivity = Duration::from_secs(300);

    // Two configs on one driver, both using `solver`'s account: the quoter
    // (`test_quote`) and the solve config (`test_solver`). They share a
    // submission address, so the autopilot settles against `test_solver`
    // no matter which config quoted.
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver_with_solver_fee(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                0,
            )
            .await,
            colocation::start_baseline_solver_with_solver_fee(
                "test_quote".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                0,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    // The quote runs on `test_quote`; the driver's shared fast-path cache lets
    // `test_solver` settle it.
    let quoter = ExternalSolver::new("test_quote", "http://localhost:11088/test_quote");

    let autopilot_config = AutopilotConfiguration {
        // Only the solve config settles. The autopilot matches by submission
        // address, which the two configs share.
        drivers: vec![Solver::test("test_solver", solver.address())],
        order_quoting: OrderQuoting::test_with_drivers(vec![quoter.clone()]),
        ..AutopilotConfiguration::test_no_drivers()
    };
    let orderbook_config = configs::orderbook::Configuration {
        order_quoting: OrderQuoting::test_with_drivers(vec![quoter]),
        ..configs::orderbook::Configuration::test_default()
    };
    let (autopilot_config, orderbook_config) =
        with_fast_path_exclusivity(autopilot_config, orderbook_config, exclusivity);

    services.start_autopilot(None, autopilot_config).await;
    services.start_api(orderbook_config).await;

    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();

    tracing::info!("Quoting with enableFastPath.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: quote.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let placed_at = std::time::Instant::now();
    let uid = services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    let elapsed = placed_at.elapsed();
    assert!(
        elapsed < exclusivity / 2,
        "settled after {elapsed:?}, but the regular-auction fallback needs at least \
         {exclusivity:?}, so the fast path must have settled it",
    );
}

/// Tests fast-path → regular-auction fallback with two solvers competing
/// for the same order.
///
/// * `solver_a` is unfunded and runs with `solver_fee_bps = 0`, so it wins the
///   quote (best price) but cannot pay for a settlement tx.
/// * `solver_b` is funded and runs with `solver_fee_bps = 100`, so it loses the
///   quote but can actually submit.
///
/// The fast-path handler routes to `solver_a` (the quote winner) and its tx
/// submission fails. When the regular auction runs after `valid_from`, the
/// driver's `Settlement::new` balance check (`crates/driver/src/domain/
/// competition/solution/settlement.rs`) filters `solver_a`'s bid out, so
/// `solver_b` wins the fallback auction without any mid-test funding.
///
/// Asserted:
/// 1. The autopilot ran the fast-path handler: a solver competition with a
///    solution that includes our order surfaces on the public
///    `solver_competition/latest` endpoint while the order is still `Open`
///    (regular auctions are barred from picking the order up until
///    `valid_from`, so this competition can only be the fast-path one).
/// 2. The order eventually becomes `Fulfilled` after `valid_from`.
/// 3. The settled competition's winning solver is `solver_b`, proving the
///    fallback actually routed around the broken solver.
async fn fast_path_regular_auction_fallback(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    // Two solvers behind a single driver process. `solver_a` is left at 0
    // ETH; `solver_b` gets funded so it can actually submit settlements.
    let [solver_a, solver_b] = onchain.make_solvers(0u64.eth()).await;
    onchain.send_wei(solver_b.address(), 10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    // Short exclusivity so the regular auction picks the order up soon
    // after it elapses, within the test timeout.
    let exclusivity = Duration::from_secs(5);

    // Both baseline solvers share the same UniV2 pool; `solver_fee_bps` is
    // what makes `solver_a` strictly beat `solver_b` during quoting.
    // `solver_a` is named `test_solver` so the default native-price
    // estimator wiring (`http://localhost:11088/test_solver`) works
    // without an override.
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver_with_solver_fee(
                "test_solver".into(),
                solver_a.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                0,
            )
            .await,
            colocation::start_baseline_solver_with_solver_fee(
                "solver_b".into(),
                solver_b.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                100,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    let quoter_a = ExternalSolver::new("test_solver", "http://localhost:11088/test_solver");
    let quoter_b = ExternalSolver::new("solver_b", "http://localhost:11088/solver_b");

    let autopilot_config = AutopilotConfiguration {
        drivers: vec![
            Solver::test("test_solver", solver_a.address()),
            Solver::test("solver_b", solver_b.address()),
        ],
        order_quoting: OrderQuoting::test_with_drivers(vec![quoter_a.clone(), quoter_b.clone()]),
        ..AutopilotConfiguration::test_no_drivers()
    };
    let orderbook_config = configs::orderbook::Configuration {
        order_quoting: OrderQuoting::test_with_drivers(vec![quoter_a, quoter_b]),
        ..configs::orderbook::Configuration::test_default()
    };
    let (autopilot_config, orderbook_config) =
        with_fast_path_exclusivity(autopilot_config, orderbook_config, exclusivity);

    services.start_autopilot(None, autopilot_config).await;
    services.start_api(orderbook_config).await;

    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();

    tracing::info!("Quoting with enableFastPath.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    // Sign at ~90% of the market quote so that `solver_b`'s
    // fee-tightened solve request (buy × ~1.01) still fits under the
    // pool's actual output. Otherwise the tightened requirement would sit
    // above market and `solver_b`'s baseline would return no solutions.
    // `solver_a` still wins the quote (no solver fee), and the fast-path
    // limit check compares the cached solver_a clearing prices — which are
    // at market rate — against `limit_prices.buy` (also market rate), so
    // that check still passes.
    let signed_buy_amount = quote.quote.buy_amount * U256::from(90u8) / U256::from(100u8);
    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: signed_buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();
    let valid_from = model::time::now_in_epoch_seconds() + exclusivity.as_secs() as u32;

    // (1) The fast-path handler promoted the order into a solver competition
    // *with a solution that includes our order* before any on-chain settlement
    // — direct evidence the fast path fired and picked up a bid. The regular
    // auction is barred from picking the order up until `valid_from`, so this
    // competition can only be the fast-path one.
    tracing::info!("Waiting for the fast-path competition to appear.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_latest_solver_competition()
            .await
            .is_ok_and(|competition| {
                competition
                    .solutions
                    .iter()
                    .any(|solution| solution.orders.iter().any(|order| order.id == uid))
            })
    })
    .await
    .unwrap();
    assert_eq!(
        services.get_order(&uid).await.unwrap().metadata.status,
        OrderStatus::Open,
        "order settled during the exclusivity window; fast path was expected to fail to submit"
    );

    // Both bids clear the signed limit price so we can assert things about the
    // solution scores and reference scores.
    let fast_path_competition = services.get_latest_solver_competition().await.unwrap();
    assert!(
        fast_path_competition
            .solutions
            .iter()
            .any(|s| s.orders.iter().any(|o| o.id == uid)),
        "latest competition should still be the fast-path one before valid_from",
    );
    let winner_sol = fast_path_competition
        .solutions
        .iter()
        .find(|s| s.is_winner)
        .expect("fast-path competition should have a winner");
    let runner_up = fast_path_competition
        .solutions
        .iter()
        .find(|s| !s.is_winner && !s.filtered_out)
        .expect("fast-path competition should have a non-filtered runner-up");
    assert!(
        !winner_sol.filtered_out,
        "the fast-path winner must not be filtered out"
    );
    assert_eq!(
        winner_sol.reference_score,
        Some(runner_up.score),
        "with two non-filtered bids, the winner's reference score is the runner-up's score \
         (winner={winner_sol:?}, runner_up={runner_up:?})",
    );
    assert!(
        winner_sol.score >= runner_up.score,
        "winner score is at least as big as runner up"
    );
    assert!(
        winner_sol.score > 0 && runner_up.score > 0,
        "both solutions have a non-zero score"
    );

    // (2) The order ends up `Fulfilled` after `valid_from`.
    tracing::info!("Waiting for the regular-auction settlement.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
            && model::time::now_in_epoch_seconds() >= valid_from
    })
    .await
    .unwrap();

    // (3) The fallback settlement was submitted by `solver_b`. `solver_a`'s
    // bid gets dropped by the driver's balance check in `Settlement::new`,
    // so the regular auction has no other winner to pick.
    let trade = services
        .get_trades(&uid)
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("settled order should have a trade");
    let tx_hash = trade.tx_hash.expect("settled trade should have a tx hash");
    let competition = services.get_solver_competition(tx_hash).await.unwrap();
    let winner = competition
        .solutions
        .iter()
        .find(|solution| solution.is_winner)
        .expect("settled competition should have a winner");
    assert_eq!(
        winner.solver_address,
        solver_b.address(),
        "fallback settlement should be submitted by solver_b (funded)"
    );
}

/// Two otherwise identical baseline solvers compete on a fast-path order,
/// with realistic volume-fee and slippage settings layered on top:
///
/// * 2% protocol volume fee (configured on both the orderbook and the
///   autopilot) — already baked into the quote returned to the user.
/// * 1% partner volume fee declared in app-data.
/// * 2% solver fee on the bad solver.
/// * user signs at 2% below the (protocol-fee-adjusted) quote. This accounts
///   for the 1% partner fee AND gives 1% slippage on top. The 2% solver fee
///   will not have an issue with the 1% partner fee because the user accounted
///   for that but the 1% slippage is not enough for the 2% solver-fee solution
///   to still clear the bar.
///
/// After the fast-path handler runs, `/solver_competition` must expose the
/// bad solver's solution as `filtered_out = true` (its 2% solver-fee bid
/// compounded with the volume fees no longer clears the signed limit)
/// while the winning solver's solution remains `filtered_out = false`.
async fn fast_path_records_filtered_out_solutions(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    // Both solvers get funded — only the good one will actually submit, but
    // this keeps the setup symmetric so the only real difference between the
    // two is the solver fee.
    let [good_solver, bad_solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    // Long exclusivity so the fast path is the only viable path within the
    // test window — the settlement we observe *must* be the fast-path one.
    let exclusivity = Duration::from_secs(300);

    // `good_solver` is named `test_solver` so the default native-price
    // estimator wiring resolves without an override; `bad_solver` runs the
    // same baseline with a 2% solver fee so its reported bid is 2% below the
    // market rate.
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver_with_solver_fee(
                "test_solver".into(),
                good_solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                0,
            )
            .await,
            colocation::start_baseline_solver_with_solver_fee(
                "bad_solver".into(),
                bad_solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                200, // 2% solver fee
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    // 2% protocol volume fee, configured symmetrically on both sides so the
    // orderbook's placement check and the autopilot's fast-path handler
    // apply the same math. 1% partner volume fee will be declared in
    // app-data further down.
    let protocol_volume_factor: f64 = 0.02;
    let partner_volume_bps: u64 = 100;
    let partner_recipient = Address::repeat_byte(0xb0);

    let quoter_good = ExternalSolver::new("test_solver", "http://localhost:11088/test_solver");
    let quoter_bad = ExternalSolver::new("bad_solver", "http://localhost:11088/bad_solver");
    let autopilot_config = AutopilotConfiguration {
        drivers: vec![
            Solver::test("test_solver", good_solver.address()),
            Solver::test("bad_solver", bad_solver.address()),
        ],
        order_quoting: OrderQuoting::test_with_drivers(vec![
            quoter_good.clone(),
            quoter_bad.clone(),
        ]),
        fee_policies: FeePoliciesConfig {
            policies: vec![ConfigFeePolicy {
                kind: ConfigFeePolicyKind::Volume {
                    factor: protocol_volume_factor.try_into().unwrap(),
                },
                order_class: ConfigFeePolicyOrderClass::Any,
            }],
            // Room for the 1% partner factor.
            max_partner_fee: 0.05.try_into().unwrap(),
            ..Default::default()
        },
        ..AutopilotConfiguration::test_no_drivers()
    };
    let orderbook_config = configs::orderbook::Configuration {
        order_quoting: OrderQuoting::test_with_drivers(vec![quoter_good, quoter_bad]),
        volume_fee: Some(configs::orderbook::VolumeFeeConfig {
            factor: Some(protocol_volume_factor.try_into().unwrap()),
            effective_from_timestamp: None,
        }),
        ..configs::orderbook::Configuration::test_default()
    };
    let (autopilot_config, orderbook_config) =
        with_fast_path_exclusivity(autopilot_config, orderbook_config, exclusivity);

    services.start_autopilot(None, autopilot_config).await;
    services.start_api(orderbook_config).await;

    let app_data = json!({
        "version": "1.1.0",
        "metadata": {
            "enableFastPath": true,
            "partnerFee": {
                "volumeBps": partner_volume_bps,
                "recipient": partner_recipient,
            }
        }
    })
    .to_string();

    tracing::info!("Quoting with enableFastPath and a partner fee.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    // The API's `quote.buy_amount` already reflects the 2% protocol fee. We
    // still need to leave room for the 1% partner fee and ~1% price
    // slippage between quote and settle time, so sign 2% below the quote.
    // At those numbers the winner's fee-adjusted bid (~= quote * 0.99) still
    // clears the signed floor, while the bad solver's 2% solver-fee bid
    // (~= quote * 0.98 * 0.99) falls just below it.
    let signed_buy = quote.quote.buy_amount * U256::from(98u8) / U256::from(100u8);
    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: signed_buy,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    let trade = services
        .get_trades(&uid)
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("settled order should have a trade");
    let tx_hash = trade.tx_hash.expect("settled trade should have a tx hash");
    let competition = services.get_solver_competition(tx_hash).await.unwrap();

    let good = competition
        .solutions
        .iter()
        .find(|solution| solution.solver_address == good_solver.address())
        .expect("good solver's solution must appear in the competition");
    let bad = competition
        .solutions
        .iter()
        .find(|solution| solution.solver_address == bad_solver.address())
        .expect("bad solver's solution must appear in the competition");

    assert!(good.is_winner, "good solver should win the fast-path quote");
    assert!(
        !good.filtered_out,
        "winning solver's bid respects the signed limit and must not be filtered out"
    );
    assert!(
        bad.filtered_out,
        "the 2% solver-fee bid compounded with volume fees can't clear the signed limit and must \
         be filtered out"
    );
    assert!(!bad.is_winner, "bad solver must not have won the quote");
}

/// Configures a protocol volume fee via the autopilot config and a partner
/// volume fee via app-data, and verifies that both are recorded against the
/// fast-path order and that every bid's `executed_sell`/`executed_buy` reflects
/// the compounded fee reduction.
async fn fast_path_volume_fees_captured(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    // 1% protocol volume fee applied to any order class.
    let protocol_volume_factor: f64 = 0.01;
    // 2% partner volume fee (200 bps).
    let partner_volume_bps: u64 = 200;
    let partner_recipient = Address::repeat_byte(0xb0);
    let exclusivity = Duration::from_secs(300);

    let autopilot_config = AutopilotConfiguration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            policies: vec![ConfigFeePolicy {
                kind: ConfigFeePolicyKind::Volume {
                    factor: protocol_volume_factor.try_into().unwrap(),
                },
                order_class: ConfigFeePolicyOrderClass::Any,
            }],
            // Room for the partner factor (2%).
            max_partner_fee: 0.05.try_into().unwrap(),
            ..Default::default()
        },
        ..AutopilotConfiguration::test_no_drivers()
    };
    let (autopilot_config, orderbook_config) = with_fast_path_exclusivity(
        autopilot_config,
        configs::orderbook::Configuration::test_default(),
        exclusivity,
    );
    services
        .start_protocol_with_args(autopilot_config, orderbook_config, solver)
        .await;

    let app_data = json!({
        "version": "1.1.0",
        "metadata": {
            "enableFastPath": true,
            "partnerFee": {
                "bps": partner_volume_bps,
                "recipient": partner_recipient,
            }
        }
    })
    .to_string();

    tracing::info!("Quoting with enableFastPath and a partner fee.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    // Sign a buy amount comfortably below the quote so the fee-reduced
    // `executed_buy` still clears the on-chain limit-price check.
    let signed_buy = quote.quote.buy_amount * U256::from(90u64) / U256::from(100u64);
    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: signed_buy,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // The /trades API rebuilds the fees from `order_execution` (written by
    // the settlement observer once the tx is mined). The observer runs
    // slightly after the trade event, so wait for both fee entries to appear.
    tracing::info!("Waiting for /trades to report both volume fees.");
    wait_for_condition(TIMEOUT, || async {
        services.get_trades(&uid).await.is_ok_and(|trades| {
            trades
                .first()
                .is_some_and(|t| t.executed_protocol_fees.len() == 2)
        })
    })
    .await
    .unwrap();
    let trades = services.get_trades(&uid).await.unwrap();
    assert_eq!(
        trades.len(),
        1,
        "expected one trade for the fast-path order"
    );
    let trade = &trades[0];
    assert_eq!(
        trade.executed_protocol_fees.len(),
        2,
        "expected two Volume fee entries (protocol + partner), got {:?}",
        trade.executed_protocol_fees
    );

    let buy_token = *token.address();
    let expected_partner_factor = partner_volume_bps as f64 / 10_000.0;
    let expected_factors = [protocol_volume_factor, expected_partner_factor];
    for (fee, expected_factor) in trade.executed_protocol_fees.iter().zip(expected_factors) {
        assert_eq!(
            fee.token, buy_token,
            "fees on a sell order are taken from the buy token"
        );
        assert!(!fee.amount.is_zero(), "fee amount should be positive");
        match fee.policy {
            TradeFeePolicy::Volume { factor } => assert!(
                (factor - expected_factor).abs() < 1e-9,
                "unexpected volume factor {factor} (expected {expected_factor})"
            ),
            ref other => panic!("fast-path fees should all be Volume, got {other:?}"),
        }
    }

    // Check the actual captured fee AMOUNTS — the observer reconstructs each
    // observed fee from the final executed buy amount using
    //     fee = base * factor / (1 - factor)
    // applied in reverse policy order (see
    // `autopilot::domain::settlement::trade::math::protocol_fees`). So:
    //     fee_partner  = executed_buy * partner_factor / (1 - partner_factor)
    //     fee_protocol = (executed_buy + fee_partner)
    //                    * protocol_factor / (1 - protocol_factor)
    // If either factor were mis-applied (wrong percentage, wrong ordering,
    // wrong base), these expected amounts would diverge from what the API
    // returns.
    let executed_buy = number::conversions::big_uint_to_u256(&trade.buy_amount)
        .expect("trade buy amount fits in U256");
    let expected_partner_fee = executed_buy
        .checked_mul_f64(expected_partner_factor / (1.0 - expected_partner_factor))
        .expect("partner fee fits in U256");
    let expected_protocol_fee = (executed_buy + expected_partner_fee)
        .checked_mul_f64(protocol_volume_factor / (1.0 - protocol_volume_factor))
        .expect("protocol fee fits in U256");
    assert_approximately_eq!(
        trade.executed_protocol_fees[0].amount,
        expected_protocol_fee
    );
    assert_approximately_eq!(trade.executed_protocol_fees[1].amount, expected_partner_fee);

    // Sanity: the on-chain buy_amount is strictly smaller than what the API
    // quoted — fees actually shrunk the fill, they're not just recorded rows.
    assert!(
        executed_buy < quote.quote.buy_amount,
        "executed buy {executed_buy} should be below the raw quote {} once fees are taken",
        quote.quote.buy_amount
    );
}

/// Ethflow variant of `fast_path_settle`: the order is placed via the ethflow
/// contract (an on-chain `OrderPlacement` event) rather than `POST /orders`,
/// which exercises the autopilot's onchain-event ingestion path into the
/// fast-path handler. Observed strictly via the orderbook HTTP API.
async fn fast_path_ethflow_settle(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(2u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    // Long enough that the fast path is the only viable path within the test
    // window; any settlement observed before `valid_from` proves the fast path
    // fired.
    let exclusivity = Duration::from_secs(300);
    let (autopilot_config, orderbook_config) = with_fast_path_exclusivity(
        AutopilotConfiguration::test("test_solver", solver.address()),
        configs::orderbook::Configuration::test_default(),
        exclusivity,
    );
    services
        .start_protocol_with_args(autopilot_config, orderbook_config, solver)
        .await;

    // Register the app-data blob that carries the fast-path opt-in — the
    // ethflow contract only emits its hash, so the orderbook needs the full
    // payload to reconstruct the metadata. The autopilot's on-chain event
    // ingestion sees `enableFastPath: true` and derives
    // `valid_from = now + fast_path_submission_deadline × block time` itself
    // (same behaviour as the orderbook applies to API-placed orders).
    tracing::info!("Registering fast-path app data.");
    let fast_path_app_data = r#"{"metadata":{"enableFastPath":true}}"#;
    let app_data_hex = services
        .put_app_data(None, fast_path_app_data)
        .await
        .unwrap();
    let app_data_hash = AppDataHash(
        const_hex::decode(&app_data_hex[2..])
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Ethflow orders sign via EIP-1271 (owner is the ethflow contract), so the
    // quote must be requested with that signing scheme.
    tracing::info!("Quoting with enableFastPath.");
    let sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        receiver: Some(trader.address()),
        validity: Validity::For(3600),
        app_data: OrderCreationAppData::Hash {
            hash: app_data_hash,
        },
        signing_scheme: QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            verification_gas_limit: 0,
        },
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        price_quality: PriceQuality::Optimal,
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote_request).await.unwrap();

    tracing::info!("Placing ethflow order on-chain.");
    let valid_to = chrono::offset::Utc::now().timestamp() as u32 + 3600;
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(300);
    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    let placed_at = std::time::Instant::now();
    ethflow_order
        .mine_order_creation(trader.address(), ethflow_contract)
        .await;

    let order_uid = ethflow_order
        .uid(onchain.contracts(), ethflow_contract)
        .await;

    // Autopilot picks the ethflow order up from an on-chain event — mine
    // blocks so the event is processed and the order becomes visible via the
    // orderbook API.
    tracing::info!("Waiting for autopilot to index the ethflow order.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services.get_order(&order_uid).await.is_ok()
    })
    .await
    .unwrap();

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&order_uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // Regular-auction fallback would need to wait for the whole
    // `exclusivity` window to elapse before touching the order. If it
    // Fulfilled well within that window, only the fast path can be
    // responsible.
    let elapsed = placed_at.elapsed();
    assert!(
        elapsed < exclusivity / 2,
        "ethflow order settled after {elapsed:?} — regular auction fallback would have taken at \
         least {exclusivity:?}, so this can't be attributed to the fast path",
    );
}

/// Exercises the fast-path placement-time limit-price check at its
/// boundary conditions. Signing exactly the amount the quote endpoint
/// returned must always be accepted — that's the API's contract with
/// integrators. Signing anything above it (which would force the solver
/// into an unfillable trade once the configured fees are taken) must be
/// rejected with `FastPathLimitTooTight` so the solver isn't put on the
/// hook.
///
/// Two scenarios:
///
/// * **Protocol-fee only.** The quoter already accounts for the configured
///   protocol volume fee, so the API-returned buy amount *is* the boundary:
///   `signed_buy == api_buy` passes, `signed_buy == api_buy
///   + 1` fails.
///
/// * **Protocol fee + partner fee via app-data.** The quoter is oblivious to
///   partner fees, so the API-returned buy amount only reflects the protocol
///   fee. At placement the validator compounds the partner fee on top — the
///   boundary moves to `api_buy * (1 - partner_factor)`. Signing the API amount
///   as-is now fails (partner fee eats into it); signing at the compounded
///   boundary passes.
async fn fast_path_limit_too_tight_rejected(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Enough balance for two 1-WETH sell orders below — both `create_order`
    // paths run the balance check, so we can't reuse the deposit.
    let sell_amount = 1u64.eth();
    let balance_needed = sell_amount * U256::from(2u64);
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, balance_needed)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(balance_needed)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    let exclusivity = Duration::from_secs(300);
    let orderbook_config = configs::orderbook::Configuration {
        volume_fee: Some(configs::orderbook::VolumeFeeConfig {
            factor: Some(0.01_f64.try_into().unwrap()), // 1% protocol volume fee
            effective_from_timestamp: None,
        }),
        ..configs::orderbook::Configuration::test_default()
    };
    // 5% max partner fee budget — comfortably above the 2% we'll declare in
    // the partner-fee scenario further down.
    let autopilot_config = AutopilotConfiguration {
        fee_policies: FeePoliciesConfig {
            max_partner_fee: 0.05_f64.try_into().unwrap(),
            ..Default::default()
        },
        ..AutopilotConfiguration::test("test_solver", solver.address())
    };
    let (autopilot_config, orderbook_config) =
        with_fast_path_exclusivity(autopilot_config, orderbook_config, exclusivity);
    services
        .start_protocol_with_args(autopilot_config, orderbook_config, solver)
        .await;

    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();

    // `AfterFee` semantics: the quote's sell_amount matches the order's
    // signed sell_amount 1:1, so `find_quote` at placement time skips its
    // re-scaling and the fee-adjusted buy amount is bit-identical to the
    // one the API returned. That's what lets the boundary sit at the exact
    // wei of `quote.quote.buy_amount`.
    tracing::info!("Quoting with enableFastPath.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    // The orderbook returns a fee-adjusted quote (`quote.quote.buy_amount`
    // is already `raw_pool_buy * (1 - protocol_fee_factor)`), so that
    // exact value is the boundary between accepted and rejected:
    //   signed_buy <= adjusted_buy → accepted
    //   signed_buy  > adjusted_buy → rejected
    // Exercise both sides — 1 wei on either side of the edge.
    let edge_buy = quote.quote.buy_amount;
    let make_order = |signed_buy: U256| -> OrderCreation {
        OrderCreation {
            quote_id: Some(quote_id),
            sell_token: *onchain.contracts().weth.address(),
            sell_amount,
            buy_token: *token.address(),
            buy_amount: signed_buy,
            valid_to: model::time::now_in_epoch_seconds() + 3600,
            kind: OrderKind::Sell,
            app_data: OrderCreationAppData::Full {
                full: app_data.clone(),
            },
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        )
    };

    tracing::info!(
        %edge_buy,
        "Signing 1 wei above the fee-adjusted quote must be rejected."
    );
    let err = services
        .create_order(&make_order(edge_buy + U256::from(1u64)))
        .await
        .unwrap_err();
    assert_eq!(err.0, reqwest::StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("FastPathLimitTooTight"),
        "error body should mention FastPathLimitTooTight, got: {}",
        err.1
    );

    tracing::info!(%edge_buy, "Signing exactly the fee-adjusted quote must be accepted.");
    // The failed attempt above didn't consume the quote_id (rejected
    // requests don't touch the DB), so we can reuse it here.
    services
        .create_order(&make_order(edge_buy))
        .await
        .expect("signing the fee-adjusted quote as-is must pass the fast-path check");

    // --- Partner-fee scenario ----------------------------------------------
    //
    // The orderbook only applies the protocol volume fee at quote time —
    // partner fees are opaque to the quoter. Yet at placement time the
    // fast-path check must account for *both* stacks. A user who signs the
    // API-returned buy amount (protocol-fee-adjusted) but attaches a partner
    // fee in the app-data is asking for a settlement the pool can't produce
    // without violating their limit. The check must reject that.
    let partner_recipient = Address::repeat_byte(0xb0);
    let partner_volume_bps: u64 = 200; // 2% partner volume fee
    let partner_app_data = json!({
        "metadata": {
            "enableFastPath": true,
            "partnerFee": {
                "volumeBps": partner_volume_bps,
                "recipient": partner_recipient,
            }
        }
    })
    .to_string();

    tracing::info!("Quoting again with a 2% partner volume fee in the app-data.");
    let partner_quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: partner_app_data.clone(),
        },
        ..Default::default()
    };
    let partner_quote = services.submit_quote(&partner_quote_request).await.unwrap();
    let partner_quote_id = partner_quote
        .id
        .expect("fast-path quote should carry an id");

    // The API-returned amount is `raw * (1 - protocol_factor)` — the partner
    // fee is invisible to the quoter. Placement compounds it: at settlement
    // the trader would receive `api_buy * (1 - partner_factor)`.
    let api_buy = partner_quote.quote.buy_amount;
    let partner_factor: configs::fee_factor::FeeFactor =
        (partner_volume_bps as f64 / 10_000.0).try_into().unwrap();
    let partner_fee_amount = shared::fee::compute_volume_fee(api_buy, partner_factor);
    let partner_edge_buy = api_buy - partner_fee_amount;

    let make_partner_order = |signed_buy: U256| -> OrderCreation {
        OrderCreation {
            quote_id: Some(partner_quote_id),
            sell_token: *onchain.contracts().weth.address(),
            sell_amount,
            buy_token: *token.address(),
            buy_amount: signed_buy,
            valid_to: model::time::now_in_epoch_seconds() + 3600,
            kind: OrderKind::Sell,
            app_data: OrderCreationAppData::Full {
                full: partner_app_data.clone(),
            },
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        )
    };

    tracing::info!(
        %api_buy, %partner_edge_buy,
        "Signing the (protocol-fee-only) API amount must be rejected once the \
         partner fee compounds on top."
    );
    let err = services
        .create_order(&make_partner_order(api_buy))
        .await
        .unwrap_err();
    assert_eq!(err.0, reqwest::StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("FastPathLimitTooTight"),
        "error body should mention FastPathLimitTooTight, got: {}",
        err.1
    );

    // 1 wei above the partner-adjusted edge must still be rejected.
    let err = services
        .create_order(&make_partner_order(partner_edge_buy + U256::from(1u64)))
        .await
        .unwrap_err();
    assert_eq!(err.0, reqwest::StatusCode::BAD_REQUEST);
    assert!(err.1.contains("FastPathLimitTooTight"));

    tracing::info!(
        %partner_edge_buy,
        "Signing at the partner-fee-adjusted amount must be accepted."
    );
    services
        .create_order(&make_partner_order(partner_edge_buy))
        .await
        .expect(
            "signing the protocol- and partner-fee-adjusted amount must pass the fast-path check",
        );
}
