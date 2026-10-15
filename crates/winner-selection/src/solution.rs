//! Minimal solution and order data structures.
//!
//! These structs contain only the data needed for winner selection,
//! making them small enough to efficiently send to/from the Pod Service.

pub use state::{RankType, Unscored};

use crate::{
    chain::ChainTypes,
    evm::{Evm, U256},
    primitives::Side,
    state,
};
pub type Scored = state::Scored<U256>;
pub type Ranked = state::Ranked<U256>;

/// Minimal solution data needed for winner selection.
///
/// This contains only what's absolutely necessary to run the winner selection
/// algorithm.
#[derive(Debug, Clone)]
pub struct Solution<State, C: ChainTypes = Evm> {
    /// Solution ID from solver (unique per solver).
    id: u64,

    /// Solver's submission address (used for identifying the solver).
    solver: C::AccountId,

    /// Orders executed in this solution.
    orders: Vec<Order<C>>,

    /// State marker (score and ranking information).
    state: State,
}

impl<T, C: ChainTypes> Solution<T, C> {
    /// Get the solution ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the solver address.
    pub fn solver(&self) -> C::AccountId {
        self.solver
    }

    /// Get the orders.
    pub fn orders(&self) -> &[Order<C>] {
        &self.orders
    }
}

impl<State, C: ChainTypes> state::HasState for Solution<State, C> {
    type Next<NewState> = Solution<NewState, C>;
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

impl<C: ChainTypes> Solution<Unscored, C> {
    /// Create a new unscored solution.
    pub fn new(id: u64, solver: C::AccountId, orders: Vec<Order<C>>) -> Self {
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
pub struct Order<C: ChainTypes = Evm> {
    /// Unique order identifier.
    pub uid: C::OrderUid,

    /// Sell token address.
    pub sell_token: C::TokenId,

    /// Buy token address.
    pub buy_token: C::TokenId,

    /// Limit amount of sell token (from original order parameters).
    ///
    /// This is the maximum amount the user is willing to sell.
    pub sell_amount: C::Amount,

    /// Limit amount of buy token (from original order parameters).
    ///
    /// This is the minimum amount the user wants to receive.
    pub buy_amount: C::Amount,

    /// Amount of sell token that left the user's wallet (including fees).
    ///
    /// This is the actual executed amount in this solution.
    pub executed_sell: C::Amount,

    /// Amount of buy token the user received (after fees).
    ///
    /// This is the actual amount the user got in this solution.
    pub executed_buy: C::Amount,

    /// Order side (Buy or Sell).
    ///
    /// Determines how surplus is calculated.
    pub side: Side,
}
