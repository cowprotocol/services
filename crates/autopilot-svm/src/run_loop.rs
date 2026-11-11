//! The chain-generic auction run loop.
//!
//! One cycle runs a fixed sequence: wait for a new chain tip, refresh the
//! order caches, cut an auction from the open orders, skip it if nothing
//! changed since the previous cycle, fan it out to the solvers, rank their
//! solutions, persist the competition outcome, dispatch the winners for
//! settlement, and report the results.
//!
//! [`AuctionLoop`] owns that sequence and the auction dedupe, and it is the
//! only concrete code here. Everything chain-specific enters through traits:
//! [`Cycle`] names the handful of types the loop must store and compare (the
//! tip, the auction, order ids, solutions, the ranking), and [`CycleTrigger`],
//! [`AuctionProvider`], [`SolverCompetition`], [`WinnerSelection`],
//! [`SettlementExecutor`], and [`SettlementObserver`] carry the per-chain
//! behaviour behind each phase. Leadership is chain-agnostic: with a
//! database leader lock, only the instance holding it cuts auctions, and the
//! others only sync to the tip so one can take over at once.

use {
    crate::{leader_lock_tracker::LeaderLockTracker, run::Liveness},
    async_trait::async_trait,
    database::leader_pg_lock::LeaderLock,
    std::{
        collections::HashSet,
        fmt::Debug,
        hash::Hash,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
    tokio_util::sync::CancellationToken,
    tracing::Instrument,
};

/// Type vocabulary of one settlement chain: the associated types the loop
/// itself has to name.
pub trait Cycle: Sized + Send + Sync + 'static {
    /// Chain progress marker (EVM block, Solana slot).
    type Tip: Clone + PartialEq + Debug + Send + Sync + 'static;

    /// Order id. After ranking, the loop partitions the ranked orders into
    /// two sets for order-event bookkeeping: orders of winning solutions
    /// (reported as executing) and orders that only appear in ranked
    /// non-winning solutions (reported as considered). Set membership is why
    /// the id must be `Eq + Hash`.
    type OrderUid: Copy + Eq + Hash + Debug + Send + Sync + 'static;

    /// The cut auction fanned out to solvers. Its `PartialEq` drives the
    /// dedupe, so it must ignore the allocated id.
    type Auction: AuctionInfo + Clone + PartialEq + Send + Sync + 'static;

    /// One solution proposed by one driver, opaque to the loop.
    type Solution: Send + 'static;

    /// Winner selection output over all solutions of one auction.
    type Ranking: RankingInfo<Self> + Send + Sync + 'static;

    /// Submission cutoff for settlements ranked at the given tip, the chain's
    /// own arithmetic over the loop's configured allowance.
    fn submission_deadline(tip: &Self::Tip, allowance: u64) -> u64;
}

/// What the loop needs to know about an auction.
pub trait AuctionInfo {
    fn id(&self) -> i64;
}

/// What the loop needs to know about a ranking.
pub trait RankingInfo<C: Cycle> {
    /// Number of winning solutions.
    fn winner_count(&self) -> usize;

    /// Orders of all winning solutions, marked Executing.
    fn winning_order_uids(&self) -> HashSet<C::OrderUid>;

    /// Orders of ranked non-winning solutions, marked Considered.
    fn considered_order_uids(&self) -> HashSet<C::OrderUid>;
}

/// Yields the tip to build the next auction on.
#[async_trait]
pub trait CycleTrigger<C: Cycle>: Send {
    /// Blocks until a new cycle is warranted, returns the tip to build on.
    async fn next_cycle(&mut self) -> C::Tip;

    /// Latest observed tip, without blocking.
    fn current_tip(&self) -> C::Tip;
}

/// Produces the cut auction for a tip.
#[async_trait]
pub trait AuctionProvider<C: Cycle>: Send + Sync {
    /// Brings the order caches up to date with the tip. An error does not
    /// stop the cycle, the auction is then cut from slightly stale caches.
    async fn sync_to_tip(&self, tip: &C::Tip) -> anyhow::Result<()>;

