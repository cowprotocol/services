//! The auction run loop parametrized over a chain vocabulary.
//!
//! `AuctionLoop` drives one settlement competition per cycle through a fixed
//! phase order, generic over the settlement chain. A chain supplies its type
//! vocabulary through the `Cycle` trait and its behaviour through six seam
//! traits. The loop owns the phase ordering and the auction dedupe and knows
//! nothing chain specific. Leadership is a chain agnostic concern handled by
//! the caller, so the loop always runs as if it were the leader.

use {
    async_trait::async_trait,
    std::{collections::HashSet, fmt::Debug, hash::Hash},
    tracing::Instrument,
};

/// Type vocabulary of one settlement chain. Every associated type is a type
/// the loop itself has to name, everything else stays inside the seam
/// implementations.
pub trait Cycle: Sized + Send + Sync + 'static {
    /// Chain progress marker (EVM block, Solana slot). Cycles are triggered
    /// by it, caches sync to it and the auction dedupe compares it.
    type Tip: Clone + PartialEq + Debug + Send + Sync + 'static;

    /// Order id. The loop keys the Executing and Considered order
    /// bookkeeping on the uid sets extracted from the ranking.
    type OrderUid: Copy + Eq + Hash + Debug + Send + Sync + 'static;

    /// The cut auction fanned out to solvers. PartialEq implements the
    /// "same auction on the same tip solves nothing new" dedupe and must
    /// ignore the allocated id.
    type Auction: AuctionInfo<Self> + Clone + PartialEq + Send + Sync + 'static;

    /// One solution proposed by one driver. Opaque to the loop, it only
    /// moves solutions from the competition into winner selection.
    type Solution: Send + 'static;

    /// Winner selection output over all solutions of one auction. Shared by
    /// the observer (persist outcome) and the executor (dispatch winners).
    type Ranking: RankingInfo<Self> + Send + Sync + 'static;
}

/// What the loop needs to know about an auction.
pub trait AuctionInfo<C: Cycle> {
    fn id(&self) -> i64;
}

/// What the loop needs to know about a ranking.
pub trait RankingInfo<C: Cycle> {
    /// Number of winning solutions.
    fn winner_count(&self) -> usize;

    /// Orders of all winning solutions, marked Executing.
    fn winning_order_uids(&self) -> HashSet<C::OrderUid>;

    /// Orders of ranked non winning solutions. The loop subtracts the
    /// winning set before marking them Considered.
    fn considered_order_uids(&self) -> HashSet<C::OrderUid>;
}

/// Yields the tip to build the next auction on. Wraps the wake sources
/// (new tip, new orders) and the staleness resync.
#[async_trait]
pub trait CycleTrigger<C: Cycle>: Send {
    /// Blocks until something happened that warrants a new cycle and
    /// returns the tip to build on.
    async fn next_cycle(&mut self) -> C::Tip;

    /// Latest observed tip without waiting. The loop reads it after ranking
    /// to derive the submission deadline.
    fn current_tip(&self) -> C::Tip;
}

/// Produces the cut auction for a tip.
#[async_trait]
pub trait AuctionProvider<C: Cycle>: Send + Sync {
    /// Brings indexers and the solvable orders cache up to date with the
    /// tip. Errors are logged by the loop but do not stop the cycle, the
    /// auction is then cut from slightly stale caches.
    async fn sync_to_tip(&self, tip: &C::Tip) -> anyhow::Result<()>;

    /// Cuts the auction for the tip, allocating an id and archiving it.
    /// None when there is nothing to solve.
    async fn cut_auction(&self, tip: &C::Tip) -> Option<C::Auction>;
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
    /// Submission cutoff for settlements of an auction ranked at the given
    /// tip.
    fn submission_deadline(&self, tip: &C::Tip) -> u64;

    /// Dispatches every winner. Implementations submit in the background,
    /// the loop does not wait for settlement results.
    async fn execute(&self, auction_id: i64, ranking: &C::Ranking, deadline: u64);
}

/// Competition bookkeeping around the settlement outcome.
#[async_trait]
pub trait SettlementObserver<C: Cycle>: Send + Sync {
    /// All auction orders entered the competition.
    fn orders_ready(&self, auction: &C::Auction);

