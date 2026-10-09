//! Solve loop: quote each auction order and assemble single-order solutions.

use {
    crate::{
        dex::{self, Dex},
        dto::{auction, auction::Auction, solution::Solution},
    },
    configs::rate_limit::Strategy,
    futures::{Stream, StreamExt, future, stream},
    rate_limit::RateLimiter,
    solana_sdk::pubkey::Pubkey,
    std::{future::Future, num::NonZeroUsize},
    tracing::Instrument,
};

/// Quotes one order into a swap. A seam over [`Dex`] so the loop is testable
/// without the network.
pub trait Quote {
    fn quote(
        &self,
        order: &dex::Order,
        taker: &Pubkey,
    ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send;
}

impl Quote for Dex {
    fn quote(
        &self,
        order: &dex::Order,
        taker: &Pubkey,
    ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send {
        self.swap(order, taker)
    }
}

/// Quotes auction orders into single-order solutions: at most
/// `concurrent_requests` orders at a time, pausing after a rate-limited
/// response, until the auction deadline.
pub struct Solver<Q> {
    quoter: Q,
    concurrent_requests: NonZeroUsize,
    rate_limiter: RateLimiter,
}

impl<Q: Quote + Sync> Solver<Q> {
    pub fn new(quoter: Q, concurrent_requests: NonZeroUsize, rate_limiting: Strategy) -> Self {
        Self {
            quoter,
            concurrent_requests,
            rate_limiter: RateLimiter::from_strategy(rate_limiting, "dex_api".to_string()),
        }
    }

    /// One single-order solution per routable order quoted before the
    /// deadline. Buys (when disabled), orders the aggregator cannot route,
    /// and swaps that undercut the order's limit yield no candidate. The rest
    /// of the auction still proceeds.
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
        stream::iter(auction.orders.iter().enumerate())
            .map(|(index, order)| {
                self.solve_order(index, order, auction)
                    .instrument(tracing::info_span!(
                        "solve",
                        auction_id = ?auction.id,
                        order = %order.uid
                    ))
            })
            .buffer_unordered(self.concurrent_requests.get())
            .filter_map(future::ready)
    }

    async fn solve_order(
        &self,
        index: usize,
        order: &auction::Order,
        auction: &Auction,
    ) -> Option<Solution> {
        let dex_order = order.to_dex_order();
        let swap = self
            .rate_limiter
            .execute_with_back_off(self.quoter.quote(&dex_order, &auction.taker), |result| {
                matches!(result, Err(dex::jupiter::Error::RateLimited))
            })
            .await
            // A quote dropped during a back-off counts as rate limited.
            .unwrap_or(Err(dex::jupiter::Error::RateLimited))
            .inspect_err(|err| match err {
                dex::jupiter::Error::NotFound | dex::jupiter::Error::OrderNotSupported => {
                    tracing::debug!("no solution for swap")
                }
                dex::jupiter::Error::RateLimited => tracing::debug!("rate limited"),
                _ => tracing::warn!(%err, "quote failed"),
            })
            .ok()?;
        if !swap.satisfies(&dex_order) {
            tracing::debug!(
                in_amount = swap.in_amount,
                out_amount = swap.out_amount,
                limit_sell = dex_order.sell_amount,
                limit_buy = dex_order.buy_amount,
                shortfall = %swap.shortfall(&dex_order),
                "swap does not satisfy order"
            );
            return None;
        }
        let solution = Solution::new(index as u64, order.uid, &dex_order, swap).ok()?;
        tracing::debug!("solved");
        Some(solution)
    }
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

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn auction(orders: Vec<auction::Order>, deadline_in: Duration) -> Auction {
        Auction {
            id: Some(1),
            taker: pubkey(1),
            orders,
            deadline: chrono::Utc::now() + deadline_in,
        }
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

    fn solver(concurrent_requests: usize, min_back_off: Duration) -> Solver<MockQuote> {
        Solver::new(
            MockQuote::default(),
            NonZeroUsize::new(concurrent_requests).unwrap(),
            Strategy::try_new(2.0, min_back_off, min_back_off * 8).unwrap(),
        )
    }

    /// Routes any sell after a millisecond, except the `NO_ROUTE` mint (no
    /// route), the `RATE_LIMITED` mint (429) and the `SLOW` mint (ten
    /// seconds). Rejects buys. Records when each quote started and the most
    /// that ran at once.
    #[derive(Default)]
    struct MockQuote {
        started: Mutex<Vec<Instant>>,
        in_flight: AtomicUsize,
        max_in_flight: AtomicUsize,
    }

    impl Quote for MockQuote {
        fn quote(
            &self,
            order: &dex::Order,
            _taker: &Pubkey,
        ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send {
            let (side, sell_mint) = (order.side, order.sell_mint);
            // Nothing runs before the first poll: a quote still held back by
            // the back-off is not recorded.
            async move {
                self.started.lock().unwrap().push(Instant::now());
                let running = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_in_flight.fetch_max(running, Ordering::SeqCst);
                let result = match side {
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
                        Ok(dex::Swap {
                            in_amount: 1_000,
                            out_amount: 2_000,
                            instructions: vec![],
                            address_lookup_tables: vec![],
                        })
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
        let mut at_limit = sell(0x02);
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
        let auction = auction((1..=6).map(sell).collect(), Duration::from_secs(60));
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
                sell(0x03),
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
}
