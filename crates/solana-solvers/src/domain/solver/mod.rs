//! Solve loop: the orders of each side and directed mint pair, quoted in
//! batches.

use {
    crate::{
        dex::{self, Dex},
        dto::{
            auction::{self, Auction},
            solution::Solution,
        },
    },
    configs::rate_limit::Strategy,
    futures::{Stream, StreamExt, future, stream},
    rate_limit::RateLimiter,
    solana_sdk::pubkey::Pubkey,
    std::{cmp::Ordering, collections::HashMap, future::Future, num::NonZeroUsize},
    tracing::Instrument,
};

/// Prices orders and builds the swaps of the quotes kept. A seam over [`Dex`]
/// so the loop is testable without the network.
pub trait Aggregator {
    fn quote(
        &self,
        order: &dex::Order,
    ) -> impl Future<Output = Result<dex::Quote, dex::jupiter::Error>> + Send;

    fn swap(
        &self,
        quote: &dex::Quote,
        taker: &Pubkey,
    ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send;
}

impl Aggregator for Dex {
    fn quote(
        &self,
        order: &dex::Order,
    ) -> impl Future<Output = Result<dex::Quote, dex::jupiter::Error>> + Send {
        Dex::quote(self, order)
    }

    fn swap(
        &self,
        quote: &dex::Quote,
        taker: &Pubkey,
    ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send {
        Dex::swap(self, quote, taker)
    }
}

/// Quotes an auction's orders into solutions, as [`Solver::solve`] describes.
pub struct Solver<Q> {
    quoter: Q,
    concurrent_requests: NonZeroUsize,
    rate_limiter: RateLimiter,
    /// See `Config::max_batch_size`.
    max_batch_size: NonZeroUsize,
}

impl<Q: Aggregator + Sync> Solver<Q> {
    pub fn new(
        quoter: Q,
        concurrent_requests: NonZeroUsize,
        rate_limiting: Strategy,
        max_batch_size: NonZeroUsize,
    ) -> Self {
        Self {
            quoter,
            concurrent_requests,
            rate_limiter: RateLimiter::from_strategy(rate_limiting, "dex_api".to_string()),
            max_batch_size,
        }
    }

    /// The solutions quoted before the deadline, one per batch of orders of
    /// one side on one directed mint pair.
    ///
    /// The autopilot's arbitrator picks at most one winner per directed
    /// pair, so a batch is how more than one of a pair's orders fill in one
    /// auction. The orders of a side are quoted most permissive first, in
    /// batches of up to `max_batch_size`, each as one swap over its total.
    /// If the swap's rate undercuts an order, the least permissive one is
    /// dropped and the rest requoted, so a batch clears, trims down to a
    /// single order, or yields nothing. Every batch that clears is a
    /// solution; which of a pair's batches wins is the arbitrator's call.
    /// Buys (when disabled) and orders the aggregator cannot route yield no
    /// candidate; the rest of the auction still proceeds. At most
    /// `concurrent_requests` quotes run at a time, pausing after a
    /// rate-limited response.
    ///
    /// TODO(BE-308): retry partially fillable orders at smaller amounts when
    /// the swap undercuts the limit, like the EVM engine's `Fills`. The
    /// wire's `partiallyFillable` is ignored until then.
    pub async fn solve(&self, auction: &Auction) -> Vec<Solution> {
        let mut solutions = Vec::new();
        let solve_orders = async {
            let mut stream = self.solution_stream(auction);
            while let Some(solution) = stream.next().await {
                solutions.push(solution);
            }
        };

        let remaining = (auction.deadline - chrono::Utc::now())
            .to_std()
            .unwrap_or_default();
        if tokio::time::timeout(remaining, solve_orders).await.is_err() {
            tracing::debug!("deadline reached, stopping to solve");
        }
        solutions
    }

    fn solution_stream<'a>(&'a self, auction: &'a Auction) -> impl Stream<Item = Solution> + 'a {
        // A batch's index is its solution's id, unique within one response.
        stream::iter(self.batches(auction).into_iter().enumerate())
            .map(|(id, orders)| {
                let span = tracing::info_span!(
                    "solve",
                    auction_id = ?auction.id,
                    batch = id,
                    orders = ?orders.iter().map(|order| order.uid).collect::<Vec<_>>()
                );
                self.solve_batch(id as u64, orders, auction)
                    .instrument(span)
            })
            .buffer_unordered(self.concurrent_requests.get())
            .filter_map(future::ready)
    }

