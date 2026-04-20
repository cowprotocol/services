//! Minimal solution and order data structures.
//!
//! These structs contain only the data needed for winner selection,
//! making them small enough to efficiently send to/from the Pod Service.

pub use state::{RankType, Unscored};
use {
    crate::{
        primitives::{OrderUid, Side},
        state,
    },
    alloy_primitives::{Address, U256},
};
pub type Scored = state::Scored<U256>;
pub type Ranked = state::Ranked<U256>;

/// Minimal solution data needed for winner selection.
///
/// This contains only what's absolutely necessary to run the winner selection
/// algorithm.
#[derive(Debug, Clone)]
pub struct Solution<State> {
    /// Solution ID from solver (unique per solver).
    id: u64,

    /// Solver's submission address (used for identifying the solver).
    solver: Address,

    /// Orders executed in this solution.
    orders: Vec<Order>,

    /// State marker (score and ranking information).
    state: State,
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
}

impl<State> state::HasState for Solution<State> {
    type Next<NewState> = Solution<NewState>;
    type State = State;

    fn with_state<NewState>(self, state: NewState) -> Self::Next<NewState> {
        Solution {
            id: self.id,
            solver: self.solver,
            orders: self.orders,
            state,
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl Solution<Unscored> {
    /// Create a new unscored solution.
    pub fn new(id: u64, solver: Address, orders: Vec<Order>) -> Self {
        Self {
            id,
            solver,
            orders,
            state: Unscored,
        }
    }
}

/// Minimal order data needed for winner selection.
///
/// Contains the essential information about how an order was executed,
/// including limit amounts (from the original order) and executed amounts
/// (what actually happened in this solution).
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