    /// Cuts the auction for the tip.
    async fn cut_auction(&self, tip: &C::Tip) -> NextAuction<C::Auction>;
}

/// The result of an auction cut.
pub enum NextAuction<A> {
    /// The auction to solve.
    Auction(A),
    /// The cut read the orders, and no order can go into the auction.
    Empty,
    /// The order data lags behind the tip, so the cut waits for it to catch
    /// up.
    Stale,
    /// The cut stopped before it could make an auction because a read or a
    /// write failed.
    Failed,
}

/// Fans the auction out to all drivers and returns attributable solutions.
#[async_trait]
pub trait SolverCompetition<C: Cycle>: Send + Sync {
    async fn solve(&self, auction: &C::Auction) -> Vec<C::Solution>;
}

/// Scores, filters and ranks solutions, marking winners.
pub trait WinnerSelection<C: Cycle>: Send + Sync {
    fn arbitrate(&self, solutions: Vec<C::Solution>, auction: &C::Auction) -> C::Ranking;
}

/// Dispatches winning solutions for execution.
#[async_trait]
pub trait SettlementExecutor<C: Cycle>: Send + Sync {
    /// Dispatches every winner, from the tip the ranking was made at until
    /// the deadline. Implementations submit in the background, the loop does
    /// not wait for settlement results.
    async fn execute(&self, auction_id: i64, ranking: &C::Ranking, tip: &C::Tip, deadline: u64);
}

/// Competition bookkeeping around the settlement outcome.
#[async_trait]
pub trait SettlementObserver<C: Cycle>: Send + Sync {
    /// All auction orders entered the competition.
    fn on_orders_ready(&self, auction: &C::Auction);

    /// Persists the competition outcome (auction, solutions, scores, fees).
    /// An error aborts the cycle before any settlement is dispatched.
    async fn persist_competition_ranking(
        &self,
        auction: &C::Auction,
        tip: &C::Tip,
        ranking: &C::Ranking,
        deadline: u64,
    ) -> anyhow::Result<()>;

    /// Orders of winning solutions are executing, other ranked orders are
    /// considered.
    fn on_orders_matched(&self, executing: HashSet<C::OrderUid>, considered: HashSet<C::OrderUid>);

    /// Final per-cycle reporting after settlements were dispatched.
    fn on_competition_ended(&self, auction: &C::Auction, ranking: &C::Ranking);
}

/// The state that the metrics server gives to the Kubernetes probes.
pub struct Probes {
    pub liveness: Arc<Liveness>,
    pub startup: Arc<Option<AtomicBool>>,
}

/// Chain generic auction loop. Owns the seams and the dedupe state, drives
/// the phases in a fixed order.
pub struct AuctionLoop<C: Cycle> {
    trigger: Box<dyn CycleTrigger<C>>,
    provider: Box<dyn AuctionProvider<C>>,
    competition: Box<dyn SolverCompetition<C>>,
    winner_selection: Box<dyn WinnerSelection<C>>,
    executor: Box<dyn SettlementExecutor<C>>,
    observer: Box<dyn SettlementObserver<C>>,
    /// Slots a settlement may take after ranking before it counts as late.
    submission_deadline_allowance: u64,
    prev_auction: Option<C::Auction>,
    prev_tip: Option<C::Tip>,
    probes: Probes,
    /// Set from the lock in [`Self::run_forever`]. Disabled before that, so
    /// a bare [`Self::run_cycle`] leads.
    leader: LeaderLockTracker,
}

