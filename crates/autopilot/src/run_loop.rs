use {
    crate::{
        database::competition::Competition,
        domain::{self, auction::order::Class, OrderUid},
        infra::{
            self,
            solvers::dto::{reveal, settle, solve},
        },
        run::Liveness,
        solvable_orders::SolvableOrdersCache,
    },
    ::observe::metrics,
    anyhow::Result,
    database::order_events::OrderEventLabel,
    model::solver_competition::{
        CompetitionAuction,
        Order,
        Score,
        SolverCompetitionDB,
        SolverSettlement,
    },
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, H256, U256},
    rand::seq::SliceRandom,
    shared::{event_handling::MAX_REORG_BLOCK_COUNT, token_list::AutoUpdatingTokenList},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::Mutex,
    tracing::Instrument,
    web3::types::TransactionReceipt,
};

pub struct RunLoop {
    pub eth: infra::Ethereum,
    pub persistence: infra::Persistence,
    pub drivers: Vec<infra::Driver>,

    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub market_makable_token_list: AutoUpdatingTokenList,
    pub submission_deadline: u64,
    pub additional_deadline_for_rewards: u64,
    pub max_settlement_transaction_wait: Duration,
    pub solve_deadline: Duration,
    pub in_flight_orders: Arc<Mutex<Option<InFlightOrders>>>,
    pub liveness: Arc<Liveness>,
}

