//! Skeleton proving the Chain abstraction admits a non EVM chain.
//!
//! The type vocabulary (Pubkey, IntentHash, u64 amounts) comes from the
//! winner-selection crate's Solana instantiation, so the loop and the
//! shared CIP-38 logic agree on types by construction. The WinnerSelection
//! seam is real: it runs the same generic arbitrator the EVM loop uses.
//! The remaining seams are constructible stubs, their backends (slot
//! stream, orderbook, driver protocol, persistence) do not exist yet.

use {
    super::{
        AuctionInfo,
        AuctionProvider,
        Chain,
        CycleTrigger,
        RankingInfo,
        SettlementExecutor,
        SettlementObserver,
        SolverCompetition,
        WinnerSelection,
    },
    async_trait::async_trait,
    std::collections::{HashMap, HashSet},
    winner_selection::{
        self as ws,
        solana::{IntentHash, Pubkey},
    },
};

pub struct SolanaChain;

/// Solana observes chain progress in slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SolanaTip {
    pub slot: u64,
}

/// A solvable order as the auction carries it: enough for the dedupe and
/// for building the winner-selection context.
#[derive(Clone, PartialEq, Debug)]
pub struct SolanaOrder {
    pub uid: IntentHash,
    pub owner: Pubkey,
}

#[derive(Clone, Debug)]
pub struct SolanaAuction {
    pub id: i64,
    pub slot: u64,
    pub orders: Vec<SolanaOrder>,
    /// Native (wSOL) prices per mint, lamport-scaled.
    pub prices: HashMap<Pubkey, u64>,
}

impl PartialEq for SolanaAuction {
    // the dedupe must ignore the allocated id, same as domain::Auction
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.orders == other.orders && self.prices == other.prices
    }
}

impl Chain for SolanaChain {
    type Auction = SolanaAuction;
    type AuctionId = i64;
    type OrderUid = IntentHash;
    /// The shared crate's ranking over Solana types, same struct the EVM
    /// loop consumes over EVM types.
    type Ranking = ws::Ranking<ws::solana::Solana>;
    /// A solver's proposed execution in the shared crate's vocabulary.
    type Solution = ws::Solution<ws::Unscored, ws::solana::Solana>;
    // slot by which winners must have settled
    type SubmissionDeadline = u64;
    type Tip = SolanaTip;
}

impl AuctionInfo<SolanaChain> for SolanaAuction {
    fn id(&self) -> i64 {
        self.id
    }
}

impl RankingInfo<SolanaChain> for ws::Ranking<ws::solana::Solana> {
    fn winner_count(&self) -> usize {
        self.winners().count()
    }

    fn winning_order_uids(&self) -> HashSet<IntentHash> {
        self.winners()
            .flat_map(|solution| solution.orders().iter().map(|order| order.uid))
            .collect()
    }

    fn considered_order_uids(&self) -> HashSet<IntentHash> {
        self.non_winners()
            .flat_map(|solution| solution.orders().iter().map(|order| order.uid))
            .collect()
    }
}

pub struct SolanaCycleTrigger;

#[async_trait]
impl CycleTrigger<SolanaChain> for SolanaCycleTrigger {
    async fn next_cycle(&mut self) -> SolanaTip {
        unimplemented!("spike: no solana slot stream exists yet")
    }

    fn current_tip(&self) -> SolanaTip {
        unimplemented!("spike: no solana slot stream exists yet")
    }
}

pub struct SolanaAuctionProvider;

#[async_trait]
impl AuctionProvider<SolanaChain> for SolanaAuctionProvider {
    async fn sync_to_tip(&self, _tip: &SolanaTip, _is_leader: bool) -> anyhow::Result<()> {
        unimplemented!("spike: no solana indexer or order cache exists yet")
    }

    async fn cut_auction(&self, _tip: &SolanaTip) -> Option<SolanaAuction> {
        unimplemented!("spike: no solana orderbook exists yet")
    }
}

pub struct SolanaSolverCompetition;

#[async_trait]
impl SolverCompetition<SolanaChain> for SolanaSolverCompetition {
    async fn solve(
        &self,
        _auction: &SolanaAuction,
    ) -> Vec<ws::Solution<ws::Unscored, ws::solana::Solana>> {
        unimplemented!("spike: no solana driver protocol exists yet")
    }
}

/// The real thing: CIP-38 winner selection through the same generic
/// arbitrator the EVM loop uses, instantiated over Solana types.
pub struct SolanaWinnerSelection {
    arbitrator: ws::Arbitrator<ws::solana::Solana>,
}

impl SolanaWinnerSelection {
    pub fn new(max_winners: usize, wsol_mint: Pubkey) -> Self {
        Self {
            arbitrator: ws::Arbitrator {
                max_winners,
                wrapped_native: wsol_mint,
            },
        }
    }
}

impl WinnerSelection<SolanaChain> for SolanaWinnerSelection {
    fn arbitrate(
        &self,
        solutions: Vec<ws::Solution<ws::Unscored, ws::solana::Solana>>,
        auction: &SolanaAuction,
    ) -> ws::Ranking<ws::solana::Solana> {
        let context = ws::AuctionContext::<ws::solana::Solana> {
            // Every auction order is a user order. No protocol fees at the
            // demo stage, so the policy list is empty but present, which is
            // what makes the order count toward the score.
            fee_policies: auction
                .orders
                .iter()
                .map(|order| (order.uid, vec![]))
                .collect(),
            // JIT surplus capture is unsupported on Solana, the intent hash
            // embeds no owner to attribute.
            surplus_capturing_jit_order_owners: HashSet::new(),
            native_prices: auction.prices.clone(),
        };
        self.arbitrator.arbitrate(solutions, &context)
    }
}

pub struct SolanaSettlementExecutor;

#[async_trait]
impl SettlementExecutor<SolanaChain> for SolanaSettlementExecutor {
    fn submission_deadline(&self, _tip: &SolanaTip) -> u64 {
        unimplemented!("spike: solana submission windows are undecided")
    }

    async fn execute(
        &self,
        _auction_id: i64,
        _ranking: &ws::Ranking<ws::solana::Solana>,
        _deadline: u64,
    ) {
        unimplemented!("spike: no solana settlement submission exists yet")
    }
}

pub struct SolanaSettlementObserver;

#[async_trait]
impl SettlementObserver<SolanaChain> for SolanaSettlementObserver {
    fn orders_ready(&self, _auction: &SolanaAuction) {
        unimplemented!("spike: no solana persistence exists yet")
    }

    async fn competition_ranked(
        &self,
        _auction: &SolanaAuction,
        _tip: &SolanaTip,
        _ranking: &ws::Ranking<ws::solana::Solana>,
        _deadline: u64,
    ) -> anyhow::Result<()> {
        unimplemented!("spike: no solana persistence exists yet")
    }

    fn orders_matched(&self, _executing: HashSet<IntentHash>, _considered: HashSet<IntentHash>) {
        unimplemented!("spike: no solana persistence exists yet")
    }

    fn competition_ended(
        &self,
        _auction: &SolanaAuction,
        _ranking: &ws::Ranking<ws::solana::Solana>,
    ) {
        unimplemented!("spike: no solana persistence exists yet")
    }
}
