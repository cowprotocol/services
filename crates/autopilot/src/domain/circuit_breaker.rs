//! This module is responsible for monitoring the solvers and checking the
//! solvers do not place a malicious settlement. If a solver is detected to
//! be malicious, the circuit breaker will be triggered and the solver will
//! be removed from the allow-list.

use {
    crate::{domain::eth, infra::blockchain::authenticator},
    anyhow::Result,
};

#[allow(dead_code)]
pub struct CircuitBreaker {
    authenticator: authenticator::Manager,
    solvers: Vec<eth::Address>,
}

impl CircuitBreaker {
    pub fn build(authenticator: authenticator::Manager, solvers: Vec<eth::Address>) -> Self {
        Self {
            authenticator,
            solvers,
        }
    }

    #[allow(dead_code)]
    async fn apply(&self) -> Result<bool> {
        todo!()
    }
}
