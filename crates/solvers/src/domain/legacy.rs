//! Legacy HTTP solver adapter implementation.
//!
//! In order to faciliate the transition from the legacy HTTP solver API to the
//! new HTTP API, we provide a solver "wrapper" that just marshals the API
//! request types to and from the legacy format.

use {
    crate::{
        boundary,
        domain::{auction, solution, Solver},
        infra::config::legacy::LegacyConfig,
    },
    futures::future::{BoxFuture, FutureExt},
};

pub struct Legacy(boundary::legacy::Legacy);

impl Legacy {
    pub fn new(config: LegacyConfig) -> Self {
        Self(boundary::legacy::Legacy::new(config))
    }
}

impl Solver for Legacy {
    fn solve(&self, auction: auction::Auction) -> BoxFuture<Vec<solution::Solution>> {
        async move {
            match self.0.solve(auction).await {
                Ok(solution) => vec![solution],
                Err(err) => {
                    tracing::warn!(?err, "failed to solve auction");
                    vec![]
                }
            }
        }
        .boxed()
    }
}
