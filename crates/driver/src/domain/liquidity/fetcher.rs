use crate::domain::{competition::auction, liquidity};

/// Fetch liquidity for auctions to be sent to solver engines.
#[derive(Debug)]
pub struct Fetcher {}

impl Fetcher {
    /// Fetches all relevant liquidity for the specified auction.
    pub async fn fetch(
        &self,
        _auction: &auction::Auction,
    ) -> Result<Vec<liquidity::Liquidity>, Error> {
        todo!()
    }
}

pub struct Error;
