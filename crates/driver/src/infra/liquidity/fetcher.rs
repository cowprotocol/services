use crate::{
    boundary,
    domain::{competition::order, liquidity},
    infra::{self, blockchain::Ethereum},
};

/// Fetch liquidity for auctions to be sent to solver engines.
#[derive(Debug)]
pub struct Fetcher {
    inner: boundary::liquidity::Fetcher,
}

impl Fetcher {
    /// Creates a new liquidity fetcher for the specified Ethereum instance and
    /// configuration.
    pub async fn new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self, Error> {
        let inner = boundary::liquidity::Fetcher::new(eth, config).await?;
        Ok(Self { inner })
    }

    /// Fetches all relevant liquidity for the orders.
    pub async fn fetch(&self, orders: &[order::Order]) -> Result<Vec<liquidity::Liquidity>, Error> {
        let liquidity = self.inner.fetch(orders).await?;
        Ok(liquidity)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
}
