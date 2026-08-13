use {
    crate::{
        domain::competition::order,
        tests::{
            self,
            cases::EtherExt,
            setup::{self, ab_order, ab_pool, ab_solution},
        },
    },
    eth_domain_types as eth,
    number::testing::ApproxEq,
};

/// Extracts the buy amount from a quote response using clearing prices.
///
/// For a sell order, calculates: `sell_amount * sell_price / buy_price`
/// Since the sell token has the lower price in our test setup, this becomes:
/// `sell_amount * price_low / price_high`
fn extract_buy_amount(response_body: &str, sell_amount: eth::U256) -> eth::U256 {
    let body: serde_json::Value = serde_json::from_str(response_body).unwrap();
    let clearing_prices = body.get("clearingPrices").unwrap().as_object().unwrap();

    let mut prices: Vec<eth::U256> = clearing_prices
        .values()
        .map(|v| v.as_str().unwrap().parse::<eth::U256>().unwrap())
        .collect();
    prices.sort();

    let (price_low, price_high) = (prices[0], prices[1]);
    sell_amount * price_low / price_high
}

/// Run a matrix of tests for all meaningful combinations of order kind and
/// side, verifying that they get quoted successfully.
#[tokio::test]
#[ignore]
async fn matrix() {
    for side in [order::Side::Buy, order::Side::Sell] {
        for kind in [order::Kind::Market, order::Kind::Limit] {
            let test = tests::setup()
                .name(format!("{side:?} {kind:?}"))
                .pool(ab_pool())
                .order(ab_order().side(side).kind(kind))
                .solution(ab_solution())
                .quote()
                .done()
                .await;

            let quote = test.quote().await;

            quote.ok().amount().interactions();
        }
    }
}

#[tokio::test]
#[ignore]
async fn with_jit_order() {
    let side = order::Side::Sell;
    let kind = order::Kind::Limit;
    let jit_order = setup::JitOrder {
        order: ab_order()
            .kind(order::Kind::Limit)
            .side(side)
            .kind(kind)
            .pre_interaction(setup::blockchain::Interaction {
                address: ab_order().owner,
                calldata: std::iter::repeat_n(0xab, 32).collect(),
                inputs: Default::default(),
                outputs: Default::default(),
                internalize: false,
            })
            .no_surplus(),
    };

    let test = tests::setup()
        .name(format!("{side:?} {kind:?}"))
        .pool(ab_pool())
        .jit_order(jit_order)
        .order(ab_order().side(side).kind(kind).no_surplus())
        .solution(ab_solution())
        .quote()
        .done()
        .await;

    let quote = test.quote().await;

    // Check whether the returned data aligns with the expected.
    quote.ok().amount().interactions().jit_order();
}

/// A fast-path quote caches its solution in the driver, echoing the solution id
/// so the autopilot can later settle it.
#[tokio::test]
#[ignore]
async fn fast_path_caching() {
    let test = tests::setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().fast_path_enabled()])
        .auction_id(42)
        .quote()
        .quote_fast_path()
        .done()
        .await;

    let quote = test.quote().await.ok();

    // The solution id is echoed only when the solution was cached (its value is
    // process-global, so not asserted). The accessor panics if it is absent.
    quote.solution_id();
}

/// A regular quote (no `enableFastPath`), even from a fast-path-capable solver,
/// does not cache a settlement and carries no settle info.
#[tokio::test]
#[ignore]
async fn regular_quote_has_no_settle_info() {
    let test = tests::setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().fast_path_enabled()])
        .quote()
        .done()
        .await;

    test.quote().await.ok().no_fast_path_settle_info();
}

/// Set up a fast-path quote test: a fast-path solver quoting `ab_order`.
async fn fast_path_test() -> setup::Test {
    tests::setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .solvers(vec![setup::test_solver().fast_path_enabled()])
        .auction_id(42)
        .quote()
        .quote_fast_path()
        .done()
        .await
}

/// `/settle` carrying the real signed order re-encodes the cached solution and
/// executes on-chain.
#[tokio::test]
#[ignore]
async fn fast_path_settle() {
    let test = fast_path_test().await;
    let solution_id = test.quote().await.ok().solution_id();
    test.settle_with_order(
        solution_id,
        test.order_json(),
        test.limit_prices_json(),
        test.prices_json(),
    )
    .await
    .ok()
    .await
    .ab_order_executed(&test)
    .await;
}

/// The re-encoded settlement fills the order at exactly the signed limit, not
/// the surplus the cached route would otherwise deliver.
#[tokio::test]
#[ignore]
async fn fast_path_settle_fills_at_limit() {
    let test = fast_path_test().await;
    let solution_id = test.quote().await.ok().solution_id();
    let limit_prices = test.limit_prices_json();
    let expected_buy: eth::U256 = limit_prices["buy"].as_str().unwrap().parse().unwrap();
    test.settle_with_order(
        solution_id,
        test.order_json(),
        limit_prices,
        test.prices_json(),
    )
    .await
    .ok()
    .await
    .balance(&test, "B", setup::Balance::GreaterBy(expected_buy))
    .await;
}

