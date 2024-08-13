use {
    crate::{
        infra::config::file::{FeeHandler, OrderPriorityStrategy},
        tests::{
            cases::EtherExt,
            setup::{ab_order, ab_pool, ab_solution, setup, test_solver, Order, OrderQuote},
        },
    },
    chrono::Utc,
    std::time::Duration,
};

/// Test that orders are correctly sorted before being sent to the solver:
/// - Market orders precede limit orders.
/// - Own quoted orders within the default order creation timestamp threshold.
/// - Orders are then sorted by creation timestamp within the default threshold.
/// - Orders with higher fulfillment likelihood (based on token prices in ETH).
#[tokio::test]
#[ignore]
async fn default_sorting() {
    let now = Utc::now().timestamp() as u32;
    let solver = test_solver().fee_handler(FeeHandler::Driver);
    let test = setup()
        .solvers(vec![solver.clone()])
        .pool(ab_pool())
        // A different `valid_to` is required in order to generate a separate UID for orders
        // with the same buy/sell amounts, since it participates in the hash function,
        // which makes it possible to validate orders sorting.
        //
        // Orders with own quotes come first.
        .order(
            ab_order()
                .created(now - 1)
                .quote(OrderQuote::default().solver(solver.address()))
                .reduce_amount("1e-2".ether().into_wei())
                .valid_to(u32::MAX - 1)
        )
        // Most recent orders get higher priority.
        .order(
            ab_order()
                .rename("2")
                .created(now - 2)
                .quote(OrderQuote::default().solver(solver.address()))
                .reduce_amount("1e-2".ether().into_wei())
                .valid_to(u32::MAX - 2)
        )
        // Orders with better price are prioritized.
        .order(
            ab_order()
                .rename("3")
                .created(now - 2)
                .quote(OrderQuote::default().solver(solver.address()))
                .reduce_amount("1e-1".ether().into_wei())
                .valid_to(u32::MAX - 2)
        )
        // Even though the order was created earlier and has a better price, it comes after the own quoted orders.
        .order(ab_order().rename("4").created(now))
        // Most recent orders get higher priority.
        .order(ab_order().rename("5").created(now - 1).valid_to(u32::MAX - 3))
        // Similar to the previous order, but has a worse price gets lowest priority.
        .order(
            ab_order()
                .rename("6")
                .created(now - 1)
                .reduce_amount("1e-2".ether().into_wei())
                .valid_to(u32::MAX - 4)
        )
        // The order was quoted by the solver and has one of the best prices, but it is too old.
        .order(
            ab_order()
                .rename("7")
                .created(now - 310)
                .quote(OrderQuote::default().solver(solver.address()))
                .valid_to(u32::MAX - 5)
        )
        // Limit orders come after market orders.
        .order(ab_order().rename("8").created(now).limit())
        .order(
            ab_order()
                .rename("9")
                .created(now)
                .reduce_amount("1e-3".ether().into_wei())
                .limit()
        )
        .solution(ab_solution())
        .done()
        .await;

    // Only check that the solve endpoint can be called successfully, which means
    // that the solver received the orders sorted.
    test.solve().await.ok();
}

