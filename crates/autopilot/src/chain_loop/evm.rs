//! EvmChain binds the Chain vocabulary to the exact types run_loop.rs uses
//! today and adapts the real components to the seams. Adapters delegate to
//! public infra APIs where that is cheap. Where the real logic only exists
//! as private methods on RunLoop the adapter is a stub with a BLOCKER
//! comment naming the coupling.

use {
    super::{
        AuctionInfo,
        AuctionLoop,
        AuctionProvider,
        Chain,
        CycleTrigger,
        Leadership,
        RankingInfo,
        SettlementExecutor,
        SettlementObserver,
        SolverCompetition,
        WinnerSelection,
    },
    crate::{
        domain,
        domain::competition::{Bid, Unscored, winner_selection},
        infra::{self, solvers::dto::solve},
        leader_lock_tracker::LeaderLockTracker,
        maintenance::{MaintenanceSync, SyncTarget},
        run::Liveness,
        run_loop::{self, Probes, SlotConfig},
        solvable_orders::SolvableOrdersCache,
    },
    alloy::primitives::B256,
    async_trait::async_trait,
    database::order_events::OrderEventLabel,
    eth_domain_types::WrappedNativeToken,
    ethrpc::block_stream::{BlockInfo, CurrentBlockWatcher},
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{
        collections::{HashMap, HashSet},
        num::NonZeroUsize,
        sync::Arc,
        time::Duration,
    },
    tokio::sync::Notify,
};

pub struct EvmChain;

impl Chain for EvmChain {
    type Auction = domain::Auction;
    type AuctionId = domain::auction::Id;
    type OrderUid = domain::OrderUid;
    type Ranking = winner_selection::Ranking;
    type Solution = Bid<Unscored>;
    // block number, run_loop.rs:361
    type SubmissionDeadline = u64;
    type Tip = BlockInfo;
}

impl AuctionInfo<EvmChain> for domain::Auction {
    fn id(&self) -> domain::auction::Id {
        self.id
    }
}

impl RankingInfo<EvmChain> for winner_selection::Ranking {
    fn winner_count(&self) -> usize {
        self.winners().count()
    }

    fn winning_order_uids(&self) -> HashSet<domain::OrderUid> {
        self.winners()
            .flat_map(|bid| bid.solution().order_ids().copied())
            .collect()
    }

    fn considered_order_uids(&self) -> HashSet<domain::OrderUid> {
        self.non_winners()
            .flat_map(|bid| bid.solution().order_ids().copied())
            .collect()
    }
}

/// Wakes on new blocks or new orders, mirroring the listeners RunLoop::new
/// spawns (run_loop.rs:136-141) and the staleness resync of update_caches
/// (run_loop.rs:238-251).
pub struct EvmCycleTrigger {
    blocks: CurrentBlockWatcher,
    wake: Arc<Notify>,
    max_run_loop_delay: Duration,
    prev_hash: Option<B256>,
}

impl EvmCycleTrigger {
    pub fn new(
        eth: &infra::Ethereum,
        persistence: &infra::Persistence,
        max_run_loop_delay: Duration,
    ) -> Self {
        let wake = Arc::new(Notify::new());
        persistence.spawn_order_listener(wake.clone());

        let blocks = eth.current_block().clone();
        {
            // block listener, run_loop.rs:212-223
            let blocks = blocks.clone();
            let wake = wake.clone();
            tokio::spawn(async move {
                loop {
                    ethrpc::block_stream::next_block(&blocks).await;
                    tracing::debug!("received new block");
                    wake.notify_one();
                }
            });
        }

        Self {
            blocks,
            wake,
            max_run_loop_delay,
            prev_hash: None,
        }
    }
}

#[async_trait]
impl CycleTrigger<EvmChain> for EvmCycleTrigger {
    async fn next_cycle(&mut self) -> BlockInfo {
        self.wake.notified().await;

        let current = *self.blocks.borrow();
        let time_since_last_block = current.observed_at.elapsed();
        let block = if time_since_last_block > self.max_run_loop_delay {
            if self.prev_hash.is_some_and(|prev| prev != current.hash) {
                tracing::warn!(
                    missed_by = ?time_since_last_block - self.max_run_loop_delay,
                    "missed optimal auction start, wait for new block"
                );
            }
            ethrpc::block_stream::next_block(&self.blocks).await
        } else {
            current
        };
        // BLOCKER(state sharing): run_loop.rs uses one `last_block` for this
        // warning (run_loop.rs:241) and for the auction dedupe
        // (run_loop.rs:293) and only writes it when the auction repeats.
        // Splitting trigger and dedupe forces two markers, so the warning
        // can fire in cases the old code suppressed.
        self.prev_hash = Some(block.hash);
        block
    }

