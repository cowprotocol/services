pub use self::config::Config;
use crate::{
    boundary,
    domain::{competition::auction, liquidity},
    infra::blockchain::Ethereum,
};

pub mod config;

/// Fetch liquidity for auctions to be sent to solver engines.
#[derive(Debug)]
pub struct Fetcher {
    inner: boundary::liquidity::Fetcher,
}

impl Fetcher {
    /// Creates a new liquidity fetcher for the specified Ethereum instance and
    /// configuration.
    pub async fn new(eth: &Ethereum, config: &Config) -> Result<Self, Error> {
        let inner = boundary::liquidity::Fetcher::new(eth, config).await?;
        Ok(Self { inner })
    }

    /// Fetches all relevant liquidity for the specified auction.
    pub async fn fetch(
        &self,
        auction: &auction::Auction,
    ) -> Result<Vec<liquidity::Liquidity>, Error> {
        let liquidity = self.inner.fetch(auction).await?;
        Ok(liquidity)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
}
