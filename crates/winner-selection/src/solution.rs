//! Minimal solution and order data structures.
//!
//! These structs contain only the data needed for winner selection,
//! making them small enough to efficiently send to/from the Pod Service.

use {
    crate::primitives::{OrderUid, Side},
    alloy::primitives::{Address, U256},
    std::collections::HashMap,
};

/// Minimal solution data needed for winner selection.
///
/// This contains only what's absolutely necessary to run the winner selection
/// algorithm. Autopilot and driver convert their full solution types to this
/// minimal format before sending to the Pod Service.
///
/// Estimated size: ~1.7KB for a solution with 5 orders and 10 unique tokens.
#[derive(Debug, Clone)]
pub struct Solution<State = Ranked> {
    /// Solution ID from solver (unique per solver).
    pub id: u64,

    /// Solver's submission address (used for identifying the solver).
    pub solver: Address,

    /// Orders executed in this solution.
    ///
    /// Uses Vec instead of HashMap for smaller serialization size.
    pub orders: Vec<Order>,

    /// Uniform clearing prices for all tokens in the solution.
    ///
    /// Maps token address to its price in the native token (ETH/XDAI).
    /// These are the prices at which all orders trading these tokens are
    /// settled.
    pub prices: HashMap<Address, U256>,

    /// State marker (score and ranking information).
    state: State,
}

/// Solution that hasn't been scored yet.
#[derive(Debug, Clone)]
pub struct Unscored;

/// Solution with a computed score.
#[derive(Debug, Clone)]
pub struct Scored {
    pub score: U256,
}

/// Solution with ranking information.
#[derive(Debug, Clone)]
pub struct Ranked {
    pub rank_type: RankType,
    pub score: U256,
}

/// The type of ranking assigned to a solution.
#[derive(Debug, Clone, Copy)]
pub enum RankType {
    Winner,
    NonWinner,
    FilteredOut,
}

impl<T> Solution<T> {
    /// Get the solution ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the solver address.
    pub fn solver(&self) -> Address {
        self.solver
    }

    /// Get the orders.
    pub fn orders(&self) -> &[Order] {
        &self.orders
    }

    /// Get the clearing prices.
    pub fn prices(&self) -> &HashMap<Address, U256> {
        &self.prices
    }
}

impl Solution<Unscored> {
    /// Create a new unscored solution.
    pub fn new(
        id: u64,
        solver: Address,
        orders: Vec<Order>,
        prices: HashMap<Address, U256>,
    ) -> Self {
        Self {
            id,
            solver,
            orders,
            prices,
            state: Unscored,
        }
    }

    /// Add a score to this solution.
    pub fn with_score(self, score: U256) -> Solution<Scored> {
        Solution {
            id: self.id,
            solver: self.solver,
            orders: self.orders,
            prices: self.prices,
            state: Scored { score },
        }
    }
}

impl Solution<Scored> {
    /// Get the score.
    pub fn score(&self) -> U256 {
        self.state.score
    }

    /// Rank this solution.
    pub fn rank(self, rank_type: RankType) -> Solution<Ranked> {
        Solution {
            id: self.id,
            solver: self.solver,
            orders: self.orders,
            prices: self.prices,
            state: Ranked {
                rank_type,
                score: self.state.score,
            },
        }
    }
}

impl Solution<Ranked> {
    /// Get the score.
    pub fn score(&self) -> U256 {
        self.state.score
    }

    /// Check if this solution is a winner.
    pub fn is_winner(&self) -> bool {
        matches!(self.state.rank_type, RankType::Winner)
    }

    /// Check if this solution was filtered out.
    pub fn is_filtered_out(&self) -> bool {
        matches!(self.state.rank_type, RankType::FilteredOut)
    }
}

/// Minimal order data needed for winner selection.
///
/// Contains the essential information about how an order was executed,
/// including limit amounts (from the original order) and executed amounts
/// (what actually happened in this solution).
///
/// Estimated size: ~225 bytes per order.
#[derive(Debug, Clone)]
pub struct Order {
    /// Unique order identifier (56 bytes).
    pub uid: OrderUid,

    /// Sell token address.
    pub sell_token: Address,

    /// Buy token address.
    pub buy_token: Address,

    /// Limit amount of sell token (from original order parameters).
    ///
    /// This is the maximum amount the user is willing to sell.
    pub sell_amount: U256,

    /// Limit amount of buy token (from original order parameters).
    ///
    /// This is the minimum amount the user wants to receive.
    pub buy_amount: U256,

    /// Amount of sell token that left the user's wallet (including fees).
    ///
    /// This is the actual executed amount in this solution.
    pub executed_sell: U256,

    /// Amount of buy token the user received (after fees).
    ///
    /// This is the actual amount the user got in this solution.
    pub executed_buy: U256,

    /// Order side (Buy or Sell).
    ///
    /// Determines how surplus is calculated.
    pub side: Side,
}
