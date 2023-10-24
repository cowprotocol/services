//! Legacy HTTP solver adapter implementation.
//!
//! In order to facilitate the transition from the legacy HTTP solver API to the
//! new HTTP API, we provide a solver "wrapper" that just marshals the API
//! request types to and from the legacy format.

use {
    crate::{
        boundary,
        domain::{auction, eth, notification, solution},
    },
    reqwest::Url,
};

pub struct Config {
    pub weth: eth::WethAddress,
    pub solver_name: String,
    pub chain_id: eth::ChainId,
    pub endpoint: Url,
}

pub struct Legacy(boundary::legacy::Legacy);

impl Legacy {
    pub fn new(config: Config) -> Self {
        Self(boundary::legacy::Legacy::new(config))
    }

    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        match self.0.solve(auction).await {
            Ok(solution) => vec![solution.with_id(solution::Id(0))],
            Err(err) => {
                tracing::warn!(?err, "failed to solve auction");
                vec![]
            }
        }
    }

    pub fn notify(&self, notification: notification::Notification) {
        self.0.notify(notification);
    }
}
