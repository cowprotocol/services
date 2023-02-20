//! Legacy HTTP solver adapter implementation.
//!
//! In order to faciliate the transition from the legacy HTTP solver API to the
//! new HTTP API, we provide a solver "wrapper" that just marshals the API
//! request types to and from the legacy format.

use crate::{
    boundary,
    domain::{auction, solution},
    infra::config::legacy::LegacyConfig,
};

pub struct Legacy(boundary::legacy::Legacy);

impl Legacy {
    pub fn new(config: LegacyConfig) -> Self {
        Self(boundary::legacy::Legacy::new(config))
    }

    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        match self.0.solve(auction).await {
            Ok(solution) => vec![solution],
            Err(err) => {
                tracing::warn!(?err, "failed to solve auction");
                vec![]
            }
        }
    }
}