/// Without the real order there is nothing to re-encode, so `/settle` finds no
/// settlement.
#[tokio::test]
#[ignore]
async fn fast_path_settle_requires_order() {
    let test = fast_path_test().await;
    let solution_id = test.quote().await.ok().solution_id();
    test.settle(solution_id)
        .await
        .err()
        .kind("SolutionNotAvailable");
}

/// A settle order that doesn't match the quote — different buy token — is
/// rejected as an invalid fast-path order.
#[tokio::test]
#[ignore]
async fn fast_path_settle_rejects_mismatched_order() {
    let test = fast_path_test().await;
    let solution_id = test.quote().await.ok().solution_id();
    let mut order = test.order_json();
    order["buyToken"] = serde_json::json!("0x0101010101010101010101010101010101010101");
    test.settle_with_order(
        solution_id,
        order,
        test.limit_prices_json(),
        test.prices_json(),
    )
    .await
    .err()
    .kind("FastPathOrderMismatch");
}

/// A settle order for a different amount than the quote is rejected.
#[tokio::test]
#[ignore]
async fn fast_path_settle_rejects_wrong_amount() {
    let test = fast_path_test().await;
    let solution_id = test.quote().await.ok().solution_id();
    let mut order = test.order_json();
    order["sellAmount"] = serde_json::json!("1");
    test.settle_with_order(
        solution_id,
        order,
        test.limit_prices_json(),
        test.prices_json(),
    )
    .await
    .err()
    .kind("FastPathOrderMismatch");
}

/// A settle whose limit prices demand more than the cached quote can deliver is
/// rejected, so the order falls back to the normal auction.
#[tokio::test]
#[ignore]
async fn fast_path_settle_rejects_tight_limit() {
    let test = fast_path_test().await;
    let solution_id = test.quote().await.ok().solution_id();
    let mut limit_prices = test.limit_prices_json();
    limit_prices["buy"] = serde_json::json!("1000000000000000000000000000000");
    test.settle_with_order(
        solution_id,
        test.order_json(),
        limit_prices,
        test.prices_json(),
    )
    .await
    .err()
    .kind("FastPathLimitNotMet");
}

/// Test that quote haircut correctly reduces the executed amount for quotes
/// when configured. The haircut should make quotes more conservative without
/// affecting the ability to place and execute orders.
#[tokio::test]
#[ignore]
async fn with_quote_haircut() {
    // Test with a sell order - haircut should reduce the buy amount user receives
    // Set up an order that sells 50 A tokens for at least 40 B tokens (creating
    // slack) The solver will quote ~41-42 B tokens, leaving room for 2% haircut
    let test_no_haircut = tests::setup()
        .name("Sell order without haircut (baseline)")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(order::Side::Sell)
                .kind(order::Kind::Limit)
                .buy_amount(40u64.ether().into_wei()) // Set a limit to create slack
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(0)]) // No haircut
        .quote()
        .done()
        .await;

    let quote_no_haircut = test_no_haircut.quote().await;
    let response_no_haircut = quote_no_haircut.ok();

    let sell_amount = ab_order().sell_amount;
    let buy_amount_no_haircut = extract_buy_amount(response_no_haircut.body(), sell_amount);

    // Now get a quote with 200 bps (2%) haircut
    let test_with_haircut = tests::setup()
        .name("Sell order with 200 bps (2%) haircut")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(order::Side::Sell)
                .kind(order::Kind::Limit)
                .buy_amount(40u64.ether().into_wei()) // Same limit to create slack
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(200)]) // 2% haircut
        .quote()
        .done()
        .await;

    let quote_with_haircut = test_with_haircut.quote().await;
    let response_with_haircut = quote_with_haircut.ok();

    let buy_amount_with_haircut = extract_buy_amount(response_with_haircut.body(), sell_amount);

    // Verify haircut was applied: haircutted amount should be ~2% less than
    // baseline Expected: buy_amount_with_haircut ≈ buy_amount_no_haircut * 0.98
    let expected_haircutted = buy_amount_no_haircut * eth::U256::from(98) / eth::U256::from(100);

    // Calculate actual haircut in basis points for diagnostics
    let ratio = buy_amount_with_haircut * eth::U256::from(10000) / buy_amount_no_haircut;
    let haircut_bps = eth::U256::from(10000) - ratio;

    tracing::info!(
        buy_amount_no_haircut = %buy_amount_no_haircut,
        buy_amount_with_haircut = %buy_amount_with_haircut,
        expected_haircutted = %expected_haircutted,
        haircut_bps = %haircut_bps,
        "Comparing buy amounts with and without haircut"
    );

    // The haircutted amount should be approximately 2% less (within 1% tolerance)
    assert!(
        buy_amount_with_haircut.is_approx_eq(&expected_haircutted, Some(0.01)),
        "Haircutted amount {} should be approximately 2% less than baseline {} (expected: {}, \
         actual haircut: {} bps)",
        buy_amount_with_haircut,
        buy_amount_no_haircut,
        expected_haircutted,
        haircut_bps
    );
}
