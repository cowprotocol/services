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
//! tip, the auction, order ids, solutions, the ranking), and the seam traits
//! ([`CycleTrigger`], [`AuctionProvider`], [`SolverCompetition`],
//! [`WinnerSelection`], [`SettlementExecutor`], [`SettlementObserver`]) carry
//! the per-chain behaviour behind each phase. Leadership is chain-agnostic
//! and belongs to the caller, the loop always runs as if it were the leader.

use {
    async_trait::async_trait,
    std::{collections::HashSet, fmt::Debug, hash::Hash},
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

    /// Cuts the auction for the tip. None when there is nothing to solve.
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
        let tip = self.trigger.next_cycle().await;

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
        self.observer.on_orders_ready(auction);

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

        self.executor
            .execute(auction.id(), &ranking, deadline)
            .await;

        self.observer.on_competition_ended(auction, &ranking);
    }
}
