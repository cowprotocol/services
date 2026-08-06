//! Auction context for winner selection.
//!
//! The auction context provides additional data needed for winner selection
//! that isn't part of the solution itself. This data comes from the auction
//! and is the same for all solutions.

use {
    crate::{chain::Scoring, evm::Evm, primitives::FeePolicy},
    std::collections::{HashMap, HashSet},
};

/// Auction context needed for winner selection.
///
/// This contains auction-level data that's needed to run the winner selection
/// algorithm but isn't part of individual solutions. Both autopilot and driver
/// build this from their respective auction representations.
pub struct AuctionContext<C: Scoring = Evm> {
    /// Fee policies for each order in the auction.
    ///
    /// Maps order UID to the list of fee policies that apply to that order.
    /// Fee policies determine how protocol fees are calculated.
    pub fee_policies: HashMap<C::OrderUid, Vec<FeePolicy<C>>>,

    /// Addresses that are allowed to create JIT orders that count toward score.
    ///
    /// JIT (Just-In-Time) orders created by these addresses will contribute to
    /// the solution's score during winner selection.
    pub surplus_capturing_jit_order_owners: HashSet<C::AccountId>,

    /// Native token prices for all tokens in the auction.
    ///
    /// These prices are used to convert token amounts to native token
    /// for score calculation. Maps token address to its price in native token.
    pub native_prices: HashMap<C::TokenId, C::Amount>,
}

// Manual impl because `derive(Default)` would wrongly require `C: Default`.
impl<C: Scoring> Default for AuctionContext<C> {
    fn default() -> Self {
        Self {
            fee_policies: HashMap::default(),
            surplus_capturing_jit_order_owners: HashSet::default(),
            native_prices: HashMap::default(),
        }
    }
}

impl<C: Scoring> AuctionContext<C> {
    /// Check if an order contributes to the solution's score.
    ///
    /// An order contributes to score if:
    /// 1. It has fee policies defined (it's a user order from the auction), OR
    /// 2. It's a JIT order from an allowed surplus-capturing owner
    pub fn contributes_to_score(&self, uid: &C::OrderUid) -> bool {
        self.fee_policies.contains_key(uid)
            || C::uid_owner(uid)
                .is_some_and(|owner| self.surplus_capturing_jit_order_owners.contains(&owner))
    }
}