    /// The batches to quote: the orders of each side, directed mint pair and
    /// swap destination, most permissive first, in runs of up to
    /// `max_batch_size` whose quoted total fits a `u64`. Sells and buys stay
    /// apart since one swap fixes either its input or its output.
    ///
    /// The order decides who is left out when the deadline or the rate
    /// limit cuts quoting short. Every pair's first batch comes before any
    /// pair's second, and pairs go by their most permissive order's
    /// [`limit_to_native`], highest first, then the pairs whose sell mint
    /// has no price. Ties keep their order of first appearance, so one
    /// auction batches the same on every run. A pair whose buy mint has no
    /// price is left out: the arbitrator cannot score its solution.
    fn batches<'a>(&self, auction: &'a Auction) -> Vec<Vec<&'a auction::Order>> {
        let mut pairs: Vec<Vec<&auction::Order>> = Vec::new();
        let mut pair_index = HashMap::new();
        for order in &auction.orders {
            let pair = *pair_index
                .entry((
                    order.side,
                    order.sell_mint,
                    order.buy_mint,
                    order.buy_destination,
                ))
                .or_insert_with(|| {
                    pairs.push(Vec::new());
                    pairs.len() - 1
                });
            pairs[pair].push(order);
        }
        let mut ranked: Vec<_> = pairs
            .into_iter()
            .filter_map(|mut orders| {
                orders.sort_by(|a, b| by_permissiveness(a, b));
                // Within a pair the prices are shared, so its most
                // permissive order, the first, stands for it.
                let head = *orders.first()?;
                if auction.native_price(&head.buy_mint).is_none() {
                    tracing::debug!(
                        orders = ?orders.iter().map(|order| order.uid).collect::<Vec<_>>(),
                        "skipping pair, its buy mint has no native price"
                    );
                    return None;
                }
                Some((orders, limit_to_native(head, auction)))
            })
            .collect();
        // Stable, highest first, `None` last.
        ranked.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));
        tracing::debug!(
            ranking = ?ranked
                .iter()
                .map(|(orders, limit_to_native)| (orders[0].uid, limit_to_native))
                .collect::<Vec<_>>(),
            "pairs by limit to native rate"
        );
        let per_pair: Vec<Vec<Vec<&auction::Order>>> = ranked
            .into_iter()
            .map(|(orders, _)| split(&orders, self.max_batch_size))
            .collect();
        let ranks = per_pair.iter().map(Vec::len).max().unwrap_or(0);
        (0..ranks)
            .flat_map(|rank| {
                per_pair
                    .iter()
                    .filter_map(move |batches| batches.get(rank).cloned())
            })
            .collect()
    }

    /// The solution for one batch, see [`Solver::solve`]. Rounds only price
    /// the batch; the swap is built once it clears.
    async fn solve_batch(
        &self,
        id: u64,
        mut orders: Vec<&auction::Order>,
        auction: &Auction,
    ) -> Option<Solution> {
        while !orders.is_empty() {
            let batch = batch_order(&orders)?;
            let quote = self.limited("quote", self.quoter.quote(&batch)).await?;
            // The limits are checked at the rate the solution's prices will
            // encode, which for a batch of buys is a little worse than the
            // swap's own.
            let (in_amount, out_amount) = quote.clearing_rate(batch.side, orders.len())?;
            if orders
                .iter()
                .all(|order| dex::satisfies(&order.to_dex_order(), in_amount, out_amount))
            {
                let swap = self
                    .limited("swap", self.quoter.swap(&quote, &auction.taker))
                    .await?;
                return assemble(id, &orders, swap);
            }
            // The orders are sorted most to least permissive. Removing only
            // the tail matters because price impact can make orders that fail
            // at this size clear once the batch is smaller.
            let order = orders.pop()?;
            tracing::debug!(
                order = %order.uid,
                in_amount,
                out_amount,
                shortfall = %dex::shortfall(&order.to_dex_order(), in_amount, out_amount),
                "swap does not satisfy order, requoting without it"
            );
        }
        tracing::debug!("swap satisfies no order");
        None
    }

    /// One aggregator request through the rate limiter, logged under `what`.
    /// A request dropped during a back-off counts as rate limited.
    async fn limited<T>(
        &self,
        what: &'static str,
        request: impl Future<Output = Result<T, dex::jupiter::Error>>,
    ) -> Option<T> {
        self.rate_limiter
            .execute_with_back_off(request, |result| {
                matches!(result, Err(dex::jupiter::Error::RateLimited))
            })
            .await
            .unwrap_or(Err(dex::jupiter::Error::RateLimited))
            .inspect_err(|err| match err {
                dex::jupiter::Error::NotFound | dex::jupiter::Error::OrderNotSupported => {
                    tracing::debug!(%err, "no {what}")
                }
                dex::jupiter::Error::RateLimited => tracing::debug!("{what} rate limited"),
                _ => tracing::warn!(%err, "{what} failed"),
            })
            .ok()
    }
}

/// The solution filling `orders`, which `swap` clears: a single-order
/// solution for one order, a batch for more.
fn assemble(id: u64, orders: &[&auction::Order], swap: dex::Swap) -> Option<Solution> {
    if let [order] = orders {
        let solution = Solution::new(id, order.uid, &order.to_dex_order(), swap).ok()?;
        tracing::debug!("solved");
        return Some(solution);
    }
    let orders: Vec<_> = orders
        .iter()
        .map(|order| (order.uid, order.to_dex_order()))
        .collect();
    match Solution::batch(id, &orders, swap) {
        Ok(solution) => {
            tracing::debug!(orders = orders.len(), "solved batch");
            Some(solution)
        }
        Err(err) => {
            tracing::warn!(%err, "batch swap does not assemble");
            None
        }
    }
}

