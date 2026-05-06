//! Generic typestate bid wrapper.
//!
//! `Bid<P, State>` pairs a domain payload `P` with a winner-selection state
//! marker. `P` is exposed via `Deref<Target = P>` so methods on the payload
//! are callable through the bid. Used together with
//! [`crate::Arbitrator::arbitrate_paired_and_rejoin`] to share the typestate
//! and rejoin glue across domains.

use crate::state::{HasState, Unscored};

pub struct Bid<P, State> {
    payload: P,
    state: State,
}

impl<P: Clone, State: Clone> Clone for Bid<P, State> {
    fn clone(&self) -> Self {
        Self {
            payload: self.payload.clone(),
            state: self.state.clone(),
        }
    }
}

impl<P: std::fmt::Debug, State: std::fmt::Debug> std::fmt::Debug for Bid<P, State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bid")
            .field("payload", &self.payload)
            .field("state", &self.state)
            .finish()
    }
}

impl<P, State> Bid<P, State> {
    pub fn payload(&self) -> &P {
        &self.payload
    }
}

impl<P> Bid<P, Unscored> {
    pub fn new(payload: P) -> Self {
        Self {
            payload,
            state: Unscored,
        }
    }
}

impl<P, State> std::ops::Deref for Bid<P, State> {
    type Target = P;

    fn deref(&self) -> &P {
        &self.payload
    }
}

impl<P, State> HasState for Bid<P, State> {
    type Next<NewState> = Bid<P, NewState>;
    type State = State;

    fn with_state<NewState>(self, state: NewState) -> Self::Next<NewState> {
        Bid {
            payload: self.payload,
            state,
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}
