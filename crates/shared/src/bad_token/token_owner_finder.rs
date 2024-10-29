pub mod blockscout;
pub mod ethplorer;
pub mod liquidity;
pub mod solvers;
pub mod token_owner_list;

use {
    self::{
        blockscout::BlockscoutTokenOwnerFinder,
        liquidity::{
            BalancerVaultFinder,
            FeeValues,
            UniswapLikePairProviderFinder,
            UniswapV3Finder,
        },
    },
    crate::{
        arguments::{display_list, display_option, display_secret_option},
        bad_token::token_owner_finder::{
            ethplorer::EthplorerTokenOwnerFinder,
            solvers::{
                solver_api::SolverConfiguration,
                solver_finder::AutoUpdatingSolverTokenOwnerFinder,
            },
            token_owner_list::TokenOwnerList,
        },
        baseline_solver::BaseTokens,
        ethcontract_error::EthcontractErrorType,
        ethrpc::{Web3, MAX_BATCH_SIZE},
        http_client::HttpClientFactory,
        sources::uniswap_v2::pair_provider::PairProvider,
    },
    anyhow::{Context, Result},
    contracts::{BalancerV2Vault, IUniswapV3Factory, ERC20},
    ethcontract::U256,
    futures::{Stream, StreamExt as _},
    primitive_types::H160,
    rate_limit::Strategy,
    reqwest::Url,
    std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        sync::Arc,
        time::Duration,
    },
};

/// This trait abstracts various sources for proposing token owner candidates
/// which are likely, but not guaranteed, to have some token balance.
#[async_trait::async_trait]
pub trait TokenOwnerProposing: Send + Sync {
    /// Find candidate addresses that might own the token.
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>>;
}

/// To detect bad tokens we need to find some address on the network that owns
/// the token so that we can use it in our simulations.
#[async_trait::async_trait]
pub trait TokenOwnerFinding: Send + Sync {
    /// Find an addresses with at least `min_balance` of tokens and return it,
    /// along with its actual balance.
    async fn find_owner(&self, token: H160, min_balance: U256) -> Result<Option<(H160, U256)>>;
}

/// Arguments related to the token owner finder.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// The token owner finding strategies to use.
    #[clap(long, env, use_value_delimiter = true, value_enum)]
    pub token_owner_finders: Option<Vec<TokenOwnerFindingStrategy>>,

    /// The fee value strategy to use for locating Uniswap V3 pools as token
    /// holders for bad token detection.
    #[clap(long, env, default_value = "static", value_enum)]
    pub token_owner_finder_uniswap_v3_fee_values: FeeValues,

    /// The blockscout configuration.
    #[clap(flatten)]
    pub blockscout: Option<Blockscout>,

    /// The ethplorer configuration.
    #[clap(flatten)]
    pub ethplorer: Option<Ethplorer>,

    /// Token owner finding rate limiting strategy. See
    /// --price-estimation-rate-limiter documentation for format details.
    #[clap(long, env)]
    pub token_owner_finder_rate_limiter: Option<Strategy>,

    /// List of token addresses to be whitelisted as a potential token owners
    /// For each token a list of owners is defined.
    #[clap(
        long,
        env,
        value_parser = parse_owners,
        default_value = "",
    )]
    pub whitelisted_owners: HashMap<H160, Vec<H160>>,

    /// The solvers urls to query the token owner pairs.
    #[clap(long, env, use_value_delimiter = true)]
    pub solver_token_owners_urls: Vec<Url>,

    /// Interval in seconds between consecutive queries to update the solver
    /// token owner pairs. Values should be in pair with
    /// `solver_token_owners_urls`
    #[clap(long, env, use_value_delimiter = true, value_parser = humantime::parse_duration)]
    pub solver_token_owners_cache_update_intervals: Vec<Duration>,
}

