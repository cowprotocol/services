pub mod blockscout;
pub mod liquidity;

use self::{
    blockscout::BlockscoutTokenOwnerFinder,
    liquidity::{BalancerVaultFinder, FeeValues, UniswapLikePairProviderFinder, UniswapV3Finder},
};
use crate::{baseline_solver::BaseTokens, sources::uniswap_v2::pair_provider::PairProvider};
use anyhow::Result;
use contracts::{BalancerV2Vault, IUniswapV3Factory};
use primitive_types::H160;
use std::sync::Arc;

/// To detect bad tokens we need to find some address on the network that owns the token so that we
/// can use it in our simulations.
#[async_trait::async_trait]
pub trait TokenOwnerFinding: Send + Sync {
    /// Find candidate addresses that might own the token.
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>>;
}

/// Support token owner finding strategies.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ArgEnum)]
pub enum TokenOwnerFindingStrategy {
    /// Using baseline liquidity pools as token owners.
    ///
    /// The actual liquidity pools used depends on the configured baseline
    /// liquidity.
    Liquidity,

    /// Use the Blockscout token holder API to find token holders.
    Blockscout,
}

impl TokenOwnerFindingStrategy {
    /// Returns the default set of token owner finding strategies.
    pub fn defaults_for_chain(chain_id: u64) -> &'static [Self] {
        match chain_id {
            1 | 100 => &[Self::Liquidity, Self::Blockscout],
            _ => &[Self::Liquidity],
        }
    }
}

/// Initializes a set of token owner finders.
#[allow(clippy::too_many_arguments)]
pub async fn init(
    finders: Option<&[TokenOwnerFindingStrategy]>,
    pair_providers: &[PairProvider],
    base_tokens: &BaseTokens,
    vault: Option<&BalancerV2Vault>,
    uniswapv3_factory: Option<&IUniswapV3Factory>,
    current_block: u64,
    uniswapv3_fee_values: FeeValues,
    client: &reqwest::Client,
    chain_id: u64,
) -> Result<Vec<Arc<dyn TokenOwnerFinding>>> {
    let finders =
        finders.unwrap_or_else(|| TokenOwnerFindingStrategy::defaults_for_chain(chain_id));
    tracing::debug!(?finders, "initializing token owner finders");

    let mut result = Vec::<Arc<dyn TokenOwnerFinding>>::new();

    if finders.contains(&TokenOwnerFindingStrategy::Liquidity) {
        result.extend(
            pair_providers
                .iter()
                .map(|provider| -> Arc<dyn TokenOwnerFinding> {
                    Arc::new(UniswapLikePairProviderFinder {
                        inner: provider.clone(),
                        base_tokens: base_tokens.tokens().iter().copied().collect(),
                    })
                }),
        );
        if let Some(contract) = vault {
            result.push(Arc::new(BalancerVaultFinder(contract.clone())));
        }
        if let Some(contract) = uniswapv3_factory {
            result.push(Arc::new(
                UniswapV3Finder::new(
                    contract.clone(),
                    base_tokens.tokens().iter().copied().collect(),
                    current_block,
                    uniswapv3_fee_values,
                )
                .await?,
            ));
        }
    }

    if finders.contains(&TokenOwnerFindingStrategy::Liquidity) {
        result.push(Arc::new(BlockscoutTokenOwnerFinder::try_with_network(
            client.clone(),
            chain_id,
        )?));
    }

    Ok(result)
}