    /// Persists the competition outcome (auction, solutions, scores, fees).
    /// Errors abort the cycle before any settlement is dispatched.
    async fn competition_ranked(
        &self,
        auction: &C::Auction,
        tip: &C::Tip,
        ranking: &C::Ranking,
        deadline: u64,
    ) -> anyhow::Result<()>;

    /// Winning orders are Executing, other ranked orders Considered.
    fn orders_matched(&self, executing: HashSet<C::OrderUid>, considered: HashSet<C::OrderUid>);

    /// Final per cycle reporting after settlements were dispatched.
    fn competition_ended(&self, auction: &C::Auction, ranking: &C::Ranking);
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
    prev_auction: Option<C::Auction>,
    prev_tip: Option<C::Tip>,
}

impl<C: Cycle> AuctionLoop<C> {
    pub fn new(
        trigger: Box<dyn CycleTrigger<C>>,
        provider: Box<dyn AuctionProvider<C>>,
        competition: Box<dyn SolverCompetition<C>>,
        winner_selection: Box<dyn WinnerSelection<C>>,
        executor: Box<dyn SettlementExecutor<C>>,
        observer: Box<dyn SettlementObserver<C>>,
    ) -> Self {
        Self {
            trigger,
            provider,
            competition,
            winner_selection,
            executor,
            observer,
            prev_auction: None,
            prev_tip: None,
        }
    }

    /// One iteration of the outer loop.
    pub async fn run_cycle(&mut self) {
        // wait for a state change worth a new auction
        let tip = self.trigger.next_cycle().await;

        // maintenance and cache cutoff for the tip
        if let Err(err) = self.provider.sync_to_tip(&tip).await {
            tracing::warn!(?err, "failed to update auction");
        }

        let Some(auction) = self.next_auction(&tip).await else {
            return;
        };
        let auction_id = auction.id();
        self.single_run(&auction)
            .instrument(tracing::info_span!("auction", auction_id = %auction_id))
            .await;
    }

    /// Cuts the next auction and dedupes against the previous cycle.
    async fn next_auction(&mut self, tip: &C::Tip) -> Option<C::Auction> {
        let auction = self.provider.cut_auction(tip).await?;

        // Only rerun the competition if the auction or tip changed. The tip
        // marker is only written when the auction was unchanged, so the
        // dedupe kicks in one cycle after the auction first repeats.
        let previous = self.prev_auction.replace(auction.clone());
        if previous.as_ref() == Some(&auction)
            && self.prev_tip.replace(tip.clone()).as_ref() == Some(tip)
        {
            return None;
        }

        Some(auction)
    }

    /// Runs one competition for the auction.
    async fn single_run(&self, auction: &C::Auction) {
        // mark all auction orders as ready
        self.observer.orders_ready(auction);

        // collect solutions from all drivers
        let solutions = self.competition.solve(auction).await;
        if solutions.is_empty() {
            return;
        }

        let ranking = self.winner_selection.arbitrate(solutions, auction);
        tracing::debug!(winners = ranking.winner_count(), "ranked solutions");

        // the deadline derives from the tip observed after ranking
        let ranking_tip = self.trigger.current_tip();
        let deadline = self.executor.submission_deadline(&ranking_tip);

        // storing the outcome must finish before any settlement starts
        if let Err(err) = self
            .observer
            .competition_ranked(auction, &ranking_tip, &ranking, deadline)
            .await
        {
            tracing::error!(?err, "failed to post-process competition");
            return;
        }

        // winning orders are Executing, other ranked orders Considered
        let executing = ranking.winning_order_uids();
        let considered = ranking
            .considered_order_uids()
            .into_iter()
            .filter(|uid| !executing.contains(uid))
            .collect();
        self.observer.orders_matched(executing, considered);

        // dispatch winners for execution in the background
        self.executor
            .execute(auction.id(), &ranking, deadline)
            .await;

        self.observer.competition_ended(auction, &ranking);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{
            AuctionInfo,
            AuctionLoop,
            AuctionProvider,
            Cycle,
            CycleTrigger,
            RankingInfo,
            SettlementExecutor,
            SettlementObserver,
            SolverCompetition,
            WinnerSelection,
        },
        async_trait::async_trait,
        std::{
            collections::{HashSet, VecDeque},
            sync::{Arc, Mutex},
        },
    };