#[derive(clap::Parser)]
#[clap(group(
    clap::ArgGroup::new("blockscout")
    .requires_all(&[
    "blockscout_api_url",
    "blockscout_api_key",
    ])
    .multiple(true)
    .required(false),
))]
pub struct Blockscout {
    /// Override the default blockscout API url for this network
    #[clap(long, env, group = "blockscout", required = false)]
    pub blockscout_api_url: Url,

    /// The blockscout API key.
    #[clap(long, env, group = "blockscout", required = false)]
    pub blockscout_api_key: String,
}

#[derive(clap::Parser)]
#[clap(group(
    clap::ArgGroup::new("ethplorer")
    .requires_all(&[
    "ethplorer_api_url",
    "ethplorer_api_key",
    ])
    .multiple(true)
    .required(false),
))]
pub struct Ethplorer {
    /// Override the default ethplorer API url
    #[clap(long, env, group = "ethplorer", required = false)]
    pub ethplorer_api_url: Url,

    /// The Ethplorer token holder API key.
    #[clap(long, env, group = "ethplorer", required = false)]
    pub ethplorer_api_key: String,
}

fn parse_owners(s: &str) -> Result<HashMap<H160, Vec<H160>>> {
    if s.is_empty() {
        return Ok(Default::default());
    }
    s.split(';')
        .map(|pair_str| {
            let (key, values) = pair_str
                .split_once(':')
                .context("missing token and owners")?;
            let key = key.trim().parse()?;
            let values = values
                .trim()
                .split(',')
                .map(|value| value.trim().parse().context("failed to parse token owner"))
                .collect::<Result<_>>()?;
            Ok((key, values))
        })
        .collect()
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

    /// Use lists provided by the external solver teams
    Solvers,
}

impl TokenOwnerFindingStrategy {
    /// Returns the default set of token owner finding strategies.
    pub fn defaults_for_network(network: &network::Network) -> &'static [Self] {
        match network {
            network::Network::Mainnet => &[Self::Liquidity, Self::Blockscout, Self::Ethplorer],
            network::Network::Gnosis => &[Self::Liquidity, Self::Blockscout],
            network::Network::Sepolia
            | network::Network::Goerli
            | network::Network::ArbitrumOne
            | network::Network::Base => &[Self::Liquidity],
        }
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            token_owner_finders,
            token_owner_finder_uniswap_v3_fee_values,
            blockscout,
            ethplorer,
            token_owner_finder_rate_limiter,
            whitelisted_owners,
            solver_token_owners_urls,
            solver_token_owners_cache_update_intervals,
        } = self;

        writeln!(f, "token_owner_finders: {:?}", token_owner_finders)?;
        writeln!(
            f,
            "token_owner_finder_uniswap_v3_fee_values: {:?}",
            token_owner_finder_uniswap_v3_fee_values
        )?;
        display_option(
            f,
            "blockscout_api_url",
            &blockscout
                .as_ref()
                .map(|blockscout| blockscout.blockscout_api_url.clone()),
        )?;
        display_secret_option(
            f,
            "blockscout_api_key",
            blockscout
                .as_ref()
                .map(|blockscout| blockscout.blockscout_api_key.clone())
                .as_ref(),
        )?;
        display_option(
            f,
            "ethplorer_api_url",
            &ethplorer
                .as_ref()
                .map(|blockscout| blockscout.ethplorer_api_url.clone())
                .as_ref(),
        )?;
        display_secret_option(
            f,
            "ethplorer_api_key",
            ethplorer
                .as_ref()
                .map(|blockscout| blockscout.ethplorer_api_key.clone())
                .as_ref(),
        )?;
        display_option(
            f,
            "token_owner_finder_rate_limiter",
            token_owner_finder_rate_limiter,
        )?;
        writeln!(f, "whitelisted_owners, {:?}", whitelisted_owners)?;
        display_list(f, "solver_token_owners_urls", solver_token_owners_urls)?;
        writeln!(
            f,
            "solver_token_owners_cache_update_intervals, {:?}",
            solver_token_owners_cache_update_intervals
        )?;
        Ok(())
    }
}