/// The most recent orders get prioritized no matter other factors.
#[tokio::test]
#[ignore]
async fn creation_timestamp_sorting() {
    let now = Utc::now().timestamp() as u32;
    let solver = test_solver().fee_handler(FeeHandler::Driver);
    let test = setup()
        .solvers(vec![solver.clone()])
        .pool(ab_pool())
        .order_priority_strategy(OrderPriorityStrategy::CreationTimestamp {
            max_order_age: Some(Duration::from_secs(10)),
        })
        .order(
            ab_order()
                .created(now)
                .reduce_amount("1e-3".ether().into_wei()),
        )
        .order(
            ab_order()
                .rename("2")
                .limit()
                .created(now - 1)
                .reduce_amount("1e-2".ether().into_wei())
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .order(
            ab_order()
                .rename("3")
                .created(now - 20)
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok();
}

/// Orders with better price get prioritized no matter other factors.
#[tokio::test]
#[ignore]
async fn external_price_sorting() {
    let now = Utc::now().timestamp() as u32;
    let solver = test_solver().fee_handler(FeeHandler::Driver);
    let test = setup()
        .solvers(vec![solver.clone()])
        .pool(ab_pool())
        .order_priority_strategy(OrderPriorityStrategy::ExternalPrice)
        .order(ab_order())
        .order(
            ab_order()
                .rename("2")
                .limit()
                .created(now)
                .reduce_amount("1e-2".ether().into_wei())
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .order(
            ab_order()
                .rename("3")
                .created(now)
                .reduce_amount("1e-1".ether().into_wei())
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok();
}

/// Own quoted orders get prioritized no matter other factors.
#[tokio::test]
#[ignore]
async fn own_quotes_sorting() {
    let now = Utc::now().timestamp() as u32;
    let solver = test_solver().fee_handler(FeeHandler::Driver);
    let test = setup()
        .solvers(vec![solver.clone()])
        .pool(ab_pool())
        .order_priority_strategy(OrderPriorityStrategy::OwnQuotes {
            max_order_age: Some(Duration::from_secs(10)),
        })
        .order(
            ab_order()
                .created(now - 1)
                .reduce_amount("1e-1".ether().into_wei())
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .order(
            ab_order()
                .rename("2")
                .created(now - 20)
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok();
}

/// Test that orders are sorted according to the provided custom strategies
/// list:
/// - External price
/// - Creation timestamp with 10s threshold
/// - Own quotes with 20s threshold
/// - Order class
#[tokio::test]
#[ignore]
async fn mixed_custom_sorting() {
    let now = Utc::now().timestamp() as u32;
    let solver = test_solver().fee_handler(FeeHandler::Driver);
    let test = setup()
        .solvers(vec![solver.clone()])
        .pool(ab_pool())
        .order_priority_strategy(OrderPriorityStrategy::ExternalPrice)
        .order_priority_strategy(OrderPriorityStrategy::CreationTimestamp {
            max_order_age: Some(Duration::from_secs(10)),
        })
        .order_priority_strategy(OrderPriorityStrategy::OwnQuotes {
            max_order_age: Some(Duration::from_secs(20)),
        })
        .order_priority_strategy(OrderPriorityStrategy::OrderClass)
        .order(ab_order())
        .order(
            ab_order()
                .rename("2")
                .created(now - 1)
                .reduce_amount("1e-1".ether().into_wei()),
        )
        .order(
            ab_order()
                .rename("3")
                .created(now - 2)
                .reduce_amount("1e-1".ether().into_wei())
                .valid_to(u32::MAX - 1)
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .order(
            ab_order()
                .rename("4")
                .created(now - 15)
                .reduce_amount("1e-1".ether().into_wei())
                .valid_to(u32::MAX - 3)
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .order(
            ab_order()
                .rename("5")
                .created(now - 25)
                .reduce_amount("1e-1".ether().into_wei())
                .valid_to(u32::MAX - 4)
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .order(
            ab_order()
                .rename("6")
                .limit()
                .created(now - 25)
                .reduce_amount("1e-1".ether().into_wei())
                .valid_to(u32::MAX - 5)
                .quote(OrderQuote::default().solver(solver.address())),
        )
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok();
}

/// If a user does not have enough tokens to settle all their orders filter out
/// the least likely to settle ones that go over the user's budget.
#[tokio::test]
#[ignore]
async fn filtering() {
    let test = setup()
        .pool(ab_pool())
        // Orders with better price ratios come first.
        .order(ab_order())
        .order(ab_order().reduce_amount("1e-3".ether().into_wei()).rename("second order"))
        // Filter out the next order, because the trader doesn't have enough balance to cover it.
        .order(
            ab_order()
                .rename("third order")
                .multiply_amount("0.1".ether().into_wei())
                .filtered()
        )
        // Filter out the next order. It can't be fulfilled due to the balance that is required to
        // fulfill the previous orders.
        .order(
            Order {
                sell_amount: "4999999999900.002".ether().into_wei(),
                surplus_factor: 1.into(),
                ..ab_order()
            }
                .rename("fourth order")
                .unfunded()
                .filtered()
                .limit()
        )
        .solution(ab_solution())
        .done()
        .await;

    // Only check that the solve endpoint can be called successfully, which means
    // that the solver received the orders sorted.
    test.solve().await.ok();
}
