use {
    crate::{
        boundary,
        domain::liquidity,
        infra::{self, blockchain::Ethereum, observe},
    },
    std::{collections::HashSet, sync::Arc},
};

/// Fetch liquidity for auctions to be sent to solver engines.
#[derive(Clone, Debug)]
pub struct Fetcher {
    inner: Arc<boundary::liquidity::Fetcher>,
}

/// Specifies at which block liquidity should be fetched.
pub enum AtBlock {
    /// Fetches liquidity at a recent block. This will prefer reusing cached
    /// liquidity even if it is stale by a few blocks instead of fetching the
    /// absolute latest state from the blockchain.
    ///
    /// This is useful for quoting where we want an up-to-date, but not
    /// necessarily exactly correct price. In the context of quote verification,
    /// this is completely fine as the exactly input and output amounts will be
    /// computed anyway. At worse, we might provide a slightly sub-optimal
    /// route in some cases, but this is an acceptable trade-off.
    Recent,
    /// Fetches liquidity liquidity for the latest state of the blockchain.
    Latest,
}

impl Fetcher {
    /// Creates a new liquidity fetcher for the specified Ethereum instance and
    /// configuration.
    pub async fn new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self, Error> {
        let eth = eth.with_metric_label("liquidity".into());
        let inner = boundary::liquidity::Fetcher::new(&eth, config).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Fetches all relevant liquidity for the specified token pairs. Handles
    /// failures by logging and returning an empty vector.
    pub async fn fetch(
        &self,
        pairs: &HashSet<liquidity::TokenPair>,
        block: AtBlock,
    ) -> Vec<liquidity::Liquidity> {
        observe::fetching_liquidity();
        match self.inner.fetch(pairs, block).await {
            Ok(liquidity) => {
                observe::fetched_liquidity(&liquidity);
                liquidity
            }
            Err(e) => {
                observe::fetching_liquidity_failed(&e);
                Default::default()
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
}