/// Initializes a set of token owner finders.
#[allow(clippy::too_many_arguments)]
pub async fn init(
    args: &Arguments,
    web3: Web3,
    network: &network::Network,
    http_factory: &HttpClientFactory,
    pair_providers: &[PairProvider],
    vault: Option<&BalancerV2Vault>,
    uniswapv3_factory: Option<&IUniswapV3Factory>,
    base_tokens: &BaseTokens,
    settlement_contract: H160,
) -> Result<Arc<dyn TokenOwnerFinding>> {
    let web3 = ethrpc::instrumented::instrument_with_label(&web3, "tokenOwners".into());
    let finders = args
        .token_owner_finders
        .as_deref()
        .unwrap_or_else(|| TokenOwnerFindingStrategy::defaults_for_network(network));
    tracing::debug!(?finders, "initializing token owner finders");

    let mut proposers = Vec::<Arc<dyn TokenOwnerProposing>>::new();

    if finders.contains(&TokenOwnerFindingStrategy::Liquidity) {
        proposers.extend(
            pair_providers
                .iter()
                .map(|provider| -> Arc<dyn TokenOwnerProposing> {
                    Arc::new(UniswapLikePairProviderFinder {
                        inner: *provider,
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
        let mut blockscout =
            BlockscoutTokenOwnerFinder::with_network(http_factory.create(), network)?;
        if let Some(blockscout_config) = &args.blockscout {
            blockscout.with_base_url(blockscout_config.blockscout_api_url.clone());
            blockscout.with_api_key(blockscout_config.blockscout_api_key.clone());
        }
        if let Some(strategy) = args.token_owner_finder_rate_limiter.clone() {
            blockscout.with_rate_limiter(strategy);
        }
        proposers.push(Arc::new(blockscout));
    }

    if finders.contains(&TokenOwnerFindingStrategy::Ethplorer) {
        let mut ethplorer = EthplorerTokenOwnerFinder::try_with_network(
            http_factory.create(),
            args.ethplorer
                .as_ref()
                .map(|ethplorer| ethplorer.ethplorer_api_key.clone()),
            network,
        )?;
        if let Some(ethplorer_config) = &args.ethplorer {
            ethplorer.with_base_url(ethplorer_config.ethplorer_api_url.clone());
        }
        if let Some(strategy) = args.token_owner_finder_rate_limiter.clone() {
            ethplorer.with_rate_limiter(strategy);
        }
        proposers.push(Arc::new(ethplorer));
    }

    if finders.contains(&TokenOwnerFindingStrategy::Solvers) {
        for (url, update_interval) in args
            .solver_token_owners_urls
            .clone()
            .into_iter()
            .zip(args.solver_token_owners_cache_update_intervals.clone())
        {
            let identifier = url.to_string();
            let solver = Box::new(SolverConfiguration {
                url,
                client: http_factory.create(),
            });
            let solver =
                AutoUpdatingSolverTokenOwnerFinder::new(solver, update_interval, identifier);
            proposers.push(Arc::new(solver));
        }
    }

    proposers.push(Arc::new(TokenOwnerList::new(
        args.whitelisted_owners.clone(),
    )));

    Ok(Arc::new(TokenOwnerFinder {
        web3,
        proposers,
        settlement_contract,
    }))
}

/// A `TokenOwnerFinding` implementation that queries a node with proposed owner
/// candidates from an internal list of `TokenOwnerProposing` implementations.
pub struct TokenOwnerFinder {
    pub web3: Web3,
    pub proposers: Vec<Arc<dyn TokenOwnerProposing>>,
    pub settlement_contract: H160,
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

        // We use a stream with ready_chunks so that we can start with the addresses of
        // fast TokenOwnerFinding implementations first without having to wait
        // for slow ones.
        let stream = self.candidate_owners(token).ready_chunks(MAX_BATCH_SIZE);
        futures::pin_mut!(stream);

        while let Some(chunk) = stream.next().await {
            let futures = chunk
                .into_iter()
                // The token balance assertions of the bad token test assume the token
                // owner is not the settlement contract.
                .filter(|owner| *owner != self.settlement_contract)
                .map(|owner| {
                    let call = instance.balance_of(owner).call();
                    async move {
                        match call.await {
                            Ok(balance) => Ok((owner, balance)),
                            Err(err) if EthcontractErrorType::is_contract_err(&err) => {
                                Ok((owner, 0.into()))
                            }
                            Err(err) => Err(err),
                        }
                    }
                });
            let balances = futures::future::try_join_all(futures).await?;

            if let Some(holder) = balances
                .into_iter()
                .find(|(_, balance)| *balance >= min_balance)
            {
                return Ok(Some(holder));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use {super::*, clap::Parser};

    const TOKEN1: H160 = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    const TOKEN2: H160 = addr!("7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9");
    const OWNER1: H160 = addr!("06920c9fc643de77b99cb7670a944ad31eaaa260");
    const OWNER2: H160 = addr!("06601571aa9d3e8f5f7cdd5b993192618964bab5");

    #[test]
    fn parse_owners_empty() {
        assert_eq!(parse_owners("").unwrap(), Default::default());
    }

    #[test]
    fn parse_owners_one_owner() {
        let mut expected = HashMap::new();
        expected.insert(TOKEN1, vec![OWNER1]);
        let parsed = parse_owners(
            "
            0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2:
                0x06920c9fc643de77b99cb7670a944ad31eaaa260
        ",
        )
        .unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_owners_two_owners() {
        let mut expected = HashMap::new();
        expected.insert(TOKEN1, vec![OWNER1, OWNER2]);
        let parsed = parse_owners(
            "
            0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2:
                0x06920c9fc643de77b99cb7670a944ad31eaaa260,
                0x06601571aa9d3e8f5f7cdd5b993192618964bab5
        ",
        )
        .unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_owners_two_tokens_with_one_owners() {
        let mut expected = HashMap::new();
        expected.insert(TOKEN1, vec![OWNER1]);
        expected.insert(TOKEN2, vec![OWNER2]);
        let parsed = parse_owners(
            "
            0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2:
                0x06920c9fc643de77b99cb7670a944ad31eaaa260;
            0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9:
                0x06601571aa9d3e8f5f7cdd5b993192618964bab5
        ",
        )
        .unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_owners_err() {
        assert!(parse_owners("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2:").is_err());
        assert!(parse_owners("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").is_err());
        assert!(parse_owners(":0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").is_err());
    }

    #[test]
    fn blockscout_correctly_configured() {
        let args = vec![
            "test", // Program name
            "--blockscout-api-key",
            "someapikey",
            "--blockscout-api-url",
            "https://swap.cow.fi",
        ];

        let blockscout = Blockscout::try_parse_from(args);

        assert!(blockscout.is_ok());
    }

    #[test]
    fn blockscout_wrongly_configured() {
        let args = vec![
            "test", // Program name
            "--blockscout-api-key",
            "someapikey",
        ];

        let result = Blockscout::try_parse_from(args);

        assert!(result.is_err());
    }

    #[test]
    fn ethplorer_correctly_configured() {
        let args = vec![
            "test", // Program name
            "--ethplorer-api-key",
            "someapikey",
            "--ethplorer-api-url",
            "https://swap.cow.fi",
        ];

        let ethplorer = Ethplorer::try_parse_from(args);

        assert!(ethplorer.is_ok());
    }

    #[test]
    fn ethplorer_wrongly_configured() {
        let args = vec![
            "test", // Program name
            "--ethplorer-api-key",
            "someapikey",
        ];

        let result = Ethplorer::try_parse_from(args);

        assert!(result.is_err());
    }
}
