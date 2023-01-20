use {
    crate::{
        boundary,
        domain::{competition, competition::order, liquidity, quote},
        infra::{self, blockchain::Ethereum, liquidity::TokenPair},
    },
    itertools::Itertools,
    std::sync::Arc,
};

/// Fetch liquidity for auctions to be sent to solver engines.
#[derive(Clone, Debug)]
pub struct Fetcher {
    inner: Arc<boundary::liquidity::Fetcher>,
}

impl Fetcher {
    /// Creates a new liquidity fetcher for the specified Ethereum instance and
    /// configuration.
    pub async fn new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self, Error> {
        let inner = boundary::liquidity::Fetcher::new(eth, config).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Fetches all relevant liquidity for the orders.
    pub async fn for_auction(
        &self,
        auction: &competition::Auction,
    ) -> Result<Vec<liquidity::Liquidity>, Error> {
        let pairs = auction
            .orders
            .iter()
            .filter_map(|order| match order.kind {
                order::Kind::Market | order::Kind::Limit { .. } => {
                    TokenPair::new(order.sell.token, order.buy.token)
                }
                order::Kind::Liquidity => None,
            })
            .collect_vec();
        let liquidity = self.inner.fetch(&pairs).await?;
        Ok(liquidity)
    }

    /// Fetches all liquidity relevant for a quote.
    pub async fn for_quote(
        &self,
        quote: &quote::Order,
    ) -> Result<Vec<liquidity::Liquidity>, Error> {
        let pair = TokenPair::new(quote.tokens.sell(), quote.tokens.buy())
            .expect("sell != buy by construction");
        let liquidity = self.inner.fetch(&[pair]).await?;
        Ok(liquidity)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
}
