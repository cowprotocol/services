use {
    crate::{
        database::competition::Competition,
        domain::{
            self,
            OrderUid,
            auction::Id,
            competition::{
                self,
                Solution,
                SolutionError,
                SolverParticipationGuard,
                Unranked,
                winner_selection::{self, Ranking},
            },
            eth::{self, TxId},
            settlement::{ExecutionEnded, ExecutionStarted},
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
    ethrpc::block_stream::BlockInfo,
    futures::{FutureExt, TryFutureExt},
    itertools::Itertools,
    model::solver_competition::{
        CompetitionAuction,
        Order,
        Score,
        SolverCompetitionDB,
        SolverSettlement,
    },
    num::ToPrimitive,
    primitive_types::H256,
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{
        collections::{HashMap, HashSet},
        num::NonZeroUsize,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::Mutex,
    tracing::{Instrument, instrument},
};

pub struct Config {
    pub submission_deadline: u64,
    pub max_settlement_transaction_wait: Duration,
    pub solve_deadline: Duration,
    /// How much time past observing the current block the runloop is
    /// allowed to start before it has to re-synchronize to the blockchain
    /// by waiting for the next block to appear.
    pub max_run_loop_delay: Duration,
    pub max_winners_per_auction: NonZeroUsize,
    pub max_solutions_per_solver: NonZeroUsize,
}

pub struct RunLoop {
    config: Config,
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    drivers: Vec<Arc<infra::Driver>>,
    solver_participation_guard: SolverParticipationGuard,
    solvable_orders_cache: Arc<SolvableOrdersCache>,
    trusted_tokens: AutoUpdatingTokenList,
    in_flight_orders: Arc<Mutex<HashSet<OrderUid>>>,
    liveness: Arc<Liveness>,
    /// Maintenance tasks that should run before every runloop to have
    /// the most recent data available.
    maintenance: Arc<Maintenance>,
    competition_updates_sender: tokio::sync::mpsc::UnboundedSender<()>,
    winner_selection: winner_selection::Arbitrator,
}

impl RunLoop {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Config,
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        solver_participation_guard: SolverParticipationGuard,
        solvable_orders_cache: Arc<SolvableOrdersCache>,
        trusted_tokens: AutoUpdatingTokenList,
        liveness: Arc<Liveness>,
        maintenance: Arc<Maintenance>,
        competition_updates_sender: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> Self {
        let max_winners = config.max_winners_per_auction.get();
        let weth = eth.contracts().wrapped_native_token();

        Self {
            config,
            eth,
            persistence,
            drivers,
            solver_participation_guard,
            solvable_orders_cache,
            trusted_tokens,
            in_flight_orders: Default::default(),
            liveness,
            maintenance,
            competition_updates_sender,
            winner_selection: winner_selection::Arbitrator { max_winners, weth },
        }
    }

    pub async fn run_forever(self) -> ! {
        Maintenance::spawn_cow_amm_indexing_task(
            self.maintenance.clone(),
            self.eth.current_block().clone(),
        );
        let mut last_auction = None;
        let mut last_block = None;
        let self_arc = Arc::new(self);
        loop {
            let auction = self_arc
                .next_auction(&mut last_auction, &mut last_block)
                .await;
            if let Some(auction) = auction {
                let auction_id = auction.id;
                self_arc
                    .single_run(auction)
                    .instrument(tracing::info_span!("auction", auction_id))
                    .await
            };
        }
    }

    /// Sleeps until the next auction is supposed to start, builds it and
    /// returns it.
    #[instrument(skip(self, prev_auction), fields(prev_auction = prev_auction.as_ref().map(|a| a.id)))]
    async fn next_auction(
        &self,
        prev_auction: &mut Option<domain::Auction>,
        prev_block: &mut Option<H256>,
    ) -> Option<domain::Auction> {
        // wait for appropriate time to start building the auction
        let start_block = {
            let current_block = *self.eth.current_block().borrow();
            let time_since_last_block = current_block.observed_at.elapsed();
            let auction_block = if time_since_last_block > self.config.max_run_loop_delay {
                if prev_block.is_some_and(|prev_block| prev_block != current_block.hash) {
                    // don't emit warning if we finished prev run loop within the same block
                    tracing::warn!(
                        missed_by = ?time_since_last_block - self.config.max_run_loop_delay,
                        "missed optimal auction start, wait for new block"
                    );
                }
                ethrpc::block_stream::next_block(self.eth.current_block()).await
            } else {
                current_block
            };

            self.run_maintenance(&auction_block).await;
            match self
                .solvable_orders_cache
                .update(auction_block.number)
                .await
            {
                Ok(()) => {
                    tracing::trace!("solvable orders cache updated");
                    self.solvable_orders_cache.track_auction_update("success");
                }
                Err(err) => {
                    self.solvable_orders_cache.track_auction_update("failure");
                    tracing::warn!(?err, "failed to update auction");
                }
            }
            auction_block
        };

        let auction = self.cut_auction().await?;
        tracing::trace!(auction_id = ?auction.id, "auction cut");

        // Only run the solvers if the auction or block has changed.
        let previous = prev_auction.replace(auction.clone());
        if previous.as_ref() == Some(&auction)
            && prev_block.replace(start_block.hash) == Some(start_block.hash)
        {
            return None;
        }

        observe::log_auction_delta(&previous, &auction);
        self.liveness.auction();
        Metrics::auction_ready(start_block.observed_at);
        Some(auction)
    }

    /// Runs maintenance on all components to ensure the system uses
    /// the latest available state.
    async fn run_maintenance(&self, block: &BlockInfo) {
        let start = Instant::now();
        self.maintenance.update(block).await;
        Metrics::ran_maintenance(start.elapsed());
    }

    async fn cut_auction(&self) -> Option<domain::Auction> {
        let auction = match self.solvable_orders_cache.current_auction().await {
            Some(auction) => auction,
            None => {
                tracing::debug!("no current auction");
                return None;
            }
        };
        let auction = self.remove_in_flight_orders(auction).await;

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

        Some(domain::Auction {
            id,
            block: auction.block,
            orders: auction.orders,
            prices: auction.prices,
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners,
        })
    }

    #[instrument(skip_all, fields(auction_id = auction.id, auction_block = auction.block, auction_orders = auction.orders.len()))]
    async fn single_run(self: &Arc<Self>, auction: domain::Auction) {
        let single_run_start = Instant::now();
        tracing::info!(auction_id = ?auction.id, "solving");

        // Mark all auction orders as `Ready` for competition
        self.persistence
            .store_order_events(auction.orders.iter().map(|o| o.uid), OrderEventLabel::Ready);
        tracing::trace!(auction_id = ?auction.id, "orders marked as ready");

        // Collect valid solutions from all drivers
        let solutions = self.fetch_solutions(&auction).await;
        observe::solutions(&solutions);
        if solutions.is_empty() {
            return;
        }

        let ranking = self.winner_selection.arbitrate(solutions, &auction);

        // Count and record the number of winners
        let num_winners = ranking.winners().count();
        if let Some(num_winners_f64) = num_winners.to_f64() {
            Metrics::get().auction_winners.observe(num_winners_f64);
        }

        let competition_simulation_block = self.eth.current_block().borrow().number;
        let block_deadline = competition_simulation_block + self.config.submission_deadline;

        // Post-processing should not be executed asynchronously since it includes steps
        // of storing all the competition/auction-related data to the DB.
        if let Err(err) = self
            .post_processing(
                &auction,
                competition_simulation_block,
                &ranking,
                block_deadline,
                &self.winner_selection,
            )
            .await
        {
            tracing::error!(?err, "failed to post-process competition");
            return;
        }
        tracing::trace!(auction_id = ?auction.id, "post-processing completed");

        // Mark all winning orders as `Executing`
        let winning_orders = ranking
            .winners()
            .flat_map(|p| p.solution().order_ids().copied())
            .collect::<HashSet<_>>();
        self.persistence
            .store_order_events(winning_orders.clone(), OrderEventLabel::Executing);

        // Mark the rest as `Considered` for execution
        self.persistence.store_order_events(
            ranking
                .non_winners()
                .flat_map(|p| p.solution().order_ids().copied())
                .filter(|order_id| !winning_orders.contains(order_id)),
            OrderEventLabel::Considered,
        );
        tracing::trace!(auction_id = ?auction.id, "orders marked as considered");

        for (solution_uid, winner) in ranking
            .enumerated()
            .filter(|(_, participant)| participant.is_winner())
        {
            let (driver, solution) = (winner.driver(), winner.solution());
            tracing::info!(driver = %driver.name, solution = %solution.id(), "winner");

            self.start_settlement_execution(
                auction.id,
                single_run_start,
                driver,
                solution,
                solution_uid,
                block_deadline,
            )
            .await;
        }
        tracing::trace!(auction_id = ?auction.id, "settlement execution started");
        observe::unsettled(&ranking, &auction);
    }

    /// Starts settlement execution in a background task. The function is async
    /// only to get access to the locks.
    async fn start_settlement_execution(
        self: &Arc<Self>,
        auction_id: Id,
        single_run_start: Instant,
        driver: &Arc<infra::Driver>,
        solution: &Solution,
        solution_uid: usize,
        block_deadline: u64,
    ) {
        let solved_order_uids: HashSet<_> = solution.orders().keys().cloned().collect();
        self.in_flight_orders
            .lock()
            .await
            .extend(solved_order_uids.clone());

        let solution_id = solution.id();
        let solver = solution.solver();
        let self_ = self.clone();
        let driver_ = driver.clone();

        let settle_fut = async move {
            tracing::info!(driver = %driver_.name, solution = %solution_id, "settling");
            let submission_start = Instant::now();

            match self_
                .settle(
                    &driver_,
                    solved_order_uids.clone(),
                    solver,
                    auction_id,
                    solution_id,
                    solution_uid,
                    block_deadline,
                )
                .await
            {
                Ok(tx_hash) => {
                    Metrics::settle_ok(
                        &driver_,
                        solved_order_uids.len(),
                        submission_start.elapsed(),
                    );
                    tracing::debug!(?tx_hash, driver = %driver_.name, ?solver, "solution settled");
                }
                Err(err) => {
                    Metrics::settle_err(&driver_, submission_start.elapsed(), &err);
                    tracing::warn!(?err, driver = %driver_.name, "settlement failed");
                }
            }
            Metrics::single_run_completed(single_run_start.elapsed());
        }
        .instrument(tracing::Span::current());

        tokio::spawn(settle_fut);
    }

    #[instrument(skip_all)]
    async fn post_processing(
        &self,
        auction: &domain::Auction,
        competition_simulation_block: u64,
        ranking: &Ranking,
        block_deadline: u64,
        winner_selection: &winner_selection::Arbitrator,
    ) -> Result<()> {
        let start = Instant::now();
        let reference_scores = winner_selection.compute_reference_scores(ranking);

        let participants = ranking
            .all()
            .map(|participant| participant.solution().solver().into())
            .collect::<HashSet<_>>();
        let mut fee_policies = Vec::new();
        for order_id in ranking
            .ranked()
            .flat_map(|participant| participant.solution().order_ids())
            .unique()
        {
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

        let mut solutions: Vec<_> = ranking
            .enumerated()
            .map(|(index, participant)| SolverSettlement {
                solver: participant.driver().name.clone(),
                solver_address: participant.solution().solver().0,
                score: Some(Score::Solver(participant.solution().score().get().0)),
                ranking: index + 1,
                orders: participant
                    .solution()
                    .orders()
                    .iter()
                    .map(|(id, order)| Order::Colocated {
                        id: (*id).into(),
                        sell_amount: order.executed_sell.into(),
                        buy_amount: order.executed_buy.into(),
                    })
                    .collect(),
                clearing_prices: participant
                    .solution()
                    .prices()
                    .iter()
                    .map(|(token, price)| (token.0, price.get().into()))
                    .collect(),
                is_winner: participant.is_winner(),
                filtered_out: participant.filtered_out(),
            })
            .collect();
        // reverse as solver competition table is sorted from worst to best,
        // so we need to keep the ordering for backwards compatibility
        solutions.reverse();

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
            solutions,
        };
        let competition = Competition {
            auction_id: auction.id,
            reference_scores,
            participants,
            prices: auction
                .prices
                .clone()
                .into_iter()
                .map(|(key, value)| (key.into(), value.get().into()))
                .collect(),
            block_deadline,
            competition_simulation_block,
            competition_table,
        };

        match futures::try_join!(
            self.persistence
                .save_auction(auction, block_deadline)
                .map_err(|e| e.0.context("failed to save auction")),
            self.persistence
                .save_solutions(auction.id, ranking.all())
                .map_err(|e| e.0.context("failed to save solutions")),
        ) {
            Ok(_) => {
                // Notify the solver participation guard that the proposed solutions have been
                // saved.
                if let Err(err) = self.competition_updates_sender.send(()) {
                    tracing::error!(?err, "failed to notify solver participation guard");
                }
            }
            Err(err) => {
                // Don't error if saving of auction and solution fails, until stable.
                // Various edge cases with JIT orders verifiable only in production.
                tracing::warn!(?err, "failed to save new competition data");
            }
        }
        tracing::trace!(auction_id = ?auction.id, "auction saved");

        tracing::trace!(?competition, "saving competition");
        futures::try_join!(
            self.persistence
                .save_competition(&competition)
                .map_err(|e| e.0.context("failed to save competition")),
            self.persistence
                .save_surplus_capturing_jit_order_owners(
                    auction.id,
                    &auction.surplus_capturing_jit_order_owners,
                )
                .map_err(|e| e.0.context("failed to save jit order owners")),
            self.persistence
                .store_fee_policies(auction.id, fee_policies)
                .map_err(|e| e.context("failed to fee_policies")),
        )?;
        tracing::trace!(auction_id = ?auction.id, "competition saved");

        Metrics::post_processed(start.elapsed());
        Ok(())
    }

    /// Runs the solver competition, making all configured drivers participate.
    /// Returns all fair solutions sorted by their score (best to worst).
    #[instrument(skip_all)]
    async fn fetch_solutions(
        &self,
        auction: &domain::Auction,
    ) -> Vec<competition::Participant<Unranked>> {
        let request = solve::Request::new(
            auction,
            &self.trusted_tokens.all(),
            self.config.solve_deadline,
        );

        let mut solutions = futures::future::join_all(
            self.drivers
                .iter()
                .map(|driver| self.solve(driver.clone(), request.clone())),
        )
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        let mut counter = HashMap::new();
        solutions.retain(|participant| {
            let submission_address = participant.driver().submission_address;
            let is_solution_from_driver = participant.solution().solver() == submission_address;

            // Filter out solutions that don't come from their corresponding submission
            // address
            if !is_solution_from_driver {
                tracing::warn!(
                    driver = participant.driver().name,
                    ?submission_address,
                    "the solution received is not from the driver submission address"
                );
                return false;
            }

            // limit number of solutions per solver
            let driver = participant.driver().name.clone();
            let count = counter.entry(driver).or_insert(0);
            *count += 1;
            *count <= self.config.max_solutions_per_solver.get()
        });

        // Shuffle so that sorting randomly splits ties.
        solutions.shuffle(&mut rand::thread_rng());
        solutions
    }

    /// Sends a `/solve` request to the driver and manages all error cases and
    /// records metrics and logs appropriately.
    #[instrument(skip_all, fields(driver = driver.name))]
    async fn solve(
        &self,
        driver: Arc<infra::Driver>,
        request: solve::Request,
    ) -> Vec<competition::Participant<Unranked>> {
        let start = Instant::now();
        let result = self.try_solve(Arc::clone(&driver), request).await;
        let solutions = match result {
            Ok(solutions) => {
                Metrics::solve_ok(&driver, start.elapsed());
                solutions
            }
            Err(err) => {
                Metrics::solve_err(&driver, start.elapsed(), &err);
                tracing::debug!(?err, driver = %driver.name, "solver didn't provide solutions");
                vec![]
            }
        };

        solutions
            .into_iter()
            .filter_map(|solution| match solution {
                Ok(solution) => {
                    Metrics::solution_ok(&driver);
                    Some(competition::Participant::new(solution, driver.clone()))
                }
                Err(err) => {
                    Metrics::solution_err(&driver, &err);
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
        request: solve::Request,
    ) -> Result<Vec<Result<competition::Solution, domain::competition::SolutionError>>, SolveError>
    {
        let (can_participate, response) = {
            let driver = driver.clone();
            let guard = self.solver_participation_guard.clone();
            let mut handle = tokio::task::spawn(async move {
                let fetch_response = driver.solve(request);
                let check_allowed = guard.can_participate(&driver.submission_address);
                tokio::join!(check_allowed, fetch_response)
            });
            tokio::time::timeout(self.config.solve_deadline, &mut handle)
                .await
                .map_err(|_| {
                    // Abort the background task to prevent memory leaks
                    handle.abort();
                    SolveError::Timeout
                })?
                .context("could not finish the task")
                .map_err(SolveError::Failure)?
        };

        let response = match (can_participate, response) {
            (Ok(true), Ok(response)) => response,
            (Ok(false), _) => return Err(SolveError::SolverDenyListed),
            (Err(err), _) => {
                tracing::error!(
                    ?err,
                    driver = %driver.name,
                    ?driver.submission_address,
                    "solver participation check failed"
                );
                return Err(SolveError::SolverDenyListed);
            }
            (_, Err(err)) => return Err(SolveError::Failure(err)),
        };

        if response.solutions.is_empty() {
            return Err(SolveError::NoSolutions);
        }
        Ok(response.into_domain())
    }

    /// Execute the solver's solution. Returns Ok when the corresponding
    /// transaction has been mined.
    #[allow(clippy::too_many_arguments)]
    async fn settle(
        &self,
        driver: &infra::Driver,
        solved_order_uids: HashSet<OrderUid>,
        solver: eth::Address,
        auction_id: i64,
        solution_id: u64,
        solution_uid: usize,
        submission_deadline_latest_block: u64,
    ) -> Result<TxId, SettleError> {
        let settle = async move {
            let current_block = self.eth.current_block().borrow().number;
            anyhow::ensure!(
                current_block < submission_deadline_latest_block,
                "submission deadline was missed"
            );

            let request = settle::Request {
                solution_id,
                submission_deadline_latest_block,
                auction_id,
            };

            self.store_execution_started(
                auction_id,
                solver,
                solution_uid,
                current_block,
                submission_deadline_latest_block,
            );
            driver
                .settle(&request, self.config.max_settlement_transaction_wait)
                .await
        }
        .boxed();

        let wait_for_settlement_transaction = self
            .wait_for_settlement_transaction(auction_id, solver, submission_deadline_latest_block)
            .boxed();

        // Wait for either the settlement transaction to be mined or the driver returned
        // a result.
        let result = match futures::future::select(wait_for_settlement_transaction, settle).await {
            futures::future::Either::Left((res, _)) => res,
            futures::future::Either::Right((driver_result, wait_for_settlement_transaction)) => {
                match driver_result {
                    Ok(_) => wait_for_settlement_transaction.await,
                    Err(err) => Err(SettleError::Other(err)),
                }
            }
        };

        self.store_execution_ended(solver, auction_id, solution_uid, &result);

        // Clean up the in-flight orders regardless the result.
        self.in_flight_orders
            .lock()
            .await
            .retain(|order| !solved_order_uids.contains(order));

        result
    }

    /// Stores settlement execution started event in the DB in a background task
    /// to not block the runloop.
    fn store_execution_started(
        &self,
        auction_id: i64,
        solver: eth::Address,
        solution_uid: usize,
        start_block: u64,
        deadline_block: u64,
    ) {
        let persistence = self.persistence.clone();
        tokio::spawn(async move {
            let execution_started = ExecutionStarted {
                auction_id,
                solver,
                solution_uid,
                start_timestamp: chrono::Utc::now(),
                start_block,
                deadline_block,
            };

            if let Err(err) = persistence
                .store_settlement_execution_started(execution_started)
                .await
            {
                tracing::error!(?err, "failed to store settlement execution event");
            }
        });
    }

    /// Stores settlement execution ended event in the DB in a background task
    /// to not block the runloop.
    fn store_execution_ended(
        &self,
        solver: eth::Address,
        auction_id: i64,
        solution_uid: usize,
        result: &Result<TxId, SettleError>,
    ) {
        let end_timestamp = chrono::Utc::now();
        let current_block = self.eth.current_block().borrow().number;
        let persistence = self.persistence.clone();
        let outcome = match result {
            Ok(_) => "success".to_string(),
            Err(SettleError::Timeout) => "timeout".to_string(),
            Err(SettleError::Other(err)) => format!("driver failed: {err}"),
        };

        tokio::spawn(async move {
            let execution_ended = ExecutionEnded {
                auction_id,
                solver,
                solution_uid,
                end_timestamp,
                end_block: current_block,
                outcome,
            };
            if let Err(err) = persistence
                .store_settlement_execution_ended(execution_ended)
                .await
            {
                tracing::error!(?err, "failed to update settlement execution event");
            }
        });
    }

    /// Tries to find a `settle` contract call with calldata ending in `tag` and
    /// originated from the `solver`.
    ///
    /// Returns None if no transaction was found within the deadline or the task
    /// is cancelled.
    #[instrument(skip_all)]
    async fn wait_for_settlement_transaction(
        &self,
        auction_id: i64,
        solver: eth::Address,
        submission_deadline_latest_block: u64,
    ) -> Result<eth::TxId, SettleError> {
        let current = self.eth.current_block().borrow().number;
        tracing::debug!(%current, deadline=%submission_deadline_latest_block, %auction_id, "waiting for tag");
        loop {
            let block = ethrpc::block_stream::next_block(self.eth.current_block()).await;
            // Run maintenance to ensure the system processed the last available block so
            // it's possible to find the tx in the DB in the next line.
            self.run_maintenance(&block).await;

            match self
                .persistence
                .find_settlement_transaction(auction_id, solver)
                .await
            {
                Ok(Some(transaction)) => return Ok(transaction),
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        ?auction_id,
                        ?solver,
                        "failed to find settlement transaction"
                    );
                }
            }
            if block.number >= submission_deadline_latest_block {
                break;
            }
        }
        Err(SettleError::Timeout)
    }

    /// Removes orders that are currently being settled to avoid solvers trying
    /// to fill an order a second time.
    async fn remove_in_flight_orders(
        &self,
        mut auction: domain::RawAuctionData,
    ) -> domain::RawAuctionData {
        let in_flight = &*self.in_flight_orders.lock().await;
        if in_flight.is_empty() {
            return auction;
        };

        auction.orders.retain(|o| !in_flight.contains(&o.uid));
        auction
            .surplus_capturing_jit_order_owners
            .retain(|owner| !in_flight.iter().any(|i| i.owner() == *owner));
        tracing::debug!(
            orders = ?in_flight,
            "filtered out in-flight orders and surplus_capturing_jit_order_owners"
        );

        auction
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
    #[error("the solver got deny listed")]
    SolverDenyListed,
}

#[derive(Debug, thiserror::Error)]
enum SettleError {
    #[error(transparent)]
    Other(anyhow::Error),
    #[error("settlement transaction await reached deadline")]
    Timeout,
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "runloop")]
struct Metrics {
    /// Tracks the last executed auction.
    auction: prometheus::IntGauge,

    /// Tracks the number of winners per auction.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10))]
    auction_winners: prometheus::Histogram,

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
    /// solutions.
    matched_unsettled: prometheus::IntCounter,

    /// Tracks the number of orders that were settled together with the
    /// settling driver.
    #[metric(labels("driver"))]
    settled: prometheus::IntCounterVec,

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
            SolveError::SolverDenyListed => "deny_listed",
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

    fn settle_ok(driver: &infra::Driver, settled_order_count: usize, elapsed: Duration) {
        Self::get()
            .settle
            .with_label_values(&[&driver.name, "success"])
            .observe(elapsed.as_secs_f64());
        Self::get()
            .settled
            .with_label_values(&[&driver.name])
            .inc_by(settled_order_count.try_into().unwrap_or(u64::MAX));
    }

    fn settle_err(driver: &infra::Driver, elapsed: Duration, err: &SettleError) {
        let label = match err {
            SettleError::Other(_) => "error",
            SettleError::Timeout => "timeout",
        };
        Self::get()
            .settle
            .with_label_values(&[&driver.name, label])
            .observe(elapsed.as_secs_f64());
    }

    fn matched_unsettled(unsettled: HashSet<&domain::OrderUid>) {
        if !unsettled.is_empty() {
            tracing::debug!(?unsettled, "some orders were matched but not settled");
        }
        Self::get().matched_unsettled.inc_by(unsettled.len() as u64);
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

    fn single_run_completed(elapsed: Duration) {
        Self::get().single_run_time.observe(elapsed.as_secs_f64());
    }

    fn auction_ready(init_block_timestamp: Instant) {
        Self::get()
            .current_block_delay
            .observe(init_block_timestamp.elapsed().as_secs_f64())
    }
}

pub mod observe {
    use {
        crate::domain::{
            self,
            competition::{Unranked, winner_selection::Ranking},
        },
        std::collections::HashSet,
    };

    pub fn log_auction_delta(previous: &Option<domain::Auction>, current: &domain::Auction) {
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

    pub fn solutions(solutions: &[domain::competition::Participant<Unranked>]) {
        if solutions.is_empty() {
            tracing::info!("no solutions for auction");
        }
        for participant in solutions {
            tracing::debug!(
                driver = %participant.driver().name,
                orders = ?participant.solution().order_ids(),
                solution = %participant.solution().id(),
                "proposed solution"
            );
        }
    }

    /// Records metrics for the matched but unsettled orders.
    pub fn unsettled(ranking: &Ranking, auction: &domain::Auction) {
        let mut non_winning_orders = {
            let winning_orders = ranking
                .winners()
                .flat_map(|p| p.solution().order_ids())
                .collect::<HashSet<_>>();
            ranking
                .ranked()
                .flat_map(|p| p.solution().order_ids())
                .filter(|uid| !winning_orders.contains(uid))
                .collect::<HashSet<_>>()
        };
        // Report orders that were part of a non-winning solution candidate
        // but only if they were part of the auction (filter out jit orders)
        let auction_uids = auction.orders.iter().map(|o| o.uid).collect::<HashSet<_>>();
        non_winning_orders.retain(|uid| auction_uids.contains(uid));
        super::Metrics::matched_unsettled(non_winning_orders);
    }
}