    type Log = Arc<Mutex<Vec<String>>>;

    fn record(log: &Log, entry: impl Into<String>) {
        log.lock().unwrap().push(entry.into());
    }

    fn sorted(uids: HashSet<u64>) -> Vec<u64> {
        let mut uids: Vec<_> = uids.into_iter().collect();
        uids.sort();
        uids
    }

    // test only chain proving the orchestration runs on anything satisfying
    // the vocabulary

    struct MockChain;

    #[derive(Clone, Copy, PartialEq, Debug)]
    struct MockTip(u64);

    #[derive(Clone, Debug)]
    struct MockAuction {
        id: i64,
        orders: Vec<u64>,
    }

    impl PartialEq for MockAuction {
        // dedupe ignores the allocated id
        fn eq(&self, other: &Self) -> bool {
            self.orders == other.orders
        }
    }

    struct MockSolution;

    struct MockRanking {
        winning: Vec<u64>,
        considered: Vec<u64>,
    }

    impl Cycle for MockChain {
        type Auction = MockAuction;
        type OrderUid = u64;
        type Ranking = MockRanking;
        type Solution = MockSolution;
        type Tip = MockTip;
    }

    impl AuctionInfo<MockChain> for MockAuction {
        fn id(&self) -> i64 {
            self.id
        }
    }

    impl RankingInfo<MockChain> for MockRanking {
        fn winner_count(&self) -> usize {
            usize::from(!self.winning.is_empty())
        }

        fn winning_order_uids(&self) -> HashSet<u64> {
            self.winning.iter().copied().collect()
        }

        fn considered_order_uids(&self) -> HashSet<u64> {
            self.considered.iter().copied().collect()
        }
    }

    struct MockTrigger {
        log: Log,
        tips: VecDeque<MockTip>,
        current: MockTip,
    }

    #[async_trait]
    impl CycleTrigger<MockChain> for MockTrigger {
        async fn next_cycle(&mut self) -> MockTip {
            let tip = self.tips.pop_front().expect("test drove too many cycles");
            record(&self.log, format!("next_cycle tip={}", tip.0));
            tip
        }

        fn current_tip(&self) -> MockTip {
            record(&self.log, format!("current_tip tip={}", self.current.0));
            self.current
        }
    }

    struct MockProvider {
        log: Log,
        auctions: Mutex<VecDeque<Option<MockAuction>>>,
    }

    #[async_trait]
    impl AuctionProvider<MockChain> for MockProvider {
        async fn sync_to_tip(&self, tip: &MockTip) -> anyhow::Result<()> {
            record(&self.log, format!("sync tip={}", tip.0));
            Ok(())
        }

        async fn cut_auction(&self, _tip: &MockTip) -> Option<MockAuction> {
            let auction = self
                .auctions
                .lock()
                .unwrap()
                .pop_front()
                .expect("test drove too many cuts");
            match &auction {
                Some(auction) => record(&self.log, format!("cut auction={}", auction.id)),
                None => record(&self.log, "cut none"),
            }
            auction
        }
    }

    struct MockCompetition {
        log: Log,
        solution_counts: Mutex<VecDeque<usize>>,
    }

    #[async_trait]
    impl SolverCompetition<MockChain> for MockCompetition {
        async fn solve(&self, auction: &MockAuction) -> Vec<MockSolution> {
            let count = self
                .solution_counts
                .lock()
                .unwrap()
                .pop_front()
                .expect("test drove too many solves");
            record(
                &self.log,
                format!("solve auction={} solutions={count}", auction.id),
            );
            (0..count).map(|_| MockSolution).collect()
        }
    }

    struct MockWinnerSelection {
        log: Log,
    }

    impl WinnerSelection<MockChain> for MockWinnerSelection {
        fn arbitrate(&self, solutions: Vec<MockSolution>, auction: &MockAuction) -> MockRanking {
            record(
                &self.log,
                format!(
                    "arbitrate auction={} solutions={}",
                    auction.id,
                    solutions.len()
                ),
            );
            MockRanking {
                winning: vec![1, 2],
                // order 2 also appears in a winning solution, the loop must
                // subtract it before marking Considered
                considered: vec![2, 3],
            }
        }
    }

