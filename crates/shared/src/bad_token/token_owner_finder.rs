pub mod blockscout;
pub mod ethplorer;
pub mod liquidity;

use self::{
    blockscout::BlockscoutTokenOwnerFinder,
    liquidity::{BalancerVaultFinder, FeeValues, UniswapLikePairProviderFinder, UniswapV3Finder},
};
use crate::{
    arguments::duration_from_seconds,
    bad_token::token_owner_finder::ethplorer::EthplorerTokenOwnerFinder,
    baseline_solver::BaseTokens, ethcontract_error::EthcontractErrorType,
    http_client::HttpClientFactory, rate_limiter::RateLimitingStrategy,
    sources::uniswap_v2::pair_provider::PairProvider, transport::MAX_BATCH_SIZE, Web3,
    Web3CallBatch,
};
use anyhow::Result;
use contracts::{BalancerV2Vault, IUniswapV3Factory, ERC20};
use ethcontract::U256;
use futures::{Stream, StreamExt as _};
use primitive_types::H160;
use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
    time::Duration,
};

/// This trait abstracts various sources for proposing token owner candidates which are likely, but
/// not guaranteed, to have some token balance.
#[async_trait::async_trait]
pub trait TokenOwnerProposing: Send + Sync {
    /// Find candidate addresses that might own the token.
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>>;
}

/// To detect bad tokens we need to find some address on the network that owns the token so that we
/// can use it in our simulations.
#[async_trait::async_trait]
pub trait TokenOwnerFinding: Send + Sync {
    /// Find an addresses with at least `min_balance` of tokens and return it, along with its
    /// actual balance.
    async fn find_owner(&self, token: H160, min_balance: U256) -> Result<Option<(H160, U256)>>;
}

/// Arguments related to the token owner finder.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// The token owner finding strategies to use.
    #[clap(long, env, use_value_delimiter = true, value_enum)]
    pub token_owner_finders: Option<Vec<TokenOwnerFindingStrategy>>,

    /// The fee value strategy to use for locating Uniswap V3 pools as token holders for bad token
    /// detection.
    #[clap(long, env, default_value = "static", value_enum)]
    pub token_owner_finder_uniswap_v3_fee_values: FeeValues,

    /// Override the Blockscout token owner finder-specific timeout configuration.
    #[clap(long, value_parser = duration_from_seconds, default_value = "45")]
    pub blockscout_http_timeout: Duration,

    /// The Ethplorer token holder API key.
    pub ethplorer_api_key: Option<String>,

    /// Token owner finding rate limiting strategy. See --price-estimation-rate-limiter
    /// documentation for format details.
    #[clap(long, env)]
    pub token_owner_finder_rate_limiter: Option<RateLimitingStrategy>,
}

/// Support token owner finding strategies.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum TokenOwnerFindingStrategy {
    /// Using baseline liquidity pools as token owners.
    ///
    /// The actual liquidity pools used depends on the configured baseline
    /// liquidity.
    Liquidity,

    /// Use the Blockscout token holder API to find token holders.
    Blockscout,

    /// Use the Ethplorer token holder API.
    Ethplorer,
}

impl TokenOwnerFindingStrategy {
    /// Returns the default set of token owner finding strategies.
    pub fn defaults_for_chain(chain_id: u64) -> &'static [Self] {
        match chain_id {
            1 => &[Self::Liquidity, Self::Blockscout, Self::Ethplorer],
            100 => &[Self::Liquidity, Self::Blockscout],
            _ => &[Self::Liquidity],
        }
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "token_owner_finders: {:?}", self.token_owner_finders)?;
        writeln!(
            f,
            "token_owner_finder_uniswap_v3_fee_values: {:?}",
            self.token_owner_finder_uniswap_v3_fee_values
        )?;
        writeln!(
            f,
            "token_owner_finder_http_timeout: {:?}",
            self.token_owner_finders
        )?;

        Ok(())
    }
}

