use {
    crate::{
        arguments::RunLoopMode,
        database::competition::Competition,
        domain::{
            self,
            auction::order::Class,
            competition::{self, SolutionError},
            OrderUid,
        },
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
    itertools::Itertools,
    model::solver_competition::{
        CompetitionAuction,
        Order,
        Score,
        SolverCompetitionDB,
        SolverSettlement,
    },
    primitive_types::H256,
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::Mutex,
    tracing::Instrument,
};

pub struct RunLoop {
    pub eth: infra::Ethereum,
    pub persistence: infra::Persistence,
    pub drivers: Vec<infra::Driver>,

    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub market_makable_token_list: AutoUpdatingTokenList,
    pub submission_deadline: u64,
    pub max_settlement_transaction_wait: Duration,
    pub solve_deadline: Duration,
    pub in_flight_orders: Arc<Mutex<Option<InFlightOrders>>>,
    pub liveness: Arc<Liveness>,
    pub synchronization: RunLoopMode,
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
        if let RunLoopMode::SyncToBlockchain = self.synchronization {
            let _ = ethrpc::current_block::next_block(self.eth.current_block()).await;
        }

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
            solutions.sort_unstable_by_key(|participant| participant.solution.score().get().0);
            solutions
        };
        let competition_simulation_block = self.eth.current_block().borrow().number;

        let considered_orders = solutions
            .iter()
            .flat_map(|solution| solution.solution.order_ids().copied())
            .unique()
            .collect();
        self.persistence
            .store_order_events(considered_orders, OrderEventLabel::Considered);

        // TODO: Keep going with other solutions until some deadline.
        if let Some(Participant { driver, solution }) = solutions.last() {
            tracing::info!(driver = %driver.name, solution = %solution.id(), "winner");

            let revealed = match self.reveal(driver, auction_id, solution.id()).await {
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

            let winner = solution.solver().into();
            let winning_score = solution.score().get().0;
            let reference_score = solutions
                .iter()
                .nth_back(1)
                .map(|participant| participant.solution.score().get().0)
                .unwrap_or_default();
            let participants = solutions
                .iter()
                .map(|participant| participant.solution.solver().into())
                .collect::<HashSet<_>>();

            let mut prices = BTreeMap::new();
            let mut fee_policies = Vec::new();
            let block_deadline = competition_simulation_block + self.submission_deadline;
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
                        if let Some(price) = auction.prices.get(&auction_order.sell.token) {
                            prices.insert(auction_order.sell.token, *price);
                        } else {
                            tracing::error!(
                                sell_token = ?auction_order.sell.token,
                                "sell token price is missing in auction"
                            );
                        }
                        if let Some(price) = auction.prices.get(&auction_order.buy.token) {
                            prices.insert(auction_order.buy.token, *price);
                        } else {
                            tracing::error!(
                                buy_token = ?auction_order.buy.token,
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
                    prices: auction
                        .prices
                        .into_iter()
                        .map(|(key, value)| (key.into(), value.get().into()))
                        .collect(),
                },
                solutions: solutions
                    .iter()
                    .enumerate()
                    .map(|(index, participant)| {
                        let is_winner = solutions.len() - index == 1;
                        let mut settlement = SolverSettlement {
                            solver: participant.driver.name.clone(),
                            solver_address: participant.solution.solver().0,
                            score: Some(Score::Solver(participant.solution.score().get().0)),
                            ranking: solutions.len() - index,
                            orders: participant
                                .solution
                                .orders()
                                .iter()
                                .map(|(id, order)| Order::Colocated {
                                    id: (*id).into(),
                                    sell_amount: order.sell.into(),
                                    buy_amount: order.buy.into(),
                                })
                                .collect(),
                            clearing_prices: participant
                                .solution
                                .prices()
                                .iter()
                                .map(|(token, price)| (token.0, price.get().into()))
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
                prices: prices
                    .into_iter()
                    .map(|(key, value)| (key.into(), value.get().into()))
                    .collect(),
                block_deadline,
                competition_simulation_block,
                call_data,
                uninternalized_call_data,
                competition_table,
            };

            tracing::trace!(?competition, "saving competition");
            if let Err(err) = self.persistence.save_competition(&competition).await {
                tracing::error!(?err, "failed to save competition");
                return;
            }

            if let Err(err) = self
                .persistence
                .save_surplus_capturing_jit_orders_orders(
                    auction_id,
                    &auction.surplus_capturing_jit_order_owners,
                )
                .await
            {
                tracing::error!(?err, "failed to save surplus capturing jit order owners");
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
            match self
                .settle(driver, solution, auction_id, block_deadline)
                .await
            {
                Ok(()) => Metrics::settle_ok(driver, submission_start.elapsed()),
                Err(err) => {
                    Metrics::settle_err(driver, &err, submission_start.elapsed());
                    tracing::warn!(?err, driver = %driver.name, "settlement failed");
                }
            }
            let solution_uids = solution.order_ids().copied().collect::<HashSet<_>>();
            let auction_uids = auction.orders.iter().map(|o| o.uid).collect::<HashSet<_>>();

            let unsettled_orders: HashSet<_> = solutions
                .iter()
                // Report orders that were part of any solution candidate
                .flat_map(|p| p.solution.order_ids())
                // but not part of the winning one
                .filter(|uid| !solution_uids.contains(uid))
                // yet still part of the auction (filter out jit orders)
                .filter(|uid| auction_uids.contains(uid))
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
    ) -> Result<
        Vec<Result<competition::SolutionWithId, domain::competition::SolutionError>>,
        SolveError,
    > {
        let response = tokio::time::timeout(self.solve_deadline, driver.solve(request))
            .await
            .map_err(|_| SolveError::Timeout)?
            .map_err(SolveError::Failure)?;
        if response.solutions.is_empty() {
            return Err(SolveError::NoSolutions);
        }
        let solutions = response.into_domain();

        // TODO: remove this workaround when implementing #2780
        // Discard any solutions from solvers that got deny listed in the mean time.
        let futures = solutions.into_iter().map(|solution| async {
            let solution = solution?;
            let solver = solution.solver();
            let is_allowed = self
                .eth
                .contracts()
                .authenticator()
                .is_solver(solver.into())
                .call()
                .await;

            match is_allowed {
                Ok(true) => Ok(solution),
                Ok(false) => Err(domain::competition::SolutionError::SolverDenyListed),
                Err(err) => {
                    // log warning but discard solution anyway to be on the safe side
                    tracing::warn!(
                        driver = driver.name,
                        ?solver,
                        ?err,
                        "failed to check if solver is deny listed"
                    );
                    Err(domain::competition::SolutionError::SolverDenyListed)
                }
            }
        });

        Ok(futures::future::join_all(futures).await)
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
        solved: &competition::SolutionWithId,
        auction_id: i64,
        submission_deadline_latest_block: u64,
    ) -> Result<(), SettleError> {
        let order_ids = solved.order_ids().copied().collect();
        self.persistence
            .store_order_events(order_ids, OrderEventLabel::Executing);

        let request = settle::Request {
            solution_id: solved.id(),
            submission_deadline_latest_block,
        };
        let tx_hash = self
            .wait_for_settlement(driver, auction_id, request)
            .await?;
        *self.in_flight_orders.lock().await = Some(InFlightOrders {
            tx_hash,
            orders: solved.order_ids().copied().collect(),
        });
        tracing::debug!(?tx_hash, "solution settled");

        Ok(())
    }

    /// Wait for either the settlement transaction to be mined or the driver
    /// returned a result.
    async fn wait_for_settlement(
        &self,
        driver: &infra::Driver,
        auction_id: i64,
        request: settle::Request,
    ) -> Result<H256, SettleError> {
        match futures::future::select(
            Box::pin(self.wait_for_settlement_transaction(auction_id, self.submission_deadline)),
            Box::pin(driver.settle(&request, self.max_settlement_transaction_wait)),
        )
        .await
        {
            futures::future::Either::Left((res, _)) => res,
            futures::future::Either::Right((driver_result, onchain_task)) => {
                driver_result.map_err(|err| {
                    tracing::warn!(?err, "driver settle request failed");
                    SettleError::Failure(err)
                })?;
                onchain_task.await
            }
        }
    }

    /// Tries to find a `settle` contract call with calldata ending in `tag`.
    ///
    /// Returns None if no transaction was found within the deadline or the task
    /// is cancelled.
    async fn wait_for_settlement_transaction(
        &self,
        auction_id: i64,
        max_blocks_wait: u64,
    ) -> Result<H256, SettleError> {
        let current = self.eth.current_block().borrow().number;
        let deadline = current.saturating_add(max_blocks_wait);
        tracing::debug!(%current, %deadline, %auction_id, "waiting for tag");
        loop {
            if self.eth.current_block().borrow().number > deadline {
                break;
            }

            match self
                .persistence
                .find_tx_hash_by_auction_id(auction_id)
                .await
            {
                Ok(Some(hash)) => return Ok(hash),
                Err(err) => {
                    tracing::warn!(?err, "failed to fetch recent settlement tx hashes");
                }
                Ok(None) => {}
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
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

        let transaction = self.eth.transaction(in_flight.tx_hash.into()).await;

        let prev_settlement_block = match transaction {
            Ok(transaction) => transaction.block,
            // Could not find the block of the previous settlement, let's be
            // conservative and assume all orders are still in-flight.
            _ => u64::MAX.into(),
        };

        if auction.latest_settlement_block < prev_settlement_block.0 {
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
    solution: competition::SolutionWithId,
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

    fn solution_err(driver: &infra::Driver, err: &SolutionError) {
        let label = match err {
            SolutionError::ZeroScore(_) => "zero_score",
            SolutionError::InvalidPrice(_) => "invalid_price",
            SolutionError::SolverDenyListed => "solver_deny_listed",
        };
        Self::get()
            .solutions
            .with_label_values(&[&driver.name, label])
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
