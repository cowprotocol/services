//! Spike: the auction run loop parametrized over a Chain vocabulary.
//!
//! Additive experiment answering how run_loop.rs would look if its
//! orchestration were generic over the settlement chain. The real loop in
//! run_loop.rs stays untouched. evm.rs adapts the real EVM components to the
//! seams, solana.rs proves a second chain instantiates next to it.
//!
//! `AuctionLoop::run_cycle` mirrors run_loop.rs run_forever and single_run
//! phase by phase, including the maintenance cutoff, the auction dedupe and
//! the abort on failed competition persistence.

use {
    crate::{run_loop::Probes, shutdown_controller::ShutdownController},
    async_trait::async_trait,
    std::{
        collections::HashSet,
        fmt::{Debug, Display},
        hash::Hash,
        sync::atomic::Ordering,
        time::Duration,
    },
    tracing::Instrument,
};

pub mod evm;
pub mod solana;

#[cfg(test)]
mod tests;

/// How long a non leader pauses before rechecking leadership,
/// run_loop.rs:192.
const FOLLOWER_PAUSE: Duration = Duration::from_millis(200);

/// Type vocabulary of one settlement chain. Every associated type is a type
/// the loop itself has to name, everything else stays inside the seam
/// implementations.
pub trait Chain: Sized + Send + Sync + 'static {
    /// Chain progress marker (EVM block, Solana slot). Cycles are triggered
    /// by it, caches sync to it and the auction dedupe compares it.
    type Tip: Clone + PartialEq + Debug + Send + Sync + 'static;

    /// Order id. The loop keys the Executing and Considered order
    /// bookkeeping on the uid sets extracted from the ranking.
    type OrderUid: Copy + Eq + Hash + Debug + Send + Sync + 'static;

    /// Id allocated when the auction is cut. Names the tracing span and all
    /// competition persistence.
    type AuctionId: Copy + Display + Debug + Send + Sync + 'static;

    /// The cut auction fanned out to solvers. PartialEq implements the
    /// "same auction on the same tip solves nothing new" dedupe and must
    /// ignore the allocated id, like domain::Auction does.
    type Auction: AuctionInfo<Self> + Clone + PartialEq + Send + Sync + 'static;

    /// One solution proposed by one driver. Opaque to the loop, it only
    /// moves solutions from the competition into winner selection.
    type Solution: Send + 'static;

    /// Winner selection output over all solutions of one auction. Shared by
    /// the observer (persist outcome) and the executor (dispatch winners).
    type Ranking: RankingInfo<Self> + Send + Sync + 'static;

    /// Latest chain progress by which winners must have settled
    /// (EVM block number, Solana slot).
    type SubmissionDeadline: Copy + Debug + Send + Sync + 'static;
}

/// What the loop needs to know about an auction.
pub trait AuctionInfo<C: Chain> {
    fn id(&self) -> C::AuctionId;
}

/// What the loop needs to know about a ranking.
pub trait RankingInfo<C: Chain> {
    /// Number of winning solutions, feeds the auction_winners metric of
    /// run_loop.rs:355-358.
    fn winner_count(&self) -> usize;

    /// Orders of all winning solutions, marked Executing
    /// (run_loop.rs:380-385).
    fn winning_order_uids(&self) -> HashSet<C::OrderUid>;

    /// Orders of ranked non winning solutions. The loop subtracts the
    /// winning set before marking them Considered (run_loop.rs:388-394).
    fn considered_order_uids(&self) -> HashSet<C::OrderUid>;
}

/// Yields the tip to build the next auction on. Wraps the wake sources
/// (new tip, new orders) and the staleness resync of run_loop.rs:180 and
/// run_loop.rs:238-251.
#[async_trait]
pub trait CycleTrigger<C: Chain>: Send {
    /// Blocks until something happened that warrants a new cycle and
    /// returns the tip to build on.
    async fn next_cycle(&mut self) -> C::Tip;

    /// Latest observed tip without waiting. single_run reads it after
    /// ranking to derive the submission deadline (run_loop.rs:360).
    fn current_tip(&self) -> C::Tip;
}