impl RunLoop {
    pub async fn run_forever(self) -> ! {
        let mut last_auction = None;
        let mut last_block = None;
        loop {
            if let Some(domain::AuctionWithId { id, auction }) = self.next_auction().await {
                let current_block = self.eth.current_block().borrow().hash;
                // Only run the solvers if the auction or block has changed.
                let previous = last_auction.replace(auction.clone());
                if previous.as_ref() != Some(&auction)
                    || last_block.replace(current_block) != Some(current_block)
                {
                    observe::log_auction_delta(id, &previous, &auction);
                    self.liveness.auction();

                    self.single_run(id, &auction)
                        .instrument(tracing::info_span!("auction", id))
                        .await;
                }
            };
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn next_auction(&self) -> Option<domain::AuctionWithId> {
        let auction = match self.solvable_orders_cache.current_auction() {
            Some(auction) => auction,
            None => {
                tracing::debug!("no current auction");
                return None;
            }
        };

        let id = match self.persistence.replace_current_auction(&auction).await {
            Ok(id) => {
                Metrics::auction(id);
                id
            }
            Err(err) => {
                tracing::error!(?err, "failed to replace current auction");
                return None;
            }
        };

        if auction.orders.iter().all(|order| match order.class {
            Class::Market => false,
            Class::Liquidity => true,
            Class::Limit => false,
        }) {
            // Updating liveness probe to not report unhealthy due to this optimization
            self.liveness.auction();
            tracing::debug!("skipping empty auction");
            return None;
        }

        Some(domain::AuctionWithId { id, auction })
    }

    async fn single_run(&self, auction_id: domain::auction::Id, auction: &domain::Auction) {
        tracing::info!(?auction_id, "solving");

        let auction = self.remove_in_flight_orders(auction.clone()).await;

        let solutions = {
            let mut solutions = self.competition(auction_id, &auction).await;
            if solutions.is_empty() {
                tracing::info!("no solutions for auction");
                return;
            }

            // Shuffle so that sorting randomly splits ties.
            solutions.shuffle(&mut rand::thread_rng());
            solutions.sort_unstable_by_key(|participant| participant.solution.score);
            solutions
        };
        let competition_simulation_block = self.eth.current_block().borrow().number;

        // TODO: Keep going with other solutions until some deadline.
        if let Some(Participant { driver, solution }) = solutions.last() {
            tracing::info!(driver = %driver.name, solution = %solution.id, "winner");

            let revealed = match self.reveal(driver, auction_id, solution.id).await {
                Ok(result) => {
                    Metrics::reveal_ok(driver);
                    result
                }
                Err(err) => {
                    Metrics::reveal_err(driver, &err);
                    tracing::warn!(driver = %driver.name, ?err, "failed to reveal winning solution");
                    return;
                }
            };

            let order_uids = solution.order_ids().copied().collect();
            self.persistence
                .store_order_events(order_uids, OrderEventLabel::Considered);

            let winner = solution.account;
            let winning_score = solution.score.get();
            let reference_score = solutions
                .iter()
                .nth_back(1)
                .map(|participant| participant.solution.score.get())
                .unwrap_or_default();
            let participants = solutions
                .iter()
                .map(|participant| participant.solution.account)
                .collect::<HashSet<_>>();

            let mut prices = BTreeMap::new();
            let mut fee_policies = Vec::new();
            let block_deadline = competition_simulation_block
                + self.submission_deadline
                + self.additional_deadline_for_rewards;
            let call_data = revealed.calldata.internalized.clone();
            let uninternalized_call_data = revealed.calldata.uninternalized.clone();

            for order_id in solution.order_ids() {
                let auction_order = auction
                    .orders
                    .iter()
                    .find(|auction_order| &auction_order.uid == order_id);
                match auction_order {
                    Some(auction_order) => {
                        fee_policies.push((auction_order.uid, auction_order.protocol_fees.clone()));
                        if let Some(price) = auction.prices.get(&auction_order.sell_token) {
                            prices.insert(auction_order.sell_token, *price);
                        } else {
                            tracing::error!(
                                sell_token = ?auction_order.sell_token,
                                "sell token price is missing in auction"
                            );
                        }
                        if let Some(price) = auction.prices.get(&auction_order.buy_token) {
                            prices.insert(auction_order.buy_token, *price);
                        } else {
                            tracing::error!(
                                buy_token = ?auction_order.buy_token,
                                "buy token price is missing in auction"
                            );
                        }
                    }
                    None => {
                        tracing::debug!(?order_id, "order not found in auction");
                    }
                }
            }

            let competition_table = SolverCompetitionDB {
                auction_start_block: auction.block,
                competition_simulation_block,
                auction: CompetitionAuction {
                    orders: auction
                        .orders
                        .iter()
                        .map(|order| order.uid.into())
                        .collect(),
                    prices: auction.prices.clone(),
                },
                solutions: solutions
                    .iter()
                    .enumerate()
                    .map(|(index, participant)| {
                        let is_winner = solutions.len() - index == 1;
                        let mut settlement = SolverSettlement {
                            solver: participant.driver.name.clone(),
                            solver_address: participant.solution.account,
                            score: Some(Score::Solver(participant.solution.score.get())),
                            ranking: solutions.len() - index,
                            orders: participant
                                .solution
                                .orders()
                                .iter()
                                .map(|(id, order)| Order::Colocated {
                                    id: (*id).into(),
                                    sell_amount: order.sell_amount,
                                    buy_amount: order.buy_amount,
                                })
                                .collect(),
                            clearing_prices: participant
                                .solution
                                .clearing_prices
                                .iter()
                                .map(|(token, price)| (*token, *price))
                                .collect(),
                            call_data: None,
                            uninternalized_call_data: None,
                        };
                        if is_winner {
                            settlement.call_data = Some(revealed.calldata.internalized.clone());
                            settlement.uninternalized_call_data =
                                Some(revealed.calldata.uninternalized.clone());
                        }
                        settlement
                    })
                    .collect(),
            };
            let competition = Competition {
                auction_id,
                winner,
                winning_score,
                reference_score,
                participants,
                prices,
                block_deadline,
                competition_simulation_block,
                call_data,
                uninternalized_call_data,
                competition_table,
            };

            tracing::info!(?competition, "saving competition");
            if let Err(err) = self.persistence.save_competition(&competition).await {
                tracing::error!(?err, "failed to save competition");
                return;
            }

            tracing::info!("saving fee policies");
            if let Err(err) = self
                .persistence
                .store_fee_policies(auction_id, fee_policies)
                .await
            {
                Metrics::fee_policies_store_error();
                tracing::warn!(?err, "failed to save fee policies");
            }

            tracing::info!(driver = %driver.name, "settling");
            let submission_start = Instant::now();
            match self.settle(driver, solution, auction_id).await {
                Ok(()) => Metrics::settle_ok(driver, submission_start.elapsed()),
                Err(err) => {
                    Metrics::settle_err(driver, &err, submission_start.elapsed());
                    tracing::warn!(?err, driver = %driver.name, "settlement failed");
                }
            }
            let unsettled_orders: HashSet<_> = solutions
                .iter()
                .flat_map(|p| p.solution.orders.keys())
                .filter(|uid| !solution.orders.contains_key(uid))
                .collect();
            Metrics::matched_unsettled(driver, unsettled_orders);
        }
    }

    /// Runs the solver competition, making all configured drivers participate.
    async fn competition(
        &self,
        id: domain::auction::Id,
        auction: &domain::Auction,
    ) -> Vec<Participant<'_>> {
        let request = solve::Request::new(
            id,
            auction,
            &self.market_makable_token_list.all(),
            self.solve_deadline,
        );
        let request = &request;

        let order_uids = auction.orders.iter().map(|o| OrderUid(o.uid.0)).collect();
        self.persistence
            .store_order_events(order_uids, OrderEventLabel::Ready);

        let start = Instant::now();
        futures::future::join_all(self.drivers.iter().map(|driver| async move {
            let result = self.solve(driver, request).await;
            let solutions = match result {
                Ok(solutions) => {
                    Metrics::solve_ok(driver, start.elapsed());
                    solutions
                }
                Err(err) => {
                    Metrics::solve_err(driver, start.elapsed(), &err);
                    if matches!(err, SolveError::NoSolutions) {
                        tracing::debug!(driver = %driver.name, "solver found no solution");
                    } else {
                        tracing::warn!(?err, driver = %driver.name, "solve error");
                    }
                    vec![]
                }
            };

            solutions.into_iter().filter_map(|solution| match solution {
                Ok(solution) => {
                    Metrics::solution_ok(driver);
                    Some(Participant { driver, solution })
                }
                Err(err) => {
                    Metrics::solution_err(driver, &err);
                    tracing::debug!(?err, driver = %driver.name, "invalid proposed solution");
                    None
                }
            })
        }))
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    /// Computes a driver's solutions for the solver competition.
    async fn solve(
        &self,
        driver: &infra::Driver,
        request: &solve::Request,
    ) -> Result<Vec<Result<Solution, ZeroScoreError>>, SolveError> {
        let response = tokio::time::timeout(self.solve_deadline, driver.solve(request))
            .await
            .map_err(|_| SolveError::Timeout)?
            .map_err(SolveError::Failure)?;
        if response.solutions.is_empty() {
            return Err(SolveError::NoSolutions);
        }

        Ok(response
            .solutions
            .into_iter()
            .map(|solution| {
                Ok(Solution {
                    id: solution.solution_id,
                    account: solution.submission_address,
                    score: NonZeroU256::new(solution.score).ok_or(ZeroScoreError)?,
                    orders: solution
                        .orders
                        .into_iter()
                        .map(|(o, amounts)| (o.into(), amounts))
                        .collect(),
                    clearing_prices: solution.clearing_prices,
                })
            })
            .collect())
    }

    /// Ask the winning solver to reveal their solution.
    async fn reveal(
        &self,
        driver: &infra::Driver,
        auction: domain::auction::Id,
        solution_id: u64,
    ) -> Result<reveal::Response, RevealError> {
        let response = driver
            .reveal(&reveal::Request { solution_id })
            .await
            .map_err(RevealError::Failure)?;
        if !response
            .calldata
            .internalized
            .ends_with(&auction.to_be_bytes())
        {
            return Err(RevealError::AuctionMismatch);
        }

        Ok(response)
    }

    /// Execute the solver's solution. Returns Ok when the corresponding
    /// transaction has been mined.
    async fn settle(
        &self,
        driver: &infra::Driver,
        solved: &Solution,
        auction_id: i64,
    ) -> Result<(), SettleError> {
        let order_ids = solved.order_ids().copied().collect();
        self.persistence
            .store_order_events(order_ids, OrderEventLabel::Executing);

        let tag = auction_id.to_be_bytes();
        let request = settle::Request {
            solution_id: solved.id,
        };
        let tx_hash = self.wait_for_settlement(driver, &tag, request).await?;
        *self.in_flight_orders.lock().await = Some(InFlightOrders {
            tx_hash,
            orders: solved.orders.keys().copied().collect(),
        });
        tracing::debug!(?tx_hash, "solution settled");

        Ok(())
    }

    /// Wait for either the settlement transaction to be mined or the driver
    /// returned a result.
    async fn wait_for_settlement(
        &self,
        driver: &infra::Driver,
        tag: &[u8],
        request: settle::Request,
    ) -> Result<H256, SettleError> {
        match futures::future::select(
            Box::pin(self.wait_for_settlement_transaction(tag, self.submission_deadline)),
            Box::pin(driver.settle(&request, self.max_settlement_transaction_wait)),
        )
        .await
        {
            futures::future::Either::Left((res, _)) => res,
            futures::future::Either::Right((res, _)) => {
                res.map_err(SettleError::Failure).map(|tx| tx.tx_hash)
            }
        }
    }

    /// Tries to find a `settle` contract call with calldata ending in `tag`.
    ///
    /// Returns None if no transaction was found within the deadline or the task
    /// is cancelled.
    async fn wait_for_settlement_transaction(
        &self,
        tag: &[u8],
        max_blocks_wait: u64,
    ) -> Result<H256, SettleError> {
        let start_offset = MAX_REORG_BLOCK_COUNT;
        let current = self.eth.current_block().borrow().number;
        let start = current.saturating_sub(start_offset);
        let deadline = current.saturating_add(max_blocks_wait);
        tracing::debug!(%current, %start, %deadline, ?tag, "waiting for tag");
        let mut seen_transactions: HashSet<H256> = Default::default();
        loop {
            if self.eth.current_block().borrow().number > deadline {
                break;
            }
            let Ok(mut hashes) = self
                .persistence
                .recent_settlement_tx_hashes(start..deadline + 1)
                .await
                .inspect_err(|err| {
                    tracing::warn!(?err, "failed to fetch recent settlement tx hashes")
                })
            else {
                continue;
            };
            hashes.retain(|hash| !seen_transactions.contains(hash));
            for hash in hashes {
                let Ok(Some(tx)) = self.eth.transaction(hash).await else {
                    tracing::warn!(?hash, "unable to fetch a tx");
                    continue;
                };
                if tx.input.0.ends_with(tag) {
                    return Ok(tx.hash);
                }
                seen_transactions.insert(hash);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
        Err(SettleError::Failure(anyhow::anyhow!(
            "settlement transaction await reached deadline"
        )))
    }

    /// Removes orders that are currently being settled to avoid solvers trying
    /// to fill an order a second time.
    async fn remove_in_flight_orders(&self, mut auction: domain::Auction) -> domain::Auction {
        let Some(in_flight) = &*self.in_flight_orders.lock().await else {
            return auction;
        };

        let tx_receipt = self.eth.transaction_receipt(in_flight.tx_hash).await;

        let prev_settlement_block = match tx_receipt {
            Ok(Some(TransactionReceipt {
                block_number: Some(number),
                ..
            })) => number.0[0],
            // Could not find the block of the previous settlement, let's be
            // conservative and assume all orders are still in-flight.
            _ => u64::MAX,
        };

        if auction.latest_settlement_block < prev_settlement_block {
            // Auction was built before the in-flight orders were processed.
            auction
                .orders
                .retain(|o| !in_flight.orders.contains(&o.uid));
            tracing::debug!(orders = ?in_flight.orders, "filtered out in-flight orders");
        }

        auction
    }
}

/// Orders settled in the previous auction that might still be in-flight.
#[derive(Default)]
pub struct InFlightOrders {
    /// The transaction that these orders where settled in.
    tx_hash: H256,
    orders: HashSet<domain::OrderUid>,
}

struct Participant<'a> {
    driver: &'a infra::Driver,
    solution: Solution,
}

struct Solution {
    id: u64,
    account: H160,
    score: NonZeroU256,
    orders: HashMap<domain::OrderUid, solve::TradedAmounts>,
    clearing_prices: HashMap<H160, U256>,
}

impl Solution {
    pub fn order_ids(&self) -> impl Iterator<Item = &domain::OrderUid> {
        self.orders.keys()
    }

    pub fn orders(&self) -> &HashMap<domain::OrderUid, solve::TradedAmounts> {
        &self.orders
    }
}

#[derive(Debug, thiserror::Error)]
enum SolveError {
    #[error("the solver timed out")]
    Timeout,
    #[error("driver did not propose any solutions")]
    NoSolutions,
    #[error(transparent)]
    Failure(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("the solver proposed a 0-score solution")]
struct ZeroScoreError;

#[derive(Debug, thiserror::Error)]
enum RevealError {
    #[error("revealed calldata does not match auction")]
    AuctionMismatch,
    #[error(transparent)]
    Failure(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
enum SettleError {
    #[error(transparent)]
    Failure(anyhow::Error),
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "runloop")]
struct Metrics {
    /// Tracks the last executed auction.
    auction: prometheus::IntGauge,

    /// Tracks the duration of successful driver `/solve` requests.
    #[metric(
        labels("driver", "result"),
        buckets(
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20
        )
    )]
    solve: prometheus::HistogramVec,

    /// Tracks driver solutions.
    #[metric(labels("driver", "result"))]
    solutions: prometheus::IntCounterVec,

    /// Tracks the result of driver `/reveal` requests.
    #[metric(labels("driver", "result"))]
    reveal: prometheus::IntCounterVec,

    /// Tracks the times and results of driver `/settle` requests.
    #[metric(labels("driver", "result"))]
    settle_time: prometheus::IntCounterVec,

    /// Tracks the number of orders that were part of some but not the winning
    /// solution together with the winning driver that did't include it.
    #[metric(labels("ignored_by"))]
    matched_unsettled: prometheus::IntCounterVec,

    /// Tracks the number of database errors.
    #[metric(labels("error_type"))]
    db_metric_error: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(metrics::get_storage_registry()).unwrap()
    }

    fn auction(auction_id: domain::auction::Id) {
        Self::get().auction.set(auction_id)
    }

    fn solve_ok(driver: &infra::Driver, elapsed: Duration) {
        Self::get()
            .solve
            .with_label_values(&[&driver.name, "success"])
            .observe(elapsed.as_secs_f64())
    }

    fn solve_err(driver: &infra::Driver, elapsed: Duration, err: &SolveError) {
        let label = match err {
            SolveError::Timeout => "timeout",
            SolveError::NoSolutions => "no_solutions",
            SolveError::Failure(_) => "error",
        };
        Self::get()
            .solve
            .with_label_values(&[&driver.name, label])
            .observe(elapsed.as_secs_f64())
    }

    fn solution_ok(driver: &infra::Driver) {
        Self::get()
            .solutions
            .with_label_values(&[&driver.name, "success"])
            .inc();
    }

    fn solution_err(driver: &infra::Driver, _: &ZeroScoreError) {
        Self::get()
            .solutions
            .with_label_values(&[&driver.name, "zero_score"])
            .inc();
    }

    fn reveal_ok(driver: &infra::Driver) {
        Self::get()
            .reveal
            .with_label_values(&[&driver.name, "success"])
            .inc();
    }

    fn reveal_err(driver: &infra::Driver, err: &RevealError) {
        let label = match err {
            RevealError::AuctionMismatch => "mismatch",
            RevealError::Failure(_) => "error",
        };
        Self::get()
            .reveal
            .with_label_values(&[&driver.name, label])
            .inc();
    }

    fn settle_ok(driver: &infra::Driver, time: Duration) {
        Self::get()
            .settle_time
            .with_label_values(&[&driver.name, "success"])
            .inc_by(time.as_millis().try_into().unwrap_or(u64::MAX));
    }

    fn settle_err(driver: &infra::Driver, err: &SettleError, time: Duration) {
        let label = match err {
            SettleError::Failure(_) => "error",
        };
        Self::get()
            .settle_time
            .with_label_values(&[&driver.name, label])
            .inc_by(time.as_millis().try_into().unwrap_or(u64::MAX));
    }

    fn matched_unsettled(winning: &infra::Driver, unsettled: HashSet<&domain::OrderUid>) {
        if !unsettled.is_empty() {
            tracing::debug!(?unsettled, "some orders were matched but not settled");
        }
        Self::get()
            .matched_unsettled
            .with_label_values(&[&winning.name])
            .inc_by(unsettled.len() as u64);
    }

    fn fee_policies_store_error() {
        Self::get()
            .db_metric_error
            .with_label_values(&["fee_policies_store"])
            .inc();
    }
}

pub mod observe {
    use {crate::domain, std::collections::HashSet};

    pub fn log_auction_delta(
        id: i64,
        previous: &Option<domain::Auction>,
        current: &domain::Auction,
    ) {
        let previous_uids = match previous {
            Some(previous) => previous
                .orders
                .iter()
                .map(|order| order.uid)
                .collect::<HashSet<_>>(),
            None => HashSet::new(),
        };
        let current_uids = current
            .orders
            .iter()
            .map(|order| order.uid)
            .collect::<HashSet<_>>();
        let added = current_uids.difference(&previous_uids);
        let removed = previous_uids.difference(&current_uids);
        tracing::debug!(
            id,
            added = ?added,
            "New orders in auction"
        );
        tracing::debug!(
            id,
            removed = ?removed,
            "Orders no longer in auction"
        );
    }
}
