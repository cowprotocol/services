use crate::{
    domain::{competition::order, eth},
    tests::{
        self,
        cases::EtherExt,
        setup::{self, ab_order, ab_pool, ab_solution},
    },
};

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

    // Parse clearing prices from JSON response
    let body_no_haircut: serde_json::Value =
        serde_json::from_str(response_no_haircut.body()).unwrap();
    tracing::info!(
        "Quote response without haircut: {}",
        serde_json::to_string_pretty(&body_no_haircut).unwrap()
    );
    let clearing_prices = body_no_haircut
        .get("clearingPrices")
        .unwrap()
        .as_object()
        .unwrap();

    // Extract prices and calculate buy amount
    // For our test: sell_amount * sell_price / buy_price
    // Since we don't know which token has which price from sorting alone,
    // we use the ratio that gives a reasonable result (price_low / price_high)
    let order = ab_order();
    let sell_amount = order.sell_amount;
    let mut prices: Vec<eth::U256> = clearing_prices
        .values()
        .map(|v| v.as_str().unwrap().parse::<eth::U256>().unwrap())
        .collect();
    prices.sort();
    let (price_low, price_high) = (prices[0], prices[1]);
    // Note: in our test setup, sell token has lower price, so:
    // buy_amount = sell_amount * (price_low / price_high)
    let buy_amount_no_haircut = sell_amount * price_low / price_high;

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

    // Parse clearing prices from JSON response
    let body_with_haircut: serde_json::Value =
        serde_json::from_str(response_with_haircut.body()).unwrap();
    tracing::info!(
        "Quote response with haircut: {}",
        serde_json::to_string_pretty(&body_with_haircut).unwrap()
    );
    let clearing_prices_haircut = body_with_haircut
        .get("clearingPrices")
        .unwrap()
        .as_object()
        .unwrap();

    let mut prices_haircut: Vec<eth::U256> = clearing_prices_haircut
        .values()
        .map(|v| v.as_str().unwrap().parse::<eth::U256>().unwrap())
        .collect();
    prices_haircut.sort();
    let (price_low_haircut, price_high_haircut) = (prices_haircut[0], prices_haircut[1]);
    let buy_amount_with_haircut = sell_amount * price_low_haircut / price_high_haircut;

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

    // The haircutted amount should be approximately 2% less
    // Allow 1% tolerance for rounding and other factors (between 1% and 3% haircut)
    let lower_bound = buy_amount_no_haircut * eth::U256::from(97) / eth::U256::from(100); // 97%
    let upper_bound = buy_amount_no_haircut * eth::U256::from(99) / eth::U256::from(100); // 99%
    assert!(
        buy_amount_with_haircut >= lower_bound && buy_amount_with_haircut <= upper_bound,
        "Haircutted amount {} should be approximately 2% less than baseline {} (expected range: \
         {} to {}, actual haircut: {} bps)",
        buy_amount_with_haircut,
        buy_amount_no_haircut,
        lower_bound,
        upper_bound,
        haircut_bps
    );
}
