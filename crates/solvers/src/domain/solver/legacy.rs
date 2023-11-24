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
    pub gzip_requests: bool,
}

pub struct Legacy(boundary::legacy::Legacy);

impl Legacy {
    pub fn new(config: Config) -> Self {
        Self(boundary::legacy::Legacy::new(config))
    }

    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        match self.0.solve(&auction).await {
            Ok(solution) => {
                if solution.is_empty() {
                    vec![]
                } else {
                    vec![solution]
                }
            }
            Err(err) => {
                tracing::warn!(?err, "failed to solve auction");
                if err.is_timeout() {
                    self.notify_timeout(auction.id)
                }
                vec![]
            }
        }
    }

    pub fn notify(&self, notification: notification::Notification) {
        self.0.notify(notification);
    }

    fn notify_timeout(&self, auction_id: auction::Id) {
        self.notify(notification::Notification {
            auction_id,
            solution_id: None,
            kind: notification::Kind::Timeout,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("timeout")]
    Timeout,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Error {
    fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }
}
