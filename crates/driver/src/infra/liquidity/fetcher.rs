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
    fetch_at_block: AtBlock,
}

/// Specifies at which block liquidity should be fetched.
#[derive(Debug, Clone)]
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
    Recent {
        /// Whether the requested pairs get kept up to date in the background.
        /// Quotes don't need that. Auctions that want to use Recent for
        /// perfromance reasons do.
        schedule_update: bool,
    },
    /// Fetches liquidity for the latest state of the blockchain.
    Latest,
}

impl Fetcher {
    /// Creates a new liquidity fetcher for the specified Ethereum instance and
    /// configuration.
    pub async fn try_new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self, Error> {
        let eth = eth.with_metric_label("liquidity".into());
        let inner = boundary::liquidity::Fetcher::try_new(&eth, config).await?;
        Ok(Self {
            inner: Arc::new(inner),
            fetch_at_block: config.fetch_at_block.clone(),
        })
    }

    /// The block at which liquidity for `/solve` requests gets fetched.
    pub fn fetch_at_block(&self) -> AtBlock {
        self.fetch_at_block.clone()
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
