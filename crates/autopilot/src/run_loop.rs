use {
    crate::{
        arguments::RunLoopMode,
        database::competition::Competition,
        domain::{
            self,
            competition::{self, SolutionError, TradedAmounts},
            eth::{self, TxId},
            OrderUid,
        },
        infra::{
            self,
            solvers::dto::{settle, solve},
        },
        maintenance::Maintenance,
        run::Liveness,
        solvable_orders::SolvableOrdersCache,
    },
    ::observe::metrics,
    anyhow::{Context, Result},
    database::order_events::OrderEventLabel,
    ethcontract::U256,
    ethrpc::block_stream::BlockInfo,
    futures::future::BoxFuture,
    model::solver_competition::{
        CompetitionAuction,
        Order,
        Score,
        SolverCompetitionDB,
        SolverSettlement,
    },
    primitive_types::H256,
    prometheus::core::Number,
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::{mpsc, Mutex},
    tracing::{warn, Instrument},
};

pub struct RunLoop {
    pub eth: infra::Ethereum,
    pub persistence: infra::Persistence,
    pub drivers: Vec<Arc<infra::Driver>>,

    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub market_makable_token_list: AutoUpdatingTokenList,
    pub submission_deadline: u64,
    pub max_settlement_transaction_wait: Duration,
    pub solve_deadline: Duration,
    pub in_flight_orders: Arc<Mutex<HashSet<OrderUid>>>,
    pub liveness: Arc<Liveness>,
    pub synchronization: RunLoopMode,
    /// How much time past observing the current block the runloop is
    /// allowed to start before it has to re-synchronize to the blockchain
    /// by waiting for the next block to appear.
    pub max_run_loop_delay: Duration,
    /// Maintenance tasks that should run before every runloop to have
    /// the most recent data available.
    pub maintenance: Arc<Maintenance>,
    /// Queue for executing settle futures one by one guarantying FIFO execution
    /// order.
    execution_queue: mpsc::UnboundedSender<BoxFuture<'static, ()>>,
}

