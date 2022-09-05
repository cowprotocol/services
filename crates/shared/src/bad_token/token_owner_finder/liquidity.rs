//! Module containing liquidity-based token owner finding implementations.

use super::TokenOwnerProposing;
use crate::sources::{uniswap_v2::pair_provider::PairProvider, uniswap_v3_pair_provider};
use anyhow::Result;
use contracts::{BalancerV2Vault, IUniswapV3Factory};
use ethcontract::{BlockNumber, H160};
use model::TokenPair;

pub struct UniswapLikePairProviderFinder {
    pub inner: PairProvider,
    pub base_tokens: Vec<H160>,
}

#[async_trait::async_trait]
impl TokenOwnerProposing for UniswapLikePairProviderFinder {
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>> {
        Ok(self
            .base_tokens
            .iter()
            .filter_map(|&base_token| TokenPair::new(base_token, token))
            .map(|pair| self.inner.pair_address(&pair))
            .collect())
    }
}

/// The balancer vault contract contains all the balances of all pools.
pub struct BalancerVaultFinder(pub BalancerV2Vault);

#[async_trait::async_trait]
impl TokenOwnerProposing for BalancerVaultFinder {
    async fn find_candidate_owners(&self, _: H160) -> Result<Vec<H160>> {
        Ok(vec![self.0.address()])
    }
}

pub struct UniswapV3Finder {
    pub factory: IUniswapV3Factory,
    pub base_tokens: Vec<H160>,
    fee_values: Vec<u32>,
}

#[derive(Debug, Clone, Copy, clap::ArgEnum)]
pub enum FeeValues {
    /// Use hardcoded list
    Static,
    /// Fetch on creation based on events queried from node.
    /// Some nodes struggle with the request and take a long time to respond leading to timeouts.
    Dynamic,
}

impl UniswapV3Finder {
    pub async fn new(
        factory: IUniswapV3Factory,
        base_tokens: Vec<H160>,
        fee_values: FeeValues,
    ) -> Result<Self> {
        let fee_values = match fee_values {
            FeeValues::Static => vec![500, 3000, 10000, 100],
            // We fetch these once at start up because we don't expect them to change often.
            // Alternatively could use a time based cache.
            FeeValues::Dynamic => Self::fee_values(&factory).await?,
        };
        tracing::debug!(?fee_values);
        Ok(Self {
            factory,
            base_tokens,
            fee_values,
        })
    }

    // Possible fee values as given by
    // https://github.com/Uniswap/v3-core/blob/9161f9ae4aaa109f7efdff84f1df8d4bc8bfd042/contracts/UniswapV3Factory.sol#L26
    async fn fee_values(factory: &IUniswapV3Factory) -> Result<Vec<u32>> {
        // We expect there to be few of these kind of events (currently there are 4) so fetching all
        // of them is fine. Alternatively we could index these events in the database.
        let events = factory
            .events()
            .fee_amount_enabled()
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .query()
            .await?;
        let fee_values = events.into_iter().map(|event| event.data.fee).collect();
        Ok(fee_values)
    }
}

#[async_trait::async_trait]
impl TokenOwnerProposing for UniswapV3Finder {
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>> {
        Ok(self
            .base_tokens
            .iter()
            .filter_map(|base_token| TokenPair::new(*base_token, token))
            .flat_map(|pair| self.fee_values.iter().map(move |fee| (pair, *fee)))
            .map(|(pair, fee)| {
                uniswap_v3_pair_provider::pair_address(&self.factory.address(), &pair, fee)
            })
            .collect())
    }
}