/// Initializes a set of token owner finders.
#[allow(clippy::too_many_arguments)]
pub async fn init(
    args: &Arguments,
    web3: Web3,
    chain_id: u64,
    http_factory: &HttpClientFactory,
    pair_providers: &[PairProvider],
    vault: Option<&BalancerV2Vault>,
    uniswapv3_factory: Option<&IUniswapV3Factory>,
    base_tokens: &BaseTokens,
) -> Result<Arc<dyn TokenOwnerFinding>> {
    let finders = args
        .token_owner_finders
        .as_deref()
        .unwrap_or_else(|| TokenOwnerFindingStrategy::defaults_for_chain(chain_id));
    tracing::debug!(?finders, "initializing token owner finders");

    let mut proposers = Vec::<Arc<dyn TokenOwnerProposing>>::new();

    if finders.contains(&TokenOwnerFindingStrategy::Liquidity) {
        proposers.extend(
            pair_providers
                .iter()
                .map(|provider| -> Arc<dyn TokenOwnerProposing> {
                    Arc::new(UniswapLikePairProviderFinder {
                        inner: provider.clone(),
                        base_tokens: base_tokens.tokens().iter().copied().collect(),
                    })
                }),
        );
        if let Some(contract) = vault {
            proposers.push(Arc::new(BalancerVaultFinder(contract.clone())));
        }
        if let Some(contract) = uniswapv3_factory {
            proposers.push(Arc::new(
                UniswapV3Finder::new(
                    contract.clone(),
                    base_tokens.tokens().iter().copied().collect(),
                    args.token_owner_finder_uniswap_v3_fee_values,
                )
                .await?,
            ));
        }
    }

    if finders.contains(&TokenOwnerFindingStrategy::Blockscout) {
        proposers.push(Arc::new(BlockscoutTokenOwnerFinder::try_with_network(
            http_factory.configure(|builder| builder.timeout(args.blockscout_http_timeout)),
            chain_id,
        )?));
    }

    if finders.contains(&TokenOwnerFindingStrategy::Ethplorer) {
        let mut ethplorer = EthplorerTokenOwnerFinder::try_with_network(
            http_factory.create(),
            args.ethplorer_api_key.clone(),
            chain_id,
        )?;
        if let Some(strategy) = args.token_owner_finder_rate_limiter.clone() {
            ethplorer.with_rate_limiter(strategy);
        }
        proposers.push(Arc::new(ethplorer));
    }

    Ok(Arc::new(TokenOwnerFinder { web3, proposers }))
}

/// A `TokenOwnerFinding` implementation that queries a node with proposed owner candidates from an
/// internal list of `TokenOwnerProposing` implementations.
pub struct TokenOwnerFinder {
    pub web3: Web3,
    pub proposers: Vec<Arc<dyn TokenOwnerProposing>>,
}

impl TokenOwnerFinder {
    /// Stream of addresses that might own the token.
    fn candidate_owners(&self, token: H160) -> impl Stream<Item = H160> + '_ {
        // Combine the results of all finders into a single stream.
        let streams = self.proposers.iter().map(|finder| {
            futures::stream::once(finder.find_candidate_owners(token))
                .filter_map(|result| async {
                    match result {
                        Ok(inner) => Some(futures::stream::iter(inner)),
                        Err(err) => {
                            tracing::warn!(?err, "token owner proposing failed");
                            None
                        }
                    }
                })
                .flatten()
                .boxed()
        });
        futures::stream::select_all(streams)
    }
}

#[async_trait::async_trait]
impl TokenOwnerFinding for TokenOwnerFinder {
    async fn find_owner(&self, token: H160, min_balance: U256) -> Result<Option<(H160, U256)>> {
        let instance = ERC20::at(&self.web3, token);

        // We use a stream with ready_chunks so that we can start with the addresses of fast
        // TokenOwnerFinding implementations first without having to wait for slow ones.
        let stream = self.candidate_owners(token).ready_chunks(MAX_BATCH_SIZE);
        futures::pin_mut!(stream);

        while let Some(chunk) = stream.next().await {
            let mut batch = Web3CallBatch::new(self.web3.transport().clone());
            let futures = chunk
                .iter()
                .map(|&address| {
                    let balance = instance.balance_of(address).batch_call(&mut batch);
                    async move {
                        let balance = match balance.await {
                            Ok(balance) => Some(balance),
                            Err(err) if EthcontractErrorType::is_contract_err(&err) => None,
                            Err(err) => return Err(err),
                        };

                        Ok((address, balance))
                    }
                })
                .collect::<Vec<_>>();

            batch.execute_all(MAX_BATCH_SIZE).await;
            let balances = futures::future::try_join_all(futures).await?;

            if let Some(holder) = balances
                .into_iter()
                .filter_map(|(address, balance)| Some((address, balance?)))
                .find(|(_, balance)| *balance >= min_balance)
            {
                return Ok(Some(holder));
            }
        }

        Ok(None)
    }
}