/// Produces the cut auction for a tip. Wraps the maintenance cutoff and
/// solvable orders cache of run_loop.rs:253-273 and the auction cutting of
/// run_loop.rs:304-333.
#[async_trait]
pub trait AuctionProvider<C: Chain>: Send + Sync {
    /// Brings indexers and the solvable orders cache up to date with the
    /// tip. Errors are logged by the loop but do not stop the cycle, the
    /// auction is then cut from slightly stale caches like today.
    async fn sync_to_tip(&self, tip: &C::Tip, is_leader: bool) -> anyhow::Result<()>;

    /// Cuts the auction for the tip, allocating an id and archiving it.
    /// None when there is nothing to solve.
    async fn cut_auction(&self, tip: &C::Tip) -> Option<C::Auction>;
}

/// Fans the auction out to all drivers and returns attributable solutions,
/// run_loop.rs:591-639.
#[async_trait]
pub trait SolverCompetition<C: Chain>: Send + Sync {
    async fn solve(&self, auction: &C::Auction) -> Vec<C::Solution>;
}

/// Scores, filters and ranks solutions, marking winners, run_loop.rs:352.
pub trait WinnerSelection<C: Chain>: Send + Sync {
    fn arbitrate(&self, solutions: Vec<C::Solution>, auction: &C::Auction) -> C::Ranking;
}

/// Dispatches winning solutions for execution, run_loop.rs:397-409.
#[async_trait]
pub trait SettlementExecutor<C: Chain>: Send + Sync {
    /// Submission cutoff for settlements of an auction ranked at the given
    /// tip, run_loop.rs:360-361.
    fn submission_deadline(&self, tip: &C::Tip) -> C::SubmissionDeadline;

    /// Dispatches every winner. Implementations submit in the background,
    /// the loop does not wait for settlement results.
    async fn execute(
        &self,
        auction_id: C::AuctionId,
        ranking: &C::Ranking,
        deadline: C::SubmissionDeadline,
    );
}

/// Competition bookkeeping around the settlement, mirroring the persistence
/// calls the real loop makes inline.
#[async_trait]
pub trait SettlementObserver<C: Chain>: Send + Sync {
    /// All auction orders entered the competition, run_loop.rs:341.
    fn orders_ready(&self, auction: &C::Auction);

    /// Persists the competition outcome (auction, solutions, scores, fees),
    /// run_loop.rs:365-376. Errors abort the cycle before any settlement is
    /// dispatched.
    async fn competition_ranked(
        &self,
        auction: &C::Auction,
        tip: &C::Tip,
        ranking: &C::Ranking,
        deadline: C::SubmissionDeadline,
    ) -> anyhow::Result<()>;

    /// Winning orders are Executing, other ranked orders Considered,
    /// run_loop.rs:379-395.
    fn orders_matched(&self, executing: HashSet<C::OrderUid>, considered: HashSet<C::OrderUid>);

    /// Final per cycle reporting after settlements were dispatched,
    /// run_loop.rs:411.
    fn competition_ended(&self, auction: &C::Auction, ranking: &C::Ranking);
}

/// Leader election is chain agnostic (Postgres advisory lock) so it is not
/// parametrized on Chain.
#[async_trait]
pub trait Leadership: Send {
    /// Called at the start of every cycle, run_loop.rs:175.
    async fn try_acquire(&mut self) -> bool;

    /// Called once on shutdown, run_loop.rs:207.
    async fn release(&mut self);
}

/// Leadership for setups without a leader lock, always the leader.
pub struct AlwaysLeader;

#[async_trait]
impl Leadership for AlwaysLeader {
    async fn try_acquire(&mut self) -> bool {
        true
    }

    async fn release(&mut self) {}
}

/// Chain generic counterpart of run_loop::RunLoop. Owns the seams and the
/// dedupe state, drives the phases in the order of the real loop.
pub struct AuctionLoop<C: Chain> {
    trigger: Box<dyn CycleTrigger<C>>,
    provider: Box<dyn AuctionProvider<C>>,
    competition: Box<dyn SolverCompetition<C>>,
    winner_selection: Box<dyn WinnerSelection<C>>,
    executor: Box<dyn SettlementExecutor<C>>,
    observer: Box<dyn SettlementObserver<C>>,
    leadership: Box<dyn Leadership>,
    probes: Probes,
    prev_auction: Option<C::Auction>,
    prev_tip: Option<C::Tip>,
}