impl RunLoop {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        solvable_orders_cache: Arc<SolvableOrdersCache>,
        market_makable_token_list: AutoUpdatingTokenList,
        submission_deadline: u64,
        max_settlement_transaction_wait: Duration,
        solve_deadline: Duration,
        liveness: Arc<Liveness>,
        synchronization: RunLoopMode,
        max_run_loop_delay: Duration,
        maintenance: Arc<Maintenance>,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(future) = rx.recv().await {
                future.await;
            }
            tracing::info!("runloop execution queue stopped")
        });

        Self {
            eth,
            persistence,
            drivers,
            solvable_orders_cache,
            market_makable_token_list,
            submission_deadline,
            max_settlement_transaction_wait,
            solve_deadline,
            in_flight_orders: Default::default(),
            liveness,
            synchronization,
            max_run_loop_delay,
            maintenance,
            execution_queue: tx,
        }
    }

    pub async fn run_forever(self, update_interval: Duration) -> ! {
        Maintenance::spawn_background_task(
            self.maintenance.clone(),
            self.synchronization,
            self.eth.current_block().clone(),
            update_interval,
        );

        let mut last_auction = None;
        let mut last_block = None;
        let self_arc = Arc::new(self);
        loop {
            let auction_with_block_number = self_arc
                .next_auction(&mut last_auction, &mut last_block)
                .await;
            if let Some((domain::AuctionWithId { id, auction }, start_block)) =
                auction_with_block_number
            {
                self_arc
                    .single_run(id, auction, start_block)
                    .instrument(tracing::info_span!("auction", id))
                    .await;
            };
        }
    }

    /// Sleeps until the next auction is supposed to start, builds it and
    /// returns it.
    async fn next_auction(
        &self,
        prev_auction: &mut Option<domain::Auction>,
        prev_block: &mut Option<H256>,
    ) -> Option<(domain::AuctionWithId, u64)> {
        // wait for appropriate time to start building the auction
        let start_block = match self.synchronization {
            RunLoopMode::Unsynchronized => {
                // Sleep a bit to avoid busy loops.
                tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
                *self.eth.current_block().borrow()
            }
            RunLoopMode::SyncToBlockchain => {
                let current_block = *self.eth.current_block().borrow();
                let time_since_last_block = current_block.observed_at.elapsed();
                let auction_block = if time_since_last_block > self.max_run_loop_delay {
                    if prev_block.is_some_and(|prev_block| prev_block != current_block.hash) {
                        // don't emit warning if we finished prev run loop within the same block
                        tracing::warn!(
                            missed_by = ?time_since_last_block - self.max_run_loop_delay,
                            "missed optimal auction start, wait for new block"
                        );
                    }
                    ethrpc::block_stream::next_block(self.eth.current_block()).await
                } else {
                    current_block
                };

                self.run_maintenance(&auction_block).await;
                if let Err(err) = self
                    .solvable_orders_cache
                    .update(auction_block.number)
                    .await
                {
                    tracing::warn!(?err, "failed to update auction");
                }
                auction_block
            }
        };

        let auction = self.cut_auction().await?;

        // Only run the solvers if the auction or block has changed.
        let previous = prev_auction.replace(auction.auction.clone());
        if previous.as_ref() == Some(&auction.auction)
            && prev_block.replace(start_block.hash) == Some(start_block.hash)
        {
            return None;
        }

        observe::log_auction_delta(&previous, &auction);
        self.liveness.auction();
        Metrics::auction_ready(start_block.observed_at);
        Some((auction, start_block.number))
    }

    /// Runs maintenance on all components to ensure the system uses
    /// the latest available state.
    async fn run_maintenance(&self, block: &BlockInfo) {
        let start = Instant::now();
        self.maintenance.update(block).await;
        Metrics::ran_maintenance(start.elapsed());
    }

    async fn cut_auction(&self) -> Option<domain::AuctionWithId> {
        let auction = match self.solvable_orders_cache.current_auction().await {
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

        if auction.orders.is_empty() {
            // Updating liveness probe to not report unhealthy due to this optimization
            self.liveness.auction();
            tracing::debug!("skipping empty auction");
            return None;
        }

        Some(domain::AuctionWithId { id, auction })
    }

    async fn single_run(
        self: &Arc<Self>,
        auction_id: domain::auction::Id,
        auction: domain::Auction,
        start_block: u64,
    ) {
        let single_run_start = Instant::now();
        tracing::info!(?auction_id, "solving");

        let auction = self.remove_in_flight_orders(auction).await;

        let solutions = self.competition(auction_id, &auction).await;
        if solutions.is_empty() {
            tracing::info!("no solutions for auction");
            return;
        }

        let competition_simulation_block = self.eth.current_block().borrow().number;

        // TODO: Keep going with other solutions until some deadline.
        if let Some(Participant { driver, solution }) = solutions.last() {
            tracing::info!(driver = %driver.name, solution = %solution.id(), "winner");

            let block_deadline = competition_simulation_block + self.submission_deadline;

            // Post-processing should not be executed asynchronously since it includes steps
            // of storing all the competition/auction-related data to the DB.
            if let Err(err) = self
                .post_processing(
                    auction_id,
                    auction,
                    competition_simulation_block,
                    solution,
                    &solutions,
                    block_deadline,
                )
                .await
            {
                tracing::error!(?err, "failed to post-process competition");
                return;
            }

            let solved_order_uids: HashSet<_> = solution.orders().keys().cloned().collect();
            self.in_flight_orders
                .lock()
                .await
                .extend(solved_order_uids.clone());

            let solution_id = solution.id();
            let driver_ = driver.clone();
            let self_ = self.clone();
            let settle_fut = async move {
                tracing::info!(driver = %driver_.name, "settling");
                let submission_start = Instant::now();

                match self_
                    .settle(
                        driver_.clone(),
                        solution_id,
                        solved_order_uids,
                        auction_id,
                        block_deadline,
                    )
                    .await
                {
                    Ok(tx_id) => {
                        Metrics::settle_ok(&driver_, submission_start.elapsed());
                        let elapsed = single_run_start.elapsed();
                        tokio::spawn(async move {
                            let blocks_mined = self_
                                .eth
                                .transaction(tx_id)
                                .await
                                .map(|tx| tx.block.0 - start_block)
                                .ok();
                            Metrics::single_run_completed(elapsed, blocks_mined);
                        });
                    }
                    Err(err) => {
                        Metrics::settle_err(&driver_, submission_start.elapsed(), &err);
                        tracing::warn!(?err, driver = %driver_.name, "settlement failed");
                    }
                }
            };

            if let Err(err) = self.execution_queue.send(Box::pin(settle_fut)) {
                tracing::error!(?err, "failed to send settle future to execution queue");
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn post_processing(
        &self,
        auction_id: domain::auction::Id,
        auction: domain::Auction,
        competition_simulation_block: u64,
        winning_solution: &competition::SolutionWithId,
        solutions: &[Participant],
        block_deadline: u64,
    ) -> Result<()> {
        let start = Instant::now();
        let winner = winning_solution.solver().into();
        let winning_score = winning_solution.score().get().0;
        let reference_score = solutions
            .iter()
            .nth_back(1)
            .map(|participant| participant.solution.score().get().0)
            .unwrap_or_default();
        let participants = solutions
            .iter()
            .map(|participant| participant.solution.solver().into())
            .collect::<HashSet<_>>();

        let mut fee_policies = Vec::new();
        for order_id in winning_solution.order_ids() {
            match auction
                .orders
                .iter()
                .find(|auction_order| &auction_order.uid == order_id)
            {
                Some(auction_order) => {
                    fee_policies.push((auction_order.uid, auction_order.protocol_fees.clone()));
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
                    .iter()
                    .map(|(key, value)| ((*key).into(), value.get().into()))
                    .collect(),
            },
            solutions: solutions
                .iter()
                .enumerate()
                .map(|(index, participant)| SolverSettlement {
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
                })
                .collect(),
        };
        let competition = Competition {
            auction_id,
            winner,
            winning_score,
            reference_score,
            participants,
            prices: auction
                .prices
                .into_iter()
                .map(|(key, value)| (key.into(), value.get().into()))
                .collect(),
            block_deadline,
            competition_simulation_block,
            competition_table,
        };

        tracing::trace!(?competition, "saving competition");
        self.persistence
            .save_competition(&competition)
            .await
            .context("failed to save competition")?;

        self.persistence
            .save_surplus_capturing_jit_orders_orders(
                auction_id,
                &auction.surplus_capturing_jit_order_owners,
            )
            .await
            .context("failed to save surplus capturing jit order owners")?;

        tracing::info!("saving fee policies");
        if let Err(err) = self
            .persistence
            .store_fee_policies(auction_id, fee_policies)
            .await
        {
            Metrics::fee_policies_store_error();
            tracing::warn!(?err, "failed to save fee policies");
        }
        Metrics::post_processed(start.elapsed());
        Ok(())
    }

    /// Runs the solver competition, making all configured drivers participate.
    /// Returns all fair solutions sorted by their score (worst to best).
    async fn competition(
        &self,
        id: domain::auction::Id,
        auction: &domain::Auction,
    ) -> Vec<Participant> {
        let request = solve::Request::new(
            id,
            auction,
            &self.market_makable_token_list.all(),
            self.solve_deadline,
        );
        let request = &request;

        let order_uids = auction.orders.iter().map(|o| OrderUid(o.uid.0));
        self.persistence
            .store_order_events(order_uids, OrderEventLabel::Ready);

        let mut solutions = futures::future::join_all(
            self.drivers
                .iter()
                .map(|driver| self.solve(driver.clone(), request)),
        )
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        // Shuffle so that sorting randomly splits ties.
        solutions.shuffle(&mut rand::thread_rng());
        solutions.sort_unstable_by_key(|participant| participant.solution.score().get().0);

        // Make sure the winning solution is fair.
        while !Self::is_solution_fair(solutions.last(), &solutions, auction) {
            let unfair_solution = solutions.pop().expect("must exist");
            warn!(
                invalidated = unfair_solution.driver.name,
                "fairness check invalidated of solution"
            );
        }
        self.report_on_solutions(&solutions, auction);

        solutions
    }

    /// Records metrics, order events and logs for the given solutions.
    /// Expects the winning solution to be the last in the list.
    fn report_on_solutions(&self, solutions: &[Participant], auction: &domain::Auction) {
        let Some(winner) = solutions.last() else {
            // no solutions means nothing to report
            return;
        };

        solutions.iter().for_each(|solution| {
            tracing::debug!(
                driver=%solution.driver.name,
                orders=?solution.solution.order_ids(),
                solution=solution.solution.id(),
                "proposed solution"
            );
        });

        let proposed_orders: HashSet<_> = solutions
            .iter()
            .flat_map(|solution| solution.solution.order_ids().copied())
            .collect();
        let winning_orders: HashSet<_> = solutions
            .last()
            .into_iter()
            .flat_map(|solution| solution.solution.order_ids().copied())
            .collect();
        let mut non_winning_orders: HashSet<_> = proposed_orders
            .difference(&winning_orders)
            .cloned()
            .collect();
        self.persistence.store_order_events(
            non_winning_orders.iter().cloned(),
            OrderEventLabel::Considered,
        );
        self.persistence
            .store_order_events(winning_orders, OrderEventLabel::Executing);

        let auction_uids = auction.orders.iter().map(|o| o.uid).collect::<HashSet<_>>();

        // Report orders that were part of a non-winning solution candidate
        // but only if they were part of the auction (filter out jit orders)
        non_winning_orders.retain(|uid| auction_uids.contains(uid));
        Metrics::matched_unsettled(&winner.driver, non_winning_orders);
    }

    /// Returns true if winning solution is fair or winner is None
    fn is_solution_fair(
        winner: Option<&Participant>,
        remaining: &Vec<Participant>,
        auction: &domain::Auction,
    ) -> bool {
        let Some(winner) = winner else { return true };
        let Some(fairness_threshold) = winner.driver.fairness_threshold else {
            return true;
        };

        // Returns the surplus difference in the buy token if `left`
        // is better for the trader than `right`, or 0 otherwise.
        // This takes differently partial fills into account.
        let improvement_in_buy = |left: &TradedAmounts, right: &TradedAmounts| {
            // If `left.sell / left.buy < right.sell / right.buy`, left is "better" as the
            // trader either sells less or gets more. This can be reformulated as
            // `right.sell * left.buy > left.sell * right.buy`.
            let right_sell_left_buy = right.sell.0.full_mul(left.buy.0);
            let left_sell_right_buy = left.sell.0.full_mul(right.buy.0);
            let improvement = right_sell_left_buy
                .checked_sub(left_sell_right_buy)
                .unwrap_or_default();

            // The difference divided by the original sell amount is the improvement in buy
            // token. Casting to U256 is safe because the difference is smaller than the
            // original product, which if re-divided by right.sell must fit in U256.
            improvement
                .checked_div(right.sell.0.into())
                .map(|v| U256::try_from(v).expect("improvement in buy fits in U256"))
                .unwrap_or_default()
        };

        // Record best execution per order
        let mut best_executions = HashMap::new();
        for other in remaining {
            for (uid, execution) in other.solution.orders() {
                best_executions
                    .entry(uid)
                    .and_modify(|best_execution| {
                        if !improvement_in_buy(execution, best_execution).is_zero() {
                            *best_execution = *execution;
                        }
                    })
                    .or_insert(*execution);
            }
        }

        // Check if the winning solution contains an order whose execution in the
        // winning solution is more than `fairness_threshold` worse than the
        // order's best execution across all solutions
        let unfair = winner
            .solution
            .orders()
            .iter()
            .any(|(uid, winning_execution)| {
                let best_execution = best_executions.get(uid).expect("by construction above");
                let improvement = improvement_in_buy(best_execution, winning_execution);
                if improvement.is_zero() {
                    return false;
                };
                tracing::debug!(
                    ?uid,
                    ?improvement,
                    ?best_execution,
                    ?winning_execution,
                    "fairness check"
                );
                // Improvement is denominated in buy token, use buy price to normalize the
                // difference into eth
                let Some(order) = auction.orders.iter().find(|order| order.uid == *uid) else {
                    // This can happen for jit orders
                    tracing::debug!(?uid, "cannot ensure fairness, order not found in auction");
                    return false;
                };
                let Some(buy_price) = auction.prices.get(&order.buy.token) else {
                    tracing::warn!(
                        ?order,
                        "cannot ensure fairness, buy price not found in auction"
                    );
                    return false;
                };
                buy_price.in_eth(improvement.into()) > fairness_threshold
            });
        !unfair
    }

    /// Sends a `/solve` request to the driver and manages all error cases and
    /// records metrics and logs appropriately.
    async fn solve(
        &self,
        driver: Arc<infra::Driver>,
        request: &solve::Request,
    ) -> Vec<Participant> {
        let start = Instant::now();
        let result = self.try_solve(driver.clone(), request).await;
        let solutions = match result {
            Ok(solutions) => {
                Metrics::solve_ok(&driver, start.elapsed());
                solutions
            }
            Err(err) => {
                Metrics::solve_err(&driver, start.elapsed(), &err);
                if matches!(err, SolveError::NoSolutions) {
                    tracing::debug!(driver = %driver.name, "solver found no solution");
                } else {
                    tracing::warn!(?err, driver = %driver.name, "solve error");
                }
                vec![]
            }
        };

        solutions
            .into_iter()
            .filter_map(|solution| match solution {
                Ok(solution) => {
                    Metrics::solution_ok(&driver.clone());
                    Some(Participant {
                        driver: driver.clone(),
                        solution,
                    })
                }
                Err(err) => {
                    Metrics::solution_err(&driver.clone(), &err);
                    tracing::debug!(?err, driver = %driver.name, "invalid proposed solution");
                    None
                }
            })
            .collect()
    }

    /// Sends `/solve` request to the driver and forwards errors to the caller.
    async fn try_solve(
        &self,
        driver: Arc<infra::Driver>,
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
            let authenticator = self.eth.contracts().authenticator();
            let is_allowed = authenticator.is_solver(solver.into()).call().await;

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

    /// Execute the solver's solution. Returns Ok when the corresponding
    /// transaction has been mined.
    async fn settle(
        &self,
        driver: Arc<infra::Driver>,
        solution_id: u64,
        solved_order_uids: HashSet<OrderUid>,
        auction_id: i64,
        submission_deadline_latest_block: u64,
    ) -> Result<TxId, SettleError> {
        let request = settle::Request {
            solution_id,
            submission_deadline_latest_block,
        };
        let tx_hash = self
            .wait_for_settlement(driver.clone(), auction_id, request)
            .await?;
        tracing::debug!(?tx_hash, "solution settled");

        self.in_flight_orders
            .lock()
            .await
            .retain(|order| !solved_order_uids.contains(order));

        Ok(tx_hash)
    }

    /// Wait for either the settlement transaction to be mined or the driver
    /// returned a result.
    async fn wait_for_settlement(
        &self,
        driver: Arc<infra::Driver>,
        auction_id: i64,
        request: settle::Request,
    ) -> Result<eth::TxId, SettleError> {
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
    ) -> Result<eth::TxId, SettleError> {
        let current = self.eth.current_block().borrow().number;
        let deadline = current.saturating_add(max_blocks_wait);
        tracing::debug!(%current, %deadline, %auction_id, "waiting for tag");
        loop {
            let block = ethrpc::block_stream::next_block(self.eth.current_block()).await;
            // Run maintenance to ensure the system processed the last available block so
            // it's possible to find the tx in the DB in the next line.
            self.run_maintenance(&block).await;

            match self
                .persistence
                .find_settlement_transactions(auction_id)
                .await
            {
                Ok(hashes) if hashes.is_empty() => {}
                Ok(hashes) => {
                    if let Some(hash) = hashes.into_iter().next() {
                        return Ok(hash);
                    }
                }
                Err(err) => {
                    tracing::warn!(?err, "failed to fetch recent settlement tx hashes");
                }
            }
            if block.number >= deadline {
                break;
            }
        }
        Err(SettleError::Failure(anyhow::anyhow!(
            "settlement transaction await reached deadline"
        )))
    }

    /// Removes orders that are currently being settled to avoid solvers trying
    /// to fill an order a second time.
    async fn remove_in_flight_orders(&self, mut auction: domain::Auction) -> domain::Auction {
        let in_flight = &*self.in_flight_orders.lock().await;
        if in_flight.is_empty() {
            return auction;
        };

        auction.orders.retain(|o| !in_flight.contains(&o.uid));
        tracing::debug!(orders = ?in_flight, "filtered out in-flight orders");

        auction
    }
}

struct Participant {
    driver: Arc<infra::Driver>,
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
    reveal: prometheus::HistogramVec,

    /// Tracks the times and results of driver `/settle` requests.
    #[metric(
        labels("driver", "result"),
        buckets(0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 48)
    )]
    settle: prometheus::HistogramVec,

    /// Tracks the number of orders that were part of some but not the winning
    /// solution together with the winning driver that did't include it.
    #[metric(labels("ignored_by"))]
    matched_unsettled: prometheus::IntCounterVec,

    /// Tracks the number of database errors.
    #[metric(labels("error_type"))]
    db_metric_error: prometheus::IntCounterVec,

    /// Tracks the time spent in post-processing after the auction has been
    /// solved and before sending a `settle` request.
    auction_postprocessing_time: prometheus::Histogram,

    /// Tracks the time spent running maintenance. This mostly consists of
    /// indexing new events.
    #[metric(buckets(0.01, 0.05, 0.1, 0.2, 0.5, 1, 1.5, 2, 2.5, 5))]
    service_maintenance_time: prometheus::Histogram,

    /// Total time spent in a single run of the run loop.
    #[metric(buckets(0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 48))]
    single_run_time: prometheus::Histogram,

    /// Total number of blocks mined during the single run loop.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 10, 20, 30, 40, 60, 80, 100, 120))]
    single_run_blocks_count: prometheus::Histogram,

    /// Time difference between the current block and when the single run
    /// function is started.
    #[metric(buckets(0, 0.25, 0.5, 0.75, 1, 1.5, 2, 2.5, 3, 4, 5, 6))]
    current_block_delay: prometheus::Histogram,
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

    fn settle_ok(driver: &infra::Driver, elapsed: Duration) {
        Self::get()
            .settle
            .with_label_values(&[&driver.name, "success"])
            .observe(elapsed.as_secs_f64());
    }

    fn settle_err(driver: &infra::Driver, elapsed: Duration, err: &SettleError) {
        let label = match err {
            SettleError::Failure(_) => "error",
        };
        Self::get()
            .settle
            .with_label_values(&[&driver.name, label])
            .observe(elapsed.as_secs_f64());
    }

    fn matched_unsettled(winning: &infra::Driver, unsettled: HashSet<domain::OrderUid>) {
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

    fn post_processed(elapsed: Duration) {
        Self::get()
            .auction_postprocessing_time
            .observe(elapsed.as_secs_f64());
    }

    fn ran_maintenance(elapsed: Duration) {
        Self::get()
            .service_maintenance_time
            .observe(elapsed.as_secs_f64());
    }

    fn single_run_completed(elapsed: Duration, blocks: Option<u64>) {
        Self::get().single_run_time.observe(elapsed.as_secs_f64());
        if let Some(blocks) = blocks {
            Self::get()
                .single_run_blocks_count
                .observe(blocks.into_f64())
        }
    }

    fn auction_ready(init_block_timestamp: Instant) {
        Self::get()
            .current_block_delay
            .observe(init_block_timestamp.elapsed().as_secs_f64())
    }
}

pub mod observe {
    use {crate::domain, std::collections::HashSet};

    pub fn log_auction_delta(previous: &Option<domain::Auction>, current: &domain::AuctionWithId) {
        let previous_uids = match previous {
            Some(previous) => previous
                .orders
                .iter()
                .map(|order| order.uid)
                .collect::<HashSet<_>>(),
            None => HashSet::new(),
        };
        let current_uids = current
            .auction
            .orders
            .iter()
            .map(|order| order.uid)
            .collect::<HashSet<_>>();
        let added = current_uids.difference(&previous_uids);
        let removed = previous_uids.difference(&current_uids);
        tracing::debug!(
            id = current.id,
            added = ?added,
            "New orders in auction"
        );
        tracing::debug!(
            id = current.id,
            removed = ?removed,
            "Orders no longer in auction"
        );
    }
}
