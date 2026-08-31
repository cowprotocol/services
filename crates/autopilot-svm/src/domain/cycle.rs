//! The Solana instantiation of the auction loop's type vocabulary.

use {
    crate::{
        domain::auction::Auction,
        run_loop::{Cycle, RankingInfo},
    },
    chain_types::solana::{IntentHash, Pubkey, Solana},
    std::collections::{HashMap, HashSet},
    winner_selection::{Unscored, solution},
};

/// Marker type binding the generic loop to the Solana vocabulary.
pub struct SolanaCycle;

impl Cycle for SolanaCycle {
    type Auction = Auction;
    type OrderUid = IntentHash;
    type Ranking = Ranking;
    type Solution = Solution;
    /// The chain progress marker is the slot.
    type Tip = u64;

    fn submission_deadline(tip: &u64, allowance: u64) -> u64 {
        tip + allowance
    }
}

/// One driver's solution, attributed to the driver that proposed it so the
/// executor can dispatch the settlement back to it.
pub struct Solution {
    /// Index into the configured driver list.
    pub driver_index: usize,
    pub inner: solution::Solution<Unscored, Solana>,
}

/// Key attributing a ranked solution back to its driver. The arbitrator
/// keeps `(solver, solution id)` through ranking, the driver index does not
/// survive it.
pub type SolutionKey = (Pubkey, u64);

/// Arbitrated solutions plus the driver attribution the generic ranking
/// drops.
pub struct Ranking {
    pub inner: winner_selection::Ranking<Solana>,
    /// Driver index per solution, keyed by `(solver, solution id)`.
    pub drivers: HashMap<SolutionKey, usize>,
}

impl RankingInfo<SolanaCycle> for Ranking {
    fn winner_count(&self) -> usize {
        self.inner.winners().count()
    }

    fn winning_order_uids(&self) -> HashSet<IntentHash> {
        self.inner
            .winners()
            .flat_map(|solution| solution.orders().iter().map(|order| order.uid))
            .collect()
    }

    fn considered_order_uids(&self) -> HashSet<IntentHash> {
        self.inner
            .non_winners()
            .flat_map(|solution| solution.orders().iter().map(|order| order.uid))
            .collect()
    }
}
