//! Auction context for winner selection.
//!
//! The auction context provides additional data needed for winner selection
//! that isn't part of the solution itself. This data comes from the auction
//! and is the same for all solutions.

use {
    crate::primitives::{FeePolicy, OrderUid},
    alloy::primitives::{Address, U256},
    std::collections::{HashMap, HashSet},
};

/// Auction context needed for winner selection.
///
/// This contains auction-level data that's needed to run the winner selection
/// algorithm but isn't part of individual solutions. Both autopilot and driver
/// build this from their respective auction representations.
#[derive(Default)]
pub struct AuctionContext {
    /// Fee policies for each order in the auction.
    ///
    /// Maps order UID to the list of fee policies that apply to that order.
    /// Fee policies determine how protocol fees are calculated.
    pub fee_policies: HashMap<OrderUid, Vec<FeePolicy>>,

    /// Addresses that are allowed to create JIT orders that count toward score.
    ///
    /// JIT (Just-In-Time) orders created by these addresses will contribute to
    /// the solution's score during winner selection.
    pub surplus_capturing_jit_order_owners: HashSet<Address>,

    /// Native token prices for all tokens in the auction.
    ///
    /// These prices are used to convert token amounts to native token
    /// for score calculation. Maps token address to its price in native token.
    pub native_prices: HashMap<Address, U256>,
}

impl AuctionContext {
    /// Check if an order contributes to the solution's score.
    ///
    /// An order contributes to score if:
    /// 1. It has fee policies defined (it's a user order from the auction), OR
    /// 2. It's a JIT order from an allowed surplus-capturing owner
    pub fn contributes_to_score(&self, uid: &OrderUid) -> bool {
        self.fee_policies.contains_key(uid)
            || self
                .surplus_capturing_jit_order_owners
                .contains(&uid.owner())
    }
}