/// Orders the orders of one side by their limit price, the least buy per
/// unit sold first: the lowest minimum out for a sell, the highest maximum
/// in for a buy. A swap's rate then undercuts a tail of the sorted list. The
/// same cross-multiplication as [`dex::satisfies`], so no division rounds.
/// Ties go by uid, so the order is the same on every run. An order with
/// nothing to sell has no limit price and goes last.
fn by_permissiveness(a: &auction::Order, b: &auction::Order) -> std::cmp::Ordering {
    (a.sell_amount == 0)
        .cmp(&(b.sell_amount == 0))
        .then_with(|| {
            (u128::from(a.buy_amount) * u128::from(b.sell_amount))
                .cmp(&(u128::from(b.buy_amount) * u128::from(a.sell_amount)))
        })
        .then_with(|| a.uid.0.cmp(&b.uid.0))
}

/// How far `order`'s limit sits inside the native exchange rate: the
/// native value it gives up over the native value its limit accepts,
/// `sell × p_sell / (buy × p_buy)`, for sells and buys alike. Above 1 the
/// native rate clears the limit, and the higher it is the more room there
/// is for price impact. Size-independent like the EVM driver's
/// `ExternalPrice` ordering, and on one pair it ranks like
/// [`by_permissiveness`]. `None` when a mint has no price; an order
/// accepting nothing is infinitely permissive.
fn limit_to_native(order: &auction::Order, auction: &Auction) -> Option<f64> {
    let given = order.sell_amount as f64 * auction.native_price(&order.sell_mint)? as f64;
    let accepted = order.buy_amount as f64 * auction.native_price(&order.buy_mint)? as f64;
    Some(match (given > 0., accepted > 0.) {
        (_, true) => given / accepted,
        (true, false) => f64::INFINITY,
        (false, false) => 0.,
    })
}

/// Splits `orders`, all of one side and sorted most permissive first, into
/// runs of up to `max_batch_size` whose quoted amounts sum within a `u64`,
/// so each run can be quoted as one swap.
fn split<'a>(
    orders: &[&'a auction::Order],
    max_batch_size: NonZeroUsize,
) -> Vec<Vec<&'a auction::Order>> {
    let mut batches: Vec<Vec<&'a auction::Order>> = Vec::new();
    let mut total: u64 = 0;
    for &order in orders {
        let amount = order.to_dex_order().amount();
        let open = batches
            .last_mut()
            .filter(|batch| batch.len() < max_batch_size.get());
        match open.zip(total.checked_add(amount)) {
            Some((batch, sum)) => {
                batch.push(order);
                total = sum;
            }
            None => {
                batches.push(vec![order]);
                total = amount;
            }
        }
    }
    batches
}

