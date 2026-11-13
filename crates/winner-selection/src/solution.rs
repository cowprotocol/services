//! Minimal solution and order data structures.
//!
//! These structs contain only the data needed for winner selection,
//! making them small enough to efficiently send to/from the Pod Service.

pub use {
    protocol_fees::Order,
    state::{RankType, Unscored},
};

use crate::{
    chain::ChainTypes,
    evm::{Evm, U256},
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
