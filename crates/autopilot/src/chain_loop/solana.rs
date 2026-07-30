//! Skeleton proving the Chain abstraction admits a non EVM chain.
//! Placeholder newtypes stand in for solana-sdk types on purpose, the
//! autopilot must not grow a solana dependency for a spike. Seams are
//! constructible stubs so AuctionLoop<SolanaChain> instantiates next to
//! AuctionLoop<EvmChain>, none of them has a backend yet.

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
    std::collections::HashSet,
};

pub struct SolanaChain;

/// Solana observes chain progress in slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SolanaTip {
    pub slot: u64,
}

/// 32 byte account id (ed25519 pubkey).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SolanaAddress(pub [u8; 32]);

/// Placeholder order id, unlike the EVM 56 byte uid it does not embed
/// owner and validity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SolanaOrderUid(pub [u8; 32]);

#[derive(Clone, PartialEq, Debug)]
pub struct SolanaOrder {
    pub uid: SolanaOrderUid,
    pub owner: SolanaAddress,
}

#[derive(Clone, Debug)]
pub struct SolanaAuction {
    pub id: i64,
    pub slot: u64,
    pub orders: Vec<SolanaOrder>,
}

impl PartialEq for SolanaAuction {
    // the dedupe must ignore the allocated id, same as domain::Auction
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.orders == other.orders
    }
}

pub struct SolanaSolution {
    pub solver: SolanaAddress,
    pub order_uids: Vec<SolanaOrderUid>,
}

pub struct SolanaRanking {
    pub winners: Vec<SolanaSolution>,
    pub non_winners: Vec<SolanaSolution>,
}

impl Chain for SolanaChain {
    type Auction = SolanaAuction;
    type AuctionId = i64;
    type OrderUid = SolanaOrderUid;
    type Ranking = SolanaRanking;
    type Solution = SolanaSolution;
    // slot by which winners must have settled
    type SubmissionDeadline = u64;
    type Tip = SolanaTip;
}

impl AuctionInfo<SolanaChain> for SolanaAuction {
    fn id(&self) -> i64 {
        self.id
    }
}

impl RankingInfo<SolanaChain> for SolanaRanking {
    fn winner_count(&self) -> usize {
        self.winners.len()
    }

    fn winning_order_uids(&self) -> HashSet<SolanaOrderUid> {
        self.winners
            .iter()
            .flat_map(|solution| solution.order_uids.iter().copied())
            .collect()
    }

    fn considered_order_uids(&self) -> HashSet<SolanaOrderUid> {
        self.non_winners
            .iter()
            .flat_map(|solution| solution.order_uids.iter().copied())
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
    async fn solve(&self, _auction: &SolanaAuction) -> Vec<SolanaSolution> {
        unimplemented!("spike: no solana driver protocol exists yet")
    }
}

pub struct SolanaWinnerSelection;

impl WinnerSelection<SolanaChain> for SolanaWinnerSelection {
    fn arbitrate(
        &self,
        _solutions: Vec<SolanaSolution>,
        _auction: &SolanaAuction,
    ) -> SolanaRanking {
        unimplemented!("spike: winner-selection crate is Address and U256 typed")
    }
}

pub struct SolanaSettlementExecutor;

#[async_trait]
impl SettlementExecutor<SolanaChain> for SolanaSettlementExecutor {
    fn submission_deadline(&self, _tip: &SolanaTip) -> u64 {
        unimplemented!("spike: solana submission windows are undecided")
    }

    async fn execute(&self, _auction_id: i64, _ranking: &SolanaRanking, _deadline: u64) {
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
        _ranking: &SolanaRanking,
        _deadline: u64,
    ) -> anyhow::Result<()> {
        unimplemented!("spike: no solana persistence exists yet")
    }

    fn orders_matched(
        &self,
        _executing: HashSet<SolanaOrderUid>,
        _considered: HashSet<SolanaOrderUid>,
    ) {
        unimplemented!("spike: no solana persistence exists yet")
    }

    fn competition_ended(&self, _auction: &SolanaAuction, _ranking: &SolanaRanking) {
        unimplemented!("spike: no solana persistence exists yet")
    }
}