impl<C: Chain> AuctionLoop<C> {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        trigger: Box<dyn CycleTrigger<C>>,
        provider: Box<dyn AuctionProvider<C>>,
        competition: Box<dyn SolverCompetition<C>>,
        winner_selection: Box<dyn WinnerSelection<C>>,
        executor: Box<dyn SettlementExecutor<C>>,
        observer: Box<dyn SettlementObserver<C>>,
        leadership: Box<dyn Leadership>,
        probes: Probes,
    ) -> Self {
        Self {
            trigger,
            provider,
            competition,
            winner_selection,
            executor,
            observer,
            leadership,
            probes,
            prev_auction: None,
            prev_tip: None,
        }
    }

    /// Drives cycles until shutdown, run_loop.rs:157-208.
    pub async fn run(mut self, mut control: ShutdownController) {
        while !control.should_shutdown() {
            self.run_cycle().await;
        }
        self.leadership.release().await;
    }

    /// One iteration of the outer loop, run_loop.rs:174-205.
    pub async fn run_cycle(&mut self) {
        let is_leader = self.leadership.try_acquire().await;

        // wait for a state change worth a new auction, run_loop.rs:180
        let tip = self.trigger.next_cycle().await;

        // maintenance and cache cutoff for the tip, run_loop.rs:253-273
        if let Err(err) = self.provider.sync_to_tip(&tip, is_leader).await {
            tracing::warn!(?err, "failed to update auction");
        }

        // caches are warm, report readiness, run_loop.rs:186-188
        if let Some(startup) = self.probes.startup.as_ref() {
            startup.store(true, Ordering::Release);
        }

        if !is_leader {
            // followers only keep caches warm, run_loop.rs:190-194
            tokio::time::sleep(FOLLOWER_PAUSE).await;
            return;
        }

        let Some(auction) = self.next_auction(&tip).await else {
            return;
        };
        let auction_id = auction.id();
        self.single_run(&auction)
            .instrument(tracing::info_span!("auction", auction_id = %auction_id))
            .await;
    }

    /// Cuts the next auction and dedupes against the previous cycle,
    /// run_loop.rs:280-302.
    async fn next_auction(&mut self, tip: &C::Tip) -> Option<C::Auction> {
        let auction = self.provider.cut_auction(tip).await?;

        // Only rerun the competition if the auction or tip changed. The tip
        // marker is only written when the auction was unchanged, mirroring
        // the short circuit in run_loop.rs:291-295, so the dedupe kicks in
        // one cycle after the auction first repeats.
        let previous = self.prev_auction.replace(auction.clone());
        if previous.as_ref() == Some(&auction)
            && self.prev_tip.replace(tip.clone()).as_ref() == Some(tip)
        {
            return None;
        }

        self.probes.liveness.auction();
        Some(auction)
    }

    /// Runs one competition for the auction, run_loop.rs:336-412.
    async fn single_run(&self, auction: &C::Auction) {
        // mark all auction orders as ready, run_loop.rs:341
        self.observer.orders_ready(auction);

        // collect solutions from all drivers, run_loop.rs:346
        let solutions = self.competition.solve(auction).await;
        if solutions.is_empty() {
            return;
        }

        let ranking = self.winner_selection.arbitrate(solutions, auction);
        tracing::debug!(winners = ranking.winner_count(), "ranked solutions");

        // the deadline derives from the tip observed after ranking,
        // run_loop.rs:360-361
        let ranking_tip = self.trigger.current_tip();
        let deadline = self.executor.submission_deadline(&ranking_tip);

        // storing the outcome must finish before any settlement starts,
        // run_loop.rs:363-376
        if let Err(err) = self
            .observer
            .competition_ranked(auction, &ranking_tip, &ranking, deadline)
            .await
        {
            tracing::error!(?err, "failed to post-process competition");
            return;
        }

        // winning orders are Executing, other ranked orders Considered,
        // run_loop.rs:379-395
        let executing = ranking.winning_order_uids();
        let considered = ranking
            .considered_order_uids()
            .into_iter()
            .filter(|uid| !executing.contains(uid))
            .collect();
        self.observer.orders_matched(executing, considered);

        // dispatch winners for execution in the background,
        // run_loop.rs:397-409
        self.executor
            .execute(auction.id(), &ranking, deadline)
            .await;

        self.observer.competition_ended(auction, &ranking);
    }
}