impl<C: Cycle> AuctionLoop<C> {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        trigger: Box<dyn CycleTrigger<C>>,
        provider: Box<dyn AuctionProvider<C>>,
        competition: Box<dyn SolverCompetition<C>>,
        winner_selection: Box<dyn WinnerSelection<C>>,
        executor: Box<dyn SettlementExecutor<C>>,
        observer: Box<dyn SettlementObserver<C>>,
        submission_deadline_allowance: u64,
        probes: Probes,
    ) -> Self {
        Self {
            trigger,
            provider,
            competition,
            winner_selection,
            executor,
            observer,
            submission_deadline_allowance,
            prev_auction: None,
            prev_tip: None,
            probes,
            leader: LeaderLockTracker::Disabled,
        }
    }

    /// Runs cycles until `shutdown` is cancelled, then releases the leader
    /// lock. The cycle in flight when the signal arrives runs to its end.
    /// With a lock, a cycle on an instance that does not hold it only syncs.
    pub async fn run_forever(
        mut self,
        leader_lock: Option<LeaderLock>,
        shutdown: CancellationToken,
    ) {
        self.leader = LeaderLockTracker::new(leader_lock);
        while !shutdown.is_cancelled() {
            self.leader.update();
            if self.leader.is_leader() {
                self.run_cycle().await;
            } else {
                self.sync().await;
            }
        }
        self.leader.release().await;
    }

    /// One iteration of the outer loop.
    pub async fn run_cycle(&mut self) {
        let tip = self.sync().await;
        let Some(auction) = self.next_auction(&tip).await else {
            return;
        };
        let auction_id = auction.id();
        self.single_run(&auction)
            .instrument(tracing::info_span!("auction", auction_id = %auction_id))
            .await;
    }

    /// Waits for the next tip and brings the order caches up to it.
    async fn sync(&mut self) -> C::Tip {
        let tip = self.trigger.next_cycle().await;

        match self.provider.sync_to_tip(&tip).await {
            // The order caches are up to date, so the start is complete.
            Ok(()) => {
                if let Some(startup) = self.probes.startup.as_ref() {
                    startup.store(true, Ordering::Release);
                }
            }
            Err(err) => tracing::warn!(?err, "failed to update auction"),
        }
        tip
    }

    /// Cuts the next auction and dedupes against the previous cycle. A new
    /// auction, an empty cut, and a stale cut refresh the liveness probe. A
    /// failed cut and a duplicate auction do not.
    async fn next_auction(&mut self, tip: &C::Tip) -> Option<C::Auction> {
        let auction = match self.provider.cut_auction(tip).await {
            // The liveness probe refreshes below, after the duplicate check,
            // because a repeated auction does not prove progress.
            NextAuction::Auction(auction) => auction,
            // The loop works: no order can go into the auction, or the order
            // data lags, which a restart of this process cannot fix.
            NextAuction::Empty | NextAuction::Stale => {
                self.probes.liveness.auction();
                return None;
            }
            NextAuction::Failed => return None,
        };

        // Only rerun the competition if the auction or tip changed.
        let previous_auction = self.prev_auction.replace(auction.clone());
        let previous_tip = self.prev_tip.replace(tip.clone());
        if previous_auction.as_ref() == Some(&auction) && previous_tip.as_ref() == Some(tip) {
            return None;
        }

        self.probes.liveness.auction();
        Some(auction)
    }

    /// Runs one competition for the auction.
    async fn single_run(&self, auction: &C::Auction) {
        self.observer.on_orders_ready(auction);

        let solutions = self.competition.solve(auction).await;
        if solutions.is_empty() {
            return;
        }

        let ranking = self.winner_selection.arbitrate(solutions, auction);
        tracing::debug!(winners = ranking.winner_count(), "ranked solutions");

        // the deadline derives from the tip observed after ranking
        let ranking_tip = self.trigger.current_tip();
        let deadline = C::submission_deadline(&ranking_tip, self.submission_deadline_allowance);

        // Solving takes seconds, long enough for the lease to end and another
        // instance to take over, so only a current leader records the result.
        if !self.leader.is_leader() {
            tracing::warn!("lost the leader lock while solving, dropping the auction");
            return;
        }

        // storing the outcome must finish before any settlement starts
        if let Err(err) = self
            .observer
            .persist_competition_ranking(auction, &ranking_tip, &ranking, deadline)
            .await
        {
            tracing::error!(?err, "failed to post-process competition");
            return;
        }

        // a winning order can also appear in a non-winning solution, keep it
        // Executing only
        let executing = ranking.winning_order_uids();
        let considered = ranking
            .considered_order_uids()
            .into_iter()
            .filter(|uid| !executing.contains(uid))
            .collect();
        self.observer.on_orders_matched(executing, considered);

        if !self.leader.is_leader() {
            tracing::warn!("lost the leader lock, not settling");
            return;
        }
        self.executor
            .execute(auction.id(), &ranking, &ranking_tip, deadline)
            .await;

        self.observer.on_competition_ended(auction, &ranking);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        observe::metrics::LivenessChecking,
        std::{collections::VecDeque, sync::Mutex, time::Duration},
    };

    struct TestCycle;

    impl Cycle for TestCycle {
        type Auction = TestAuction;
        type OrderUid = u64;
        type Ranking = NoRanking;
        type Solution = ();
        type Tip = u64;

        fn submission_deadline(tip: &u64, allowance: u64) -> u64 {
            tip + allowance
        }
    }

    /// An auction with the uids of its orders.
    #[derive(Clone, PartialEq)]
    struct TestAuction(Vec<u64>);

    impl AuctionInfo for TestAuction {
        fn id(&self) -> i64 {
            0
        }
    }

    struct NoRanking;

    impl RankingInfo<TestCycle> for NoRanking {
        fn winner_count(&self) -> usize {
            0
        }

        fn winning_order_uids(&self) -> HashSet<u64> {
            HashSet::new()
        }

        fn considered_order_uids(&self) -> HashSet<u64> {
            HashSet::new()
        }
    }

    /// Gives the result of the sync and of the cut from a script, one entry
    /// for each cycle.
    struct ScriptedProvider(Mutex<VecDeque<(bool, NextAuction<TestAuction>)>>);

    #[async_trait]
    impl AuctionProvider<TestCycle> for ScriptedProvider {
        async fn sync_to_tip(&self, _tip: &u64) -> anyhow::Result<()> {
            let synced = self.0.lock().unwrap().front().expect("a scripted cycle").0;
            anyhow::ensure!(synced, "scripted sync failure");
            Ok(())
        }

        async fn cut_auction(&self, _tip: &u64) -> NextAuction<TestAuction> {
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .expect("a scripted cycle")
                .1
        }
    }

    /// Fires every cycle at the same tip, and gets no solution, so a cycle
    /// ends after the auction goes to the solvers.
    struct Stub;

    #[async_trait]
    impl CycleTrigger<TestCycle> for Stub {
        async fn next_cycle(&mut self) -> u64 {
            1
        }

        fn current_tip(&self) -> u64 {
            1
        }
    }

    #[async_trait]
    impl SolverCompetition<TestCycle> for Stub {
        async fn solve(&self, _auction: &TestAuction) -> Vec<()> {
            Vec::new()
        }
    }

    impl WinnerSelection<TestCycle> for Stub {
        fn arbitrate(&self, _solutions: Vec<()>, _auction: &TestAuction) -> NoRanking {
            unreachable!("no solution to rank")
        }
    }

    #[async_trait]
    impl SettlementExecutor<TestCycle> for Stub {
        async fn execute(&self, _: i64, _: &NoRanking, _: &u64, _: u64) {
            unreachable!("no winner to settle")
        }
    }

    #[async_trait]
    impl SettlementObserver<TestCycle> for Stub {
        fn on_orders_ready(&self, _auction: &TestAuction) {}

        async fn persist_competition_ranking(
            &self,
            _: &TestAuction,
            _: &u64,
            _: &NoRanking,
            _: u64,
        ) -> anyhow::Result<()> {
            unreachable!("no ranking to store")
        }

        fn on_orders_matched(&self, _: HashSet<u64>, _: HashSet<u64>) {
            unreachable!("no ranking to report")
        }

        fn on_competition_ended(&self, _: &TestAuction, _: &NoRanking) {
            unreachable!("no competition to report")
        }
    }

    /// Fires every cycle at tip 1 like [`Stub`], and cancels the shutdown
    /// token while the cycle runs.
    struct ShutdownMidCycle(CancellationToken);

    #[async_trait]
    impl CycleTrigger<TestCycle> for ShutdownMidCycle {
        async fn next_cycle(&mut self) -> u64 {
            self.0.cancel();
            1
        }

        fn current_tip(&self) -> u64 {
            1
        }
    }

    /// Returns one solution and ranks it, and the lease ends while the
    /// solvers work.
    struct LeaseEndsWhileSolving(Arc<AtomicBool>);

    #[async_trait]
    impl SolverCompetition<TestCycle> for LeaseEndsWhileSolving {
        async fn solve(&self, _auction: &TestAuction) -> Vec<()> {
            self.0.store(false, Ordering::Release);
            vec![()]
        }
    }

    impl WinnerSelection<TestCycle> for LeaseEndsWhileSolving {
        fn arbitrate(&self, _solutions: Vec<()>, _auction: &TestAuction) -> NoRanking {
            NoRanking
        }
    }

    /// A loop that runs the scripted cycles. Its liveness probe fails until
    /// the loop records an auction, and its startup probe fails until the
    /// first successful sync.
    fn scripted_loop(
        cycles: impl IntoIterator<Item = (bool, NextAuction<TestAuction>)>,
    ) -> (
        AuctionLoop<TestCycle>,
        Arc<Liveness>,
        Arc<Option<AtomicBool>>,
    ) {
        scripted_loop_with(Box::new(Stub), cycles)
    }

    /// A [`scripted_loop`] whose cycles `trigger` fires.
    fn scripted_loop_with(
        trigger: Box<dyn CycleTrigger<TestCycle>>,
        cycles: impl IntoIterator<Item = (bool, NextAuction<TestAuction>)>,
    ) -> (
        AuctionLoop<TestCycle>,
        Arc<Liveness>,
        Arc<Option<AtomicBool>>,
    ) {
        let liveness = Arc::new(Liveness::new(Duration::from_secs(60)));
        liveness.expire();
        let startup = Arc::new(Some(AtomicBool::new(false)));
        let auction_loop = AuctionLoop::new(
            trigger,
            Box::new(ScriptedProvider(Mutex::new(cycles.into_iter().collect()))),
            Box::new(Stub),
            Box::new(Stub),
            Box::new(Stub),
            Box::new(Stub),
            0,
            Probes {
                liveness: liveness.clone(),
                startup: startup.clone(),
            },
        );
        (auction_loop, liveness, startup)
    }

    #[tokio::test]
    async fn a_new_auction_refreshes_the_liveness_probe() {
        let (mut auction_loop, liveness, _) =
            scripted_loop([(true, NextAuction::Auction(TestAuction(vec![1])))]);
        auction_loop.run_cycle().await;
        assert!(liveness.is_alive().await);
    }

    /// A leader whose lease ends while the solvers work stores and settles
    /// nothing, which [`Stub`] asserts.
    #[tokio::test]
    async fn a_leader_that_loses_the_lock_while_solving_stores_and_settles_nothing() {
        let leading = Arc::new(AtomicBool::new(true));
        let mut auction_loop = AuctionLoop::new(
            Box::new(Stub),
            Box::new(ScriptedProvider(Mutex::new(
                [(true, NextAuction::Auction(TestAuction(vec![1])))].into(),
            ))),
            Box::new(LeaseEndsWhileSolving(leading.clone())),
            Box::new(LeaseEndsWhileSolving(leading.clone())),
            Box::new(Stub),
            Box::new(Stub),
            0,
            Probes {
                liveness: Arc::new(Liveness::new(Duration::from_secs(60))),
                startup: Arc::new(None),
            },
        );
        auction_loop.leader = LeaderLockTracker::Scripted(leading);
        auction_loop.run_cycle().await;
    }

    #[tokio::test]
    async fn an_empty_cut_refreshes_the_liveness_probe() {
        let (mut auction_loop, liveness, _) = scripted_loop([(true, NextAuction::Empty)]);
        auction_loop.run_cycle().await;
        assert!(liveness.is_alive().await);
    }

    #[tokio::test]
    async fn a_stale_cut_refreshes_the_liveness_probe() {
        let (mut auction_loop, liveness, _) = scripted_loop([(true, NextAuction::Stale)]);
        auction_loop.run_cycle().await;
        assert!(liveness.is_alive().await);
    }

    #[tokio::test]
    async fn a_failed_cut_does_not_refresh_the_liveness_probe() {
        let (mut auction_loop, liveness, _) = scripted_loop([(true, NextAuction::Failed)]);
        auction_loop.run_cycle().await;
        assert!(!liveness.is_alive().await);
    }

    #[tokio::test]
    async fn a_duplicate_auction_does_not_refresh_the_liveness_probe() {
        let (mut auction_loop, liveness, _) = scripted_loop([
            (true, NextAuction::Auction(TestAuction(vec![1]))),
            (true, NextAuction::Auction(TestAuction(vec![1]))),
        ]);
        auction_loop.run_cycle().await;
        liveness.expire();
        auction_loop.run_cycle().await;
        assert!(!liveness.is_alive().await);
    }

    #[tokio::test]
    async fn the_startup_probe_passes_after_the_first_successful_sync() {
        let (mut auction_loop, _, startup) =
            scripted_loop([(false, NextAuction::Failed), (true, NextAuction::Failed)]);
        let started = || startup.as_ref().as_ref().unwrap().load(Ordering::Acquire);
        auction_loop.run_cycle().await;
        assert!(!started());
        auction_loop.run_cycle().await;
        assert!(started());
    }

    /// The cut of the cycle the signal arrives in still runs and refreshes
    /// the liveness probe. A second cycle would find no scripted entry and
    /// panic.
    #[tokio::test]
    async fn shutdown_ends_the_loop_after_the_current_cycle() {
        let shutdown = CancellationToken::new();
        let (auction_loop, liveness, _) = scripted_loop_with(
            Box::new(ShutdownMidCycle(shutdown.clone())),
            [(true, NextAuction::Empty)],
        );
        auction_loop.run_forever(None, shutdown).await;
        assert!(liveness.is_alive().await);
    }

    /// While another instance holds the lock, a cycle syncs, which passes
    /// the startup probe, and cuts nothing, so the liveness probe stays
    /// expired.
    #[tokio::test]
    #[ignore = "needs a local database"]
    async fn solana_db_a_standby_syncs_without_cutting() {
        let pool = crate::test_db::pool().await;
        let key = "solana_db_a_standby_syncs_without_cutting".to_string();
        let leader = LeaderLock::spawn(&pool, key.clone());
        tokio::time::timeout(Duration::from_secs(10), async {
            while !leader.is_leader() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the first instance takes the lock");

        let shutdown = CancellationToken::new();
        let (auction_loop, liveness, startup) = scripted_loop_with(
            Box::new(ShutdownMidCycle(shutdown.clone())),
            [(true, NextAuction::Auction(TestAuction(vec![1])))],
        );
        let standby = LeaderLock::spawn(&pool, key);
        auction_loop.run_forever(Some(standby), shutdown).await;

        assert!(startup.as_ref().as_ref().unwrap().load(Ordering::Acquire));
        assert!(!liveness.is_alive().await);
        leader.release().await;
    }
}