    struct MockExecutor {
        log: Log,
    }

    #[async_trait]
    impl SettlementExecutor<MockChain> for MockExecutor {
        fn submission_deadline(&self, tip: &MockTip) -> u64 {
            record(&self.log, format!("deadline tip={}", tip.0));
            tip.0 + 100
        }

        async fn execute(&self, auction_id: i64, _ranking: &MockRanking, deadline: u64) {
            record(
                &self.log,
                format!("execute auction={auction_id} deadline={deadline}"),
            );
        }
    }

    struct MockObserver {
        log: Log,
    }

    #[async_trait]
    impl SettlementObserver<MockChain> for MockObserver {
        fn orders_ready(&self, auction: &MockAuction) {
            record(&self.log, format!("ready auction={}", auction.id));
        }

        async fn competition_ranked(
            &self,
            auction: &MockAuction,
            tip: &MockTip,
            _ranking: &MockRanking,
            deadline: u64,
        ) -> anyhow::Result<()> {
            record(
                &self.log,
                format!(
                    "ranked auction={} tip={} deadline={deadline}",
                    auction.id, tip.0
                ),
            );
            Ok(())
        }

        fn orders_matched(&self, executing: HashSet<u64>, considered: HashSet<u64>) {
            record(
                &self.log,
                format!(
                    "matched executing={:?} considered={:?}",
                    sorted(executing),
                    sorted(considered)
                ),
            );
        }

        fn competition_ended(&self, auction: &MockAuction, _ranking: &MockRanking) {
            record(&self.log, format!("ended auction={}", auction.id));
        }
    }

    /// Drives the generic loop through four cycles over a mock chain and
    /// asserts the exact phase sequence: the full happy path, the empty
    /// solution early return, the dedupe firing after its one cycle tip lag,
    /// and a new tip resetting the dedupe.
    #[tokio::test]
    async fn phases_run_in_the_order_of_the_real_loop() {
        let log = Log::default();
        let auction = |id| MockAuction {
            id,
            orders: vec![1, 2, 3],
        };

        let mut auction_loop: AuctionLoop<MockChain> = AuctionLoop::new(
            Box::new(MockTrigger {
                log: log.clone(),
                tips: [MockTip(1), MockTip(1), MockTip(1), MockTip(2)].into(),
                current: MockTip(9),
            }),
            Box::new(MockProvider {
                log: log.clone(),
                auctions: Mutex::new(
                    [
                        Some(auction(1)),
                        Some(auction(2)),
                        Some(auction(3)),
                        Some(auction(4)),
                    ]
                    .into(),
                ),
            }),
            Box::new(MockCompetition {
                log: log.clone(),
                solution_counts: Mutex::new([1, 0, 0].into()),
            }),
            Box::new(MockWinnerSelection { log: log.clone() }),
            Box::new(MockExecutor { log: log.clone() }),
            Box::new(MockObserver { log: log.clone() }),
        );

        for _ in 0..4 {
            auction_loop.run_cycle().await;
        }

        let expected = vec![
            // cycle 1, fresh auction, full competition
            "next_cycle tip=1",
            "sync tip=1",
            "cut auction=1",
            "ready auction=1",
            "solve auction=1 solutions=1",
            "arbitrate auction=1 solutions=1",
            "current_tip tip=9",
            "deadline tip=9",
            "ranked auction=1 tip=9 deadline=109",
            "matched executing=[1, 2] considered=[3]",
            "execute auction=1 deadline=109",
            "ended auction=1",
            // cycle 2, identical auction on the same tip still runs once more
            // because the tip marker is written one cycle behind the auction
            // marker
            "next_cycle tip=1",
            "sync tip=1",
            "cut auction=2",
            "ready auction=2",
            "solve auction=2 solutions=0",
            // cycle 3, now the dedupe kicks in right after cutting
            "next_cycle tip=1",
            "sync tip=1",
            "cut auction=3",
            // cycle 4, same auction but a new tip reruns the competition
            "next_cycle tip=2",
            "sync tip=2",
            "cut auction=4",
            "ready auction=4",
            "solve auction=4 solutions=0",
        ];
        assert_eq!(*log.lock().unwrap(), expected);
    }
}