/// The swap to quote for `orders`, orders of one side on one pair: their
/// mints, destination and side, over their totals. `None` for no orders or
/// when the quoted amount overflows. The other leg is a limit the batch
/// never checks, its orders are checked one by one, so it saturates: an
/// open buy limit is `u64::MAX` to sell.
fn batch_order(orders: &[&auction::Order]) -> Option<dex::Order> {
    let (first, rest) = orders.split_first()?;
    let mut batch = first.to_dex_order();
    for order in rest {
        let (quoted, limit, quoted_add, limit_add) = match batch.side {
            dex::Side::Sell => (
                &mut batch.sell_amount,
                &mut batch.buy_amount,
                order.sell_amount,
                order.buy_amount,
            ),
            dex::Side::Buy => (
                &mut batch.buy_amount,
                &mut batch.sell_amount,
                order.buy_amount,
                order.sell_amount,
            ),
        };
        *quoted = quoted.checked_add(quoted_add)?;
        *limit = limit.saturating_add(limit_add);
    }
    Some(batch)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::dto::order::OrderUid,
        std::{
            sync::{
                Mutex,
                atomic::{AtomicUsize, Ordering},
            },
            time::{Duration, Instant},
        },
    };

    /// Sell mint bytes that select the mock's answer.
    const NO_ROUTE: u8 = 0xff;
    const RATE_LIMITED: u8 = 0xfe;
    const SLOW: u8 = 0xfd;
    const IMPACT: u8 = 0xfc;
    const ROUTES_BUYS: u8 = 0xfb;

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    /// Every mint of `orders` priced at the denominator, as by an autopilot
    /// without price sources.
    fn auction(orders: Vec<auction::Order>, deadline_in: Duration) -> Auction {
        let prices: Vec<_> = orders
            .iter()
            .flat_map(|order| [order.sell_mint, order.buy_mint])
            .map(|mint| (mint, 1_000_000_000))
            .collect();
        Auction {
            id: Some(1),
            taker: pubkey(1),
            orders,
            deadline: chrono::Utc::now() + deadline_in,
            tokens: tokens(&prices),
        }
    }

    /// Native prices of 10^9 for the buy mint, `pubkey(2)`, and the given
    /// ones, and none for any other mint.
    fn priced(mut auction: Auction, sell_prices: &[(Pubkey, u64)]) -> Auction {
        let prices = std::iter::once((pubkey(2), 1_000_000_000)).chain(sell_prices.iter().copied());
        auction.tokens = tokens(&prices.collect::<Vec<_>>());
        auction
    }

    fn sell_of(uid: u8, sell_mint: Pubkey, sell_amount: u64, buy_amount: u64) -> auction::Order {
        let mut order = order(uid, dex::Side::Sell, sell_mint);
        order.sell_amount = sell_amount;
        order.amount = sell_amount;
        order.buy_amount = buy_amount;
        order
    }

    fn order(uid: u8, side: dex::Side, sell_mint: Pubkey) -> auction::Order {
        auction::Order {
            uid: OrderUid([uid; 32]),
            sell_mint,
            buy_mint: pubkey(2),
            buy_destination: pubkey(3),
            sell_amount: 1_000,
            buy_amount: 0,
            amount: match side {
                dex::Side::Sell => 1_000,
                dex::Side::Buy => 0,
            },
            full_sell_amount: 1_000,
            full_buy_amount: 0,
            side,
            partially_fillable: false,
            missing_buy_token_account: false,
        }
    }

    fn sell(uid: u8) -> auction::Order {
        order(uid, dex::Side::Sell, pubkey(0x10))
    }

    /// A solver without batching.
    fn solver(concurrent_requests: usize, min_back_off: Duration) -> Solver<MockAggregator> {
        Solver::new(
            MockAggregator::default(),
            NonZeroUsize::new(concurrent_requests).unwrap(),
            Strategy::try_new(2.0, min_back_off, min_back_off * 8).unwrap(),
            NonZeroUsize::new(1).unwrap(),
        )
    }

    /// A solver batching up to `max_batch_size` orders, without back-off.
    fn batching(concurrent_requests: usize, max_batch_size: usize) -> Solver<MockAggregator> {
        Solver::new(
            MockAggregator::default(),
            NonZeroUsize::new(concurrent_requests).unwrap(),
            Strategy::try_new(2.0, Duration::ZERO, Duration::ZERO).unwrap(),
            NonZeroUsize::new(max_batch_size).unwrap(),
        )
    }

    /// Routes any sell after a millisecond at two output units per input
    /// unit, and buys on the `ROUTES_BUYS` mint at the same rate, except the
    /// `NO_ROUTE` mint (no route), the `RATE_LIMITED` mint (429), the
    /// `SLOW` mint (ten seconds) and the `IMPACT` mint, whose rate worsens
    /// with size: `in + 1000` out. Rejects other buys. Records when
    /// each quote started, the amounts quoted in that order, the most that
    /// ran at once, and the input of every swap built.
    #[derive(Default)]
    struct MockAggregator {
        started: Mutex<Vec<Instant>>,
        amounts: Mutex<Vec<u64>>,
        swaps: Mutex<Vec<u64>>,
        in_flight: AtomicUsize,
        max_in_flight: AtomicUsize,
    }

    fn quote_of(in_amount: u64, out_amount: u64) -> dex::Quote {
        dex::Quote {
            in_amount,
            out_amount,
            destination: pubkey(3),
            raw: serde_json::Value::Null,
        }
    }

    impl Aggregator for MockAggregator {
        fn swap(
            &self,
            quote: &dex::Quote,
            _taker: &Pubkey,
        ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send {
            self.swaps.lock().unwrap().push(quote.in_amount);
            let swap = dex::Swap {
                in_amount: quote.in_amount,
                out_amount: quote.out_amount,
                instructions: vec![],
                address_lookup_tables: vec![],
            };
            async move { Ok(swap) }
        }

        fn quote(
            &self,
            order: &dex::Order,
        ) -> impl Future<Output = Result<dex::Quote, dex::jupiter::Error>> + Send {
            let (side, sell_mint, amount) = (order.side, order.sell_mint, order.amount());
            // Nothing runs before the first poll: a quote still held back by
            // the back-off is not recorded.
            async move {
                self.started.lock().unwrap().push(Instant::now());
                self.amounts.lock().unwrap().push(amount);
                let running = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_in_flight.fetch_max(running, Ordering::SeqCst);
                let result = match side {
                    dex::Side::Buy if sell_mint == pubkey(ROUTES_BUYS) => {
                        Ok(quote_of(amount / 2, amount))
                    }
                    dex::Side::Buy => Err(dex::jupiter::Error::OrderNotSupported),
                    dex::Side::Sell if sell_mint == pubkey(NO_ROUTE) => {
                        Err(dex::jupiter::Error::NotFound)
                    }
                    dex::Side::Sell if sell_mint == pubkey(RATE_LIMITED) => {
                        Err(dex::jupiter::Error::RateLimited)
                    }
                    dex::Side::Sell => {
                        let delay = if sell_mint == pubkey(SLOW) {
                            Duration::from_secs(10)
                        } else {
                            Duration::from_millis(1)
                        };
                        tokio::time::sleep(delay).await;
                        let out_amount = if sell_mint == pubkey(IMPACT) {
                            amount + 1_000
                        } else {
                            amount.saturating_mul(2)
                        };
                        Ok(quote_of(amount, out_amount))
                    }
                };
                self.in_flight.fetch_sub(1, Ordering::SeqCst);
                result
            }
        }
    }

    #[tokio::test]
    async fn emits_one_solution_per_routable_order() {
        let auction = auction(
            vec![
                sell(0x01),
                order(0x02, dex::Side::Sell, pubkey(NO_ROUTE)),
                order(0x03, dex::Side::Buy, pubkey(0x11)),
            ],
            Duration::from_secs(60),
        );

        let solutions = solver(8, Duration::ZERO).solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].trades[0].order_uid, OrderUid([0x01; 32]));
    }

    /// A 2000 buy tightened by a 500 bps solver fee is 2106; the mock route
    /// fills 2000, so that order yields nothing while the untightened one
    /// still solves.
    #[tokio::test]
    async fn drops_a_swap_that_undercuts_the_limit() {
        let mut tightened = sell(0x01);
        tightened.buy_amount = 2_106;
        let mut at_limit = order(0x02, dex::Side::Sell, pubkey(0x11));
        at_limit.buy_amount = 2_000;
        let auction = auction(vec![tightened, at_limit], Duration::from_secs(60));

        let solutions = solver(8, Duration::ZERO).solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].trades[0].order_uid, OrderUid([0x02; 32]));
    }

    #[tokio::test]
    async fn empty_auction_yields_no_solutions() {
        let auction = auction(vec![], Duration::from_secs(60));
        assert!(solver(8, Duration::ZERO).solve(&auction).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn quotes_at_most_concurrent_requests_orders_at_once() {
        let auction = auction(
            (1..=6)
                .map(|uid| order(uid, dex::Side::Sell, pubkey(0x20 + uid)))
                .collect(),
            Duration::from_secs(60),
        );
        let solver = solver(2, Duration::ZERO);

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 6);
        assert_eq!(solver.quoter.max_in_flight.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn returns_the_solutions_found_by_the_deadline() {
        let auction = auction(
            vec![
                sell(0x01),
                order(0x02, dex::Side::Sell, pubkey(SLOW)),
                order(0x03, dex::Side::Sell, pubkey(0x11)),
            ],
            Duration::from_secs(1),
        );

        let mut solutions = solver(3, Duration::ZERO).solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(solutions.len(), 2);
        assert_eq!(solutions[0].trades[0].order_uid, OrderUid([0x01; 32]));
        assert_eq!(solutions[1].trades[0].order_uid, OrderUid([0x03; 32]));
    }

    /// Real time: the limiter measures the back-off on the system clock.
    #[tokio::test]
    async fn waits_out_the_back_off_before_the_next_quote() {
        let auction = auction(
            vec![
                order(0x01, dex::Side::Sell, pubkey(RATE_LIMITED)),
                sell(0x02),
            ],
            Duration::from_secs(60),
        );
        let solver = solver(1, Duration::from_millis(50));

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        let started = solver.quoter.started.lock().unwrap();
        assert_eq!(started.len(), 2);
        assert!(started[1] - started[0] >= Duration::from_millis(50));
    }

    #[tokio::test(start_paused = true)]
    async fn the_deadline_cuts_a_back_off_short() {
        let auction = auction(
            vec![
                order(0x01, dex::Side::Sell, pubkey(RATE_LIMITED)),
                sell(0x02),
            ],
            Duration::from_secs(1),
        );
        let solver = solver(1, Duration::from_secs(10));

        let solutions = solver.solve(&auction).await;

        assert!(solutions.is_empty());
        assert_eq!(solver.quoter.started.lock().unwrap().len(), 1);
    }

    fn ids(solutions: &[Solution]) -> Vec<u64> {
        solutions.iter().map(|solution| solution.id).collect()
    }

    fn trades(solution: &Solution) -> Vec<(OrderUid, u64)> {
        solution
            .trades
            .iter()
            .map(|trade| (trade.order_uid, trade.executed_amount))
            .collect()
    }

    fn sell_with_limit(uid: u8, sell_mint: Pubkey, buy_amount: u64) -> auction::Order {
        let mut order = order(uid, dex::Side::Sell, sell_mint);
        order.buy_amount = buy_amount;
        order
    }

    /// Four sells on one pair, limits 1.5, 0, 1.8 and 1.0, in batches of
    /// three: the three most permissive as one quote over their total, most
    /// permissive first, then the fourth on its own.
    #[tokio::test]
    async fn batches_a_pair_most_permissive_first() {
        let auction = auction(
            vec![
                sell_with_limit(0x01, pubkey(0x10), 1_500),
                sell_with_limit(0x02, pubkey(0x10), 0),
                sell_with_limit(0x03, pubkey(0x10), 1_800),
                sell_with_limit(0x04, pubkey(0x10), 1_000),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 3);

        let mut solutions = solver.solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(ids(&solutions), vec![0, 1]);
        let batch = &solutions[0];
        assert_eq!(
            trades(batch),
            vec![
                (OrderUid([2; 32]), 1_000),
                (OrderUid([4; 32]), 1_000),
                (OrderUid([1; 32]), 1_000),
            ]
        );
        assert_eq!(batch.prices[&pubkey(0x10)], 6_000);
        assert_eq!(batch.prices[&pubkey(2)], 3_000);
        assert_eq!(trades(&solutions[1]), vec![(OrderUid([3; 32]), 1_000)]);
        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![3_000, 1_000]);
    }

    /// Three sells on one pair and one on another, in batches of two: the
    /// second pair's only batch is quoted before the first pair's second.
    #[tokio::test]
    async fn quotes_every_pairs_first_batch_before_any_second() {
        let mut other_pair = order(0x04, dex::Side::Sell, pubkey(0x11));
        other_pair.sell_amount = 500;
        other_pair.amount = 500;
        let auction = auction(
            vec![sell(0x01), sell(0x02), sell(0x03), other_pair],
            Duration::from_secs(60),
        );
        let solver = batching(1, 2);

        let mut solutions = solver.solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(ids(&solutions), vec![0, 1, 2]);
        assert_eq!(
            trades(&solutions[0]),
            vec![(OrderUid([1; 32]), 1_000), (OrderUid([2; 32]), 1_000)]
        );
        assert_eq!(trades(&solutions[1]), vec![(OrderUid([4; 32]), 500)]);
        assert_eq!(trades(&solutions[2]), vec![(OrderUid([3; 32]), 1_000)]);
        assert_eq!(
            *solver.quoter.amounts.lock().unwrap(),
            vec![2_000, 500, 1_000]
        );
    }

    /// Three orders of 1000 on the `IMPACT` mint, limits 0, 0 and 1.6: the
    /// batch of three rates 4000/3000, under the third order's limit, the
    /// batch of the other two rates 3000/2000 and clears.
    #[tokio::test]
    async fn trims_the_batch_to_the_orders_its_rate_clears() {
        let auction = auction(
            vec![
                sell_with_limit(0x03, pubkey(IMPACT), 1_600),
                sell_with_limit(0x01, pubkey(IMPACT), 0),
                sell_with_limit(0x02, pubkey(IMPACT), 0),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        let batch = &solutions[0];
        assert_eq!(
            trades(batch),
            vec![(OrderUid([1; 32]), 1_000), (OrderUid([2; 32]), 1_000)]
        );
        assert_eq!(batch.prices[&pubkey(IMPACT)], 3_000);
        assert_eq!(batch.prices[&pubkey(2)], 2_000);
        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![3_000, 2_000]);
    }

    /// Four orders of 1000 on the `IMPACT` mint, with limits 1.2 through
    /// 1.5: the batch of four rates 5000/4000, so only the first clears at
    /// that size. Dropping one tail order per round improves the rate until
    /// the two most permissive orders clear together at 3000/2000. Dropping
    /// every order that misses the first quote would incorrectly keep only
    /// the first one.
    #[tokio::test]
    async fn drops_only_the_least_permissive_order_before_requoting() {
        let auction = auction(
            vec![
                sell_with_limit(0x03, pubkey(IMPACT), 1_400),
                sell_with_limit(0x01, pubkey(IMPACT), 1_200),
                sell_with_limit(0x04, pubkey(IMPACT), 1_500),
                sell_with_limit(0x02, pubkey(IMPACT), 1_300),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        let batch = &solutions[0];
        assert_eq!(
            trades(batch),
            vec![(OrderUid([1; 32]), 1_000), (OrderUid([2; 32]), 1_000)]
        );
        assert_eq!(batch.prices[&pubkey(IMPACT)], 3_000);
        assert_eq!(batch.prices[&pubkey(2)], 2_000);
        assert_eq!(
            *solver.quoter.amounts.lock().unwrap(),
            vec![4_000, 3_000, 2_000]
        );
        // Only the batch that clears gets its swap built.
        assert_eq!(*solver.quoter.swaps.lock().unwrap(), vec![2_000]);
    }

    /// Limits 0 and 1.6 on the `IMPACT` mint: the batch of two rates
    /// 3000/2000, under the second order's limit, so the first is requoted
    /// alone and becomes a single-order candidate.
    #[tokio::test]
    async fn trims_a_batch_down_to_a_single_order() {
        let auction = auction(
            vec![
                sell_with_limit(0x02, pubkey(IMPACT), 1_600),
                sell_with_limit(0x01, pubkey(IMPACT), 0),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        assert_eq!(trades(&solutions[0]), vec![(OrderUid([1; 32]), 1_000)]);
        assert_eq!(solutions[0].prices[&pubkey(IMPACT)], 2_000);
        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![2_000, 1_000]);
    }

    /// Limits 2.5 and 2.6 on the `IMPACT` mint: neither the batch's 3000/2000
    /// nor the most permissive order's 2000/1000 rate clears, so the pair
    /// yields nothing after quoting down to one order.
    #[tokio::test]
    async fn a_swap_undercutting_every_order_yields_nothing() {
        let auction = auction(
            vec![
                sell_with_limit(0x01, pubkey(IMPACT), 2_500),
                sell_with_limit(0x02, pubkey(IMPACT), 2_600),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        assert!(solver.solve(&auction).await.is_empty());
        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![2_000, 1_000]);
        assert!(solver.quoter.swaps.lock().unwrap().is_empty());
    }

    /// One batch per side, pair and swap destination, numbered in order of
    /// first appearance.
    #[tokio::test]
    async fn quotes_one_batch_per_side_pair_and_destination() {
        let mut other_destination = sell(0x04);
        other_destination.buy_destination = pubkey(5);
        let auction = auction(
            vec![
                sell(0x01),
                order(0x02, dex::Side::Sell, pubkey(0x11)),
                order(0x03, dex::Side::Buy, pubkey(0x10)),
                other_destination,
                sell(0x05),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let mut solutions = solver.solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        // Group 2, the buy, is rejected by the mock.
        assert_eq!(ids(&solutions), vec![0, 1, 3]);
        assert_eq!(
            trades(&solutions[0]),
            vec![(OrderUid([1; 32]), 1_000), (OrderUid([5; 32]), 1_000)]
        );
        assert_eq!(trades(&solutions[1]), vec![(OrderUid([2; 32]), 1_000)]);
        assert_eq!(trades(&solutions[2]), vec![(OrderUid([4; 32]), 1_000)]);
        assert_eq!(solver.quoter.amounts.lock().unwrap().len(), 4);
    }

    fn buy(uid: u8, buy_amount: u64, sell_limit: u64) -> auction::Order {
        let mut buy = order(uid, dex::Side::Buy, pubkey(ROUTES_BUYS));
        buy.buy_amount = buy_amount;
        buy.sell_amount = sell_limit;
        buy.amount = buy_amount;
        buy
    }

    /// Two buys of 2000 on one pair: one `ExactOut` quote of 4000 for 2000,
    /// cleared at 2001 for 4000 so the rounded-down pulls cover the input.
    #[tokio::test]
    async fn batches_the_buy_orders_of_a_pair() {
        let auction = auction(
            vec![buy(0x01, 2_000, u64::MAX), buy(0x02, 2_000, u64::MAX)],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        let batch = &solutions[0];
        assert_eq!(
            trades(batch),
            vec![(OrderUid([1; 32]), 2_000), (OrderUid([2; 32]), 2_000)]
        );
        assert_eq!(batch.prices[&pubkey(ROUTES_BUYS)], 4_000);
        assert_eq!(batch.prices[&pubkey(2)], 2_001);
        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![4_000]);
    }

    /// A buy whose limit is exactly the swap's rate, 1000 for 2000, misses
    /// the batch's raised rate, 2001 for 4000, so it is trimmed and the
    /// other buy is requoted alone at the swap's own rate.
    #[tokio::test]
    async fn checks_buy_limits_at_the_raised_clearing_rate() {
        let auction = auction(
            vec![buy(0x02, 2_000, 1_000), buy(0x01, 2_000, 1_500)],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let solutions = solver.solve(&auction).await;

        assert_eq!(solutions.len(), 1);
        assert_eq!(trades(&solutions[0]), vec![(OrderUid([1; 32]), 2_000)]);
        assert_eq!(solutions[0].prices[&pubkey(2)], 1_000);
        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![4_000, 2_000]);
    }

    /// A buy and a sell on one directed pair are quoted apart. The sell,
    /// accepting anything, ranks ahead of the buy's finite limit.
    #[tokio::test]
    async fn keeps_buys_and_sells_of_a_pair_apart() {
        let mut sell = sell(0x02);
        sell.sell_mint = pubkey(ROUTES_BUYS);
        let auction = auction(
            vec![buy(0x01, 2_000, u64::MAX), sell],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        let mut solutions = solver.solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(ids(&solutions), vec![0, 1]);
        assert_eq!(trades(&solutions[0]), vec![(OrderUid([2; 32]), 1_000)]);
        assert_eq!(trades(&solutions[1]), vec![(OrderUid([1; 32]), 2_000)]);
    }

    /// With a batch size of one, every order of a pair is quoted on its own,
    /// most permissive first.
    #[tokio::test]
    async fn a_max_batch_size_of_one_quotes_every_order_on_its_own() {
        let auction = auction(
            vec![
                sell_with_limit(0x01, pubkey(0x10), 1_500),
                sell_with_limit(0x02, pubkey(0x10), 1_000),
                sell_with_limit(0x03, pubkey(0x10), 1_800),
            ],
            Duration::from_secs(60),
        );
        let solver = solver(1, Duration::ZERO);

        let mut solutions = solver.solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(ids(&solutions), vec![0, 1, 2]);
        assert_eq!(trades(&solutions[0]), vec![(OrderUid([2; 32]), 1_000)]);
        assert_eq!(trades(&solutions[1]), vec![(OrderUid([1; 32]), 1_000)]);
        assert_eq!(trades(&solutions[2]), vec![(OrderUid([3; 32]), 1_000)]);
        assert_eq!(
            *solver.quoter.amounts.lock().unwrap(),
            vec![1_000, 1_000, 1_000]
        );
    }

    /// Two sells of `u64::MAX` cannot be quoted together: the second starts
    /// a new batch.
    #[tokio::test]
    async fn an_order_overflowing_the_total_starts_a_new_batch() {
        let mut orders = vec![sell(0x01), sell(0x02)];
        for order in &mut orders {
            order.sell_amount = u64::MAX;
            order.amount = u64::MAX;
        }
        let auction = auction(orders, Duration::from_secs(60));
        let solver = batching(1, 4);

        let mut solutions = solver.solve(&auction).await;

        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(ids(&solutions), vec![0, 1]);
        assert_eq!(trades(&solutions[0]), vec![(OrderUid([1; 32]), u64::MAX)]);
        assert_eq!(trades(&solutions[1]), vec![(OrderUid([2; 32]), u64::MAX)]);
        assert_eq!(
            *solver.quoter.amounts.lock().unwrap(),
            vec![u64::MAX, u64::MAX]
        );
    }

    #[test]
    fn permissiveness_ranks_by_limit_then_uid_with_nothing_to_sell_last() {
        let mut empty = sell_with_limit(0x01, pubkey(0x10), 0);
        empty.sell_amount = 0;
        let mut orders = [
            sell_with_limit(0x05, pubkey(0x10), 2_000),
            empty,
            sell_with_limit(0x04, pubkey(0x10), 1_000),
            sell_with_limit(0x03, pubkey(0x10), 1_000),
            sell_with_limit(0x02, pubkey(0x10), 0),
        ];

        orders.sort_by(by_permissiveness);

        assert_eq!(
            orders
                .iter()
                .map(|order| order.uid.0[0])
                .collect::<Vec<_>>(),
            vec![0x02, 0x03, 0x04, 0x05, 0x01]
        );
    }

    /// Sell mints at twice the buy mint's native price, so the native rate
    /// is 2. A small order whose limit asks 1.5 goes before a large one
    /// asking 1.67, size does not matter; a limit of 4 the native rate
    /// misses comes after both; a sell mint without a price goes last; a
    /// buy mint without a price is never quoted. One quote at a time, so the
    /// quoted amounts show the order.
    #[tokio::test]
    async fn quotes_the_most_permissive_pairs_first() {
        let mut unpriced_buy = sell_of(0x05, pubkey(0x20), 9_000, 0);
        unpriced_buy.buy_mint = pubkey(0x30);
        let auction = priced(
            auction(
                vec![
                    sell_of(0x01, pubkey(0x22), 5_000, 20_000),
                    sell_of(0x02, pubkey(0x21), 3_000, 5_000),
                    unpriced_buy,
                    sell_of(0x03, pubkey(0x23), 7_000, 0),
                    sell_of(0x04, pubkey(0x20), 1_000, 1_500),
                ],
                Duration::from_secs(60),
            ),
            &[
                (pubkey(0x20), 2_000_000_000),
                (pubkey(0x21), 2_000_000_000),
                (pubkey(0x22), 2_000_000_000),
            ],
        );
        let solver = batching(1, 4);

        let mut solutions = solver.solve(&auction).await;

        assert_eq!(
            *solver.quoter.amounts.lock().unwrap(),
            vec![1_000, 3_000, 5_000, 7_000]
        );
        // The solution ids follow the quoting order; the out-of-market
        // pair yields none.
        solutions.sort_by_key(|solution| solution.id);
        assert_eq!(ids(&solutions), vec![0, 1, 3]);
        assert_eq!(trades(&solutions[0]), vec![(OrderUid([4; 32]), 1_000)]);
    }

    /// Pairs that rank the same are quoted in auction order.
    #[tokio::test]
    async fn quotes_tied_pairs_in_auction_order() {
        let auction = auction(
            vec![
                sell_of(0x01, pubkey(0x20), 1_000, 0),
                sell_of(0x02, pubkey(0x21), 3_000, 0),
            ],
            Duration::from_secs(60),
        );
        let solver = batching(1, 4);

        solver.solve(&auction).await;

        assert_eq!(*solver.quoter.amounts.lock().unwrap(), vec![1_000, 3_000]);
    }

    fn tokens(prices: &[(Pubkey, u64)]) -> HashMap<Pubkey, auction::Token> {
        prices
            .iter()
            .map(|&(mint, reference_price)| (mint, auction::Token { reference_price }))
            .collect()
    }

    fn priced_with(prices: &[(Pubkey, u64)]) -> Auction {
        let mut auction = auction(vec![], Duration::from_secs(60));
        auction.tokens = tokens(prices);
        auction
    }

    /// A buy willing to sell 1000 at 2 for 1000 at 1 sits at 2, a sell
    /// giving up 1000 at 1 for at least 250 at 1 at 4: the sell ranks first,
    /// whatever the side.
    #[test]
    fn limit_to_native_ranks_sells_and_buys_alike() {
        let mut buy = order(0x01, dex::Side::Buy, pubkey(0x20));
        buy.sell_amount = 1_000;
        buy.buy_amount = 1_000;
        let sell = sell_of(0x02, pubkey(0x21), 1_000, 250);
        let auction = priced_with(&[(pubkey(0x20), 2), (pubkey(0x21), 1), (pubkey(2), 1)]);

        assert_eq!(limit_to_native(&buy, &auction), Some(2.));
        assert_eq!(limit_to_native(&sell, &auction), Some(4.));
    }

    #[test]
    fn limit_to_native_handles_open_and_empty_legs() {
        let auction = priced_with(&[(pubkey(0x20), 2), (pubkey(2), 1)]);
        let market = sell_of(0x01, pubkey(0x20), 1_000, 0);
        let empty = sell_of(0x02, pubkey(0x20), 0, 0);
        let mut open_buy = order(0x03, dex::Side::Buy, pubkey(0x20));
        open_buy.sell_amount = u64::MAX;
        open_buy.buy_amount = 1_000;

        assert_eq!(limit_to_native(&market, &auction), Some(f64::INFINITY));
        assert_eq!(limit_to_native(&empty, &auction), Some(0.));
        assert!(limit_to_native(&open_buy, &auction).unwrap() > 1e15);
    }

    #[test]
    fn limit_to_native_needs_both_prices() {
        let order = sell_of(0x01, pubkey(0x20), 1_000, 0);

        assert_eq!(
            limit_to_native(&order, &priced_with(&[(pubkey(2), 1)])),
            None
        );
        assert_eq!(
            limit_to_native(&order, &priced_with(&[(pubkey(0x20), 2)])),
            None
        );
    }

    /// A zero price on the buy mint counts as none, so the pair is skipped
    /// rather than ranked.
    #[test]
    fn skips_a_pair_whose_buy_mint_prices_at_zero() {
        let mut auction = priced_with(&[(pubkey(0x20), 2), (pubkey(2), 0)]);
        auction.orders = vec![sell_of(0x01, pubkey(0x20), 1_000, 0)];

        assert!(batching(1, 4).batches(&auction).is_empty());
    }
}