    fn current_tip(&self) -> BlockInfo {
        *self.blocks.borrow()
    }
}

/// Maintenance cutoff, cache refresh and auction cutting, delegating to the
/// same public components run_loop.rs calls.
pub struct EvmAuctionProvider {
    pub maintenance: MaintenanceSync,
    pub orders_cache: Arc<SolvableOrdersCache>,
    pub persistence: infra::Persistence,
    pub liveness: Arc<Liveness>,
}

#[async_trait]
impl AuctionProvider<EvmChain> for EvmAuctionProvider {
    async fn sync_to_tip(&self, tip: &BlockInfo, is_leader: bool) -> anyhow::Result<()> {
        // wait for essential event indexing, run_loop.rs:253-258
        self.maintenance
            .wait_until_block_processed(SyncTarget::PartiallyProcessed(tip.number))
            .await;

        // refresh the solvable orders cache, run_loop.rs:260-273
        match self.orders_cache.update(tip.number, is_leader).await {
            Ok(()) => {
                self.orders_cache.track_auction_update("success");
                Ok(())
            }
            Err(err) => {
                self.orders_cache.track_auction_update("failure");
                Err(err)
            }
        }
    }

    async fn cut_auction(&self, _tip: &BlockInfo) -> Option<domain::Auction> {
        // run_loop.rs:304-333
        let Some(auction) = self.orders_cache.current_auction().await else {
            tracing::debug!("no current auction");
            return None;
        };
        let id = self
            .persistence
            .get_next_auction_id()
            .await
            .inspect_err(|err| tracing::error!(?err, "failed to get next auction id"))
            .ok()?;

        // always archive the auction because tests use it as readiness probe
        self.persistence.archive_auction(id, &auction);

        if auction.orders.is_empty() {
            // stay healthy over the empty auction optimization,
            // run_loop.rs:320-324
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
}

/// Driver fan out, reimplementing fetch_solutions (run_loop.rs:591-639)
/// against public APIs only.
pub struct EvmSolverCompetition {
    pub drivers: Vec<Arc<infra::Driver>>,
    pub trusted_tokens: AutoUpdatingTokenList,
    pub eth: infra::Ethereum,
    pub min_solve_time: Duration,
    pub slot_config: Option<SlotConfig>,
    pub max_solutions_per_solver: NonZeroUsize,
    pub compress_solve_request: bool,
}

#[async_trait]
impl SolverCompetition<EvmChain> for EvmSolverCompetition {
    async fn solve(&self, auction: &domain::Auction) -> Vec<Bid<Unscored>> {
        let deadline = run_loop::pick_solve_deadline_impl(
            chrono::Utc::now(),
            self.min_solve_time,
            self.slot_config.as_ref(),
            *self.eth.current_block().borrow(),
        );
        let request = solve::Request::new(
            auction,
            &self.trusted_tokens.all(),
            deadline,
            self.compress_solve_request,
        )
        .await;

        let mut bids: Vec<_> = futures::future::join_all(
            self.drivers
                .iter()
                .map(|driver| self.solve_one(driver.clone(), request.clone())),
        )
        .await
        .into_iter()
        .flatten()
        .collect();

        // attribution filter and per solver caps, run_loop.rs:613-634
        let mut counter = HashMap::new();
        bids.retain(|bid| {
            let submission_address = bid.driver().submission_address;
            if bid.solution().solver() != submission_address {
                tracing::warn!(
                    driver = bid.driver().name,
                    ?submission_address,
                    "the solution received is not from the driver submission address"
                );
                return false;
            }
            let count = counter.entry(bid.driver().name.clone()).or_insert(0);
            *count += 1;
            *count <= self.max_solutions_per_solver.get()
        });

        // shuffle so that sorting randomly splits ties, run_loop.rs:637
        bids.shuffle(&mut rand::rng());
        bids
    }
}

impl EvmSolverCompetition {
    /// Simplified try_solve (run_loop.rs:643-723). Same success path,
    /// same deny listing via the onchain authenticator.
    ///
    /// BLOCKER(observability, soft): run_loop.rs:682-699 runs the pair in a
    /// spawned task so it can abort it on timeout and maps every error to a
    /// metrics label through the private run_loop::Metrics struct. The spike
    /// drops the abort and the metrics, promotion has to move Metrics
    /// ownership into this adapter.
    async fn solve_one(
        &self,
        driver: Arc<infra::Driver>,
        request: solve::Request,
    ) -> Vec<Bid<Unscored>> {
        let timeout = request.time_until_deadline();
        let fetch_response = driver.solve(request);
        let check_allowed = self
            .eth
            .contracts()
            .authenticator()
            .isSolver(driver.submission_address);
        let joined = tokio::time::timeout(timeout, async {
            tokio::join!(check_allowed.call(), fetch_response)
        })
        .await;

        let (can_participate, response) = match joined {
            Ok(results) => results,
            Err(_) => {
                tracing::debug!(driver = %driver.name, "solver timed out");
                return vec![];
            }
        };
        match (can_participate, response) {
            (Ok(true), Ok(response)) => response
                .into_domain()
                .into_iter()
                .map(|solution| Bid::new(solution, driver.clone()))
                .collect(),
            (Ok(false), _) | (Err(_), _) => {
                tracing::warn!(driver = %driver.name, "solver is deny listed");
                vec![]
            }
            (_, Err(err)) => {
                tracing::debug!(?err, driver = %driver.name, "solver didn't provide solutions");
                vec![]
            }
        }
    }
}

/// Delegates to the existing arbitrator unchanged.
pub struct EvmWinnerSelection {
    arbitrator: winner_selection::Arbitrator,
}

impl EvmWinnerSelection {
    pub fn new(max_winners: usize, wrapped_native_token: WrappedNativeToken) -> Self {
        Self {
            arbitrator: winner_selection::Arbitrator::new(max_winners, wrapped_native_token),
        }
    }
}

impl WinnerSelection<EvmChain> for EvmWinnerSelection {
    fn arbitrate(
        &self,
        solutions: Vec<Bid<Unscored>>,
        auction: &domain::Auction,
    ) -> winner_selection::Ranking {
        self.arbitrator.arbitrate(solutions, auction)
    }
}

pub struct EvmSettlementExecutor {
    pub submission_deadline_blocks: u64,
    // full delegation additionally needs Ethereum, Persistence,
    // MaintenanceSync, max_settlement_transaction_wait and the driver
    // handles carried inside the ranking
}

#[async_trait]
impl SettlementExecutor<EvmChain> for EvmSettlementExecutor {
    fn submission_deadline(&self, tip: &BlockInfo) -> u64 {
        // run_loop.rs:360-361
        tip.number + self.submission_deadline_blocks
    }

    async fn execute(
        &self,
        _auction_id: domain::auction::Id,
        _ranking: &winner_selection::Ranking,
        _deadline: u64,
    ) {
        // BLOCKER(coupling): the dispatch loop (run_loop.rs:397-409) and the
        // spawned settle future (run_loop.rs:416-464) close over
        // Arc<RunLoop> to call the private methods RunLoop::settle
        // (run_loop.rs:728-787) and RunLoop::wait_for_settlement_transaction
        // (run_loop.rs:861-899). Those weld the driver HTTP call, the block
        // stream, the MaintenanceSync FullyProcessed wait and
        // Persistence::find_settlement_transaction into one select race.
        // Every dependency is public, so promotion is moving roughly 170
        // lines onto this adapter, but they cannot be delegated to in place.
        // Cross seam invariant to preserve: solution_uid is the index in
        // Ranking::enumerated(), shared with save_solutions in
        // post_processing (run_loop.rs:397 and persistence/mod.rs:250).
        unimplemented!("spike: settlement dispatch is welded to private RunLoop methods")
    }
}

/// Order event and competition persistence bookkeeping.
pub struct EvmSettlementObserver {
    pub persistence: infra::Persistence,
}

#[async_trait]
impl SettlementObserver<EvmChain> for EvmSettlementObserver {
    fn orders_ready(&self, auction: &domain::Auction) {
        // run_loop.rs:341-342
        self.persistence
            .store_order_events(auction.orders.iter().map(|o| o.uid), OrderEventLabel::Ready);
    }

    async fn competition_ranked(
        &self,
        _auction: &domain::Auction,
        _tip: &BlockInfo,
        _ranking: &winner_selection::Ranking,
        _deadline: u64,
    ) -> anyhow::Result<()> {
        // BLOCKER(bulk, not coupling): post_processing (run_loop.rs:467-586)
        // reads only public accessors (Ranking::reference_scores, Bid
        // getters, auction fields) and writes through public Persistence
        // methods (save_auction, save_solutions, save_competition,
        // store_fee_policies). The roughly 120 lines of SolverCompetitionDB
        // and Competition DTO assembly move behind this seam verbatim, no
        // visibility changes needed. Skipped here because copying them adds
        // no knowledge.
        unimplemented!("spike: post_processing DTO assembly not copied")
    }

    fn orders_matched(
        &self,
        executing: HashSet<domain::OrderUid>,
        considered: HashSet<domain::OrderUid>,
    ) {
        // run_loop.rs:379-395
        self.persistence
            .store_order_events(executing, OrderEventLabel::Executing);
        self.persistence
            .store_order_events(considered, OrderEventLabel::Considered);
    }

    fn competition_ended(&self, auction: &domain::Auction, ranking: &winner_selection::Ranking) {
        // run_loop.rs:411
        run_loop::observe::unsettled(ranking, auction);
    }
}

/// Wraps the existing tracker. Option because release consumes the tracker.
pub struct EvmLeadership(Option<LeaderLockTracker>);

impl EvmLeadership {
    pub fn new(tracker: LeaderLockTracker) -> Self {
        Self(Some(tracker))
    }
}

#[async_trait]
impl Leadership for EvmLeadership {
    async fn try_acquire(&mut self) -> bool {
        match self.0.as_mut() {
            Some(tracker) => {
                tracker.try_acquire().await;
                tracker.is_leader()
            }
            None => false,
        }
    }

    async fn release(&mut self) {
        if let Some(tracker) = self.0.take() {
            tracker.release().await;
        }
    }
}

impl AuctionLoop<EvmChain> {
    /// Builds the generic loop from the same inputs RunLoop::new receives
    /// (run_loop.rs:121-155 wired by run.rs:617-629). Proves the existing
    /// wiring maps onto the seams one to one.
    #[expect(clippy::too_many_arguments)]
    pub async fn from_evm_components(
        config: run_loop::Config,
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        solvable_orders_cache: Arc<SolvableOrdersCache>,
        trusted_tokens: AutoUpdatingTokenList,
        probes: Probes,
        maintenance: MaintenanceSync,
    ) -> Self {
        // leader lock setup, run_loop.rs:162-172
        let leader = if config.enable_leader_lock {
            Some(persistence.leader("autopilot_startup".to_string()).await)
        } else {
            None
        };
        let leadership = EvmLeadership::new(LeaderLockTracker::new(leader));

        let max_winners = config.max_winners_per_auction.get();
        let weth = eth.contracts().wrapped_native_token();

        AuctionLoop::new(
            Box::new(EvmCycleTrigger::new(
                &eth,
                &persistence,
                config.max_run_loop_delay,
            )),
            Box::new(EvmAuctionProvider {
                maintenance,
                orders_cache: solvable_orders_cache,
                persistence: persistence.clone(),
                liveness: probes.liveness.clone(),
            }),
            Box::new(EvmSolverCompetition {
                drivers,
                trusted_tokens,
                eth: eth.clone(),
                min_solve_time: config.min_solve_time,
                slot_config: config.sync_solve_deadline_to_blockchain,
                max_solutions_per_solver: config.max_solutions_per_solver,
                compress_solve_request: config.compress_solve_request,
            }),
            Box::new(EvmWinnerSelection::new(max_winners, weth)),
            Box::new(EvmSettlementExecutor {
                submission_deadline_blocks: config.submission_deadline,
            }),
            Box::new(EvmSettlementObserver { persistence }),
            Box::new(leadership),
            probes,
        )
    }
}
