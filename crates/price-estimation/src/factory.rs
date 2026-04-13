use {
    super::{
        NativePriceEstimator as NativePriceEstimatorSource,
        PriceEstimating,
        competition::CompetitionEstimator,
        external::ExternalPriceEstimator,
        instrumented::InstrumentedPriceEstimator,
        native::{self, NativePriceEstimating, NativePriceEstimator},
        native_price_cache::{self, ApproximationToken},
        sanitized::SanitizedPriceEstimator,
        trade_verifier::{TradeVerifier, TradeVerifying},
    },
    crate::{
        ExternalSolver,
        buffered::{self, BufferedRequest, NativePriceBatchFetching},
        competition::PriceRanking,
        config::{native_price::NativePriceConfig, price_estimation::BalanceOverridesConfigExt},
        trade_verifier::code_fetching::CachedCodeFetcher,
    },
    alloy::primitives::Address,
    anyhow::{Context as _, Result},
    bad_tokens::list_based::DenyListedTokens,
    configs::price_estimation::PriceEstimation,
    contracts::WETH9,
    ethrpc::{Web3, alloy::ProviderLabelingExt, block_stream::CurrentBlockWatcher},
    gas_price_estimation::GasPriceEstimating,
    http_client::HttpClientFactory,
    number::nonzero::NonZeroU256,
    rate_limit::RateLimiter,
    reqwest::Url,
    simulator::{swap_simulator::SwapSimulator, tenderly},
    std::{collections::HashMap, num::NonZeroUsize, sync::Arc},
    token_info::TokenInfoFetching,
};

#[derive(Clone)]
struct EstimatorEntry {
    optimal: Arc<dyn PriceEstimating>,
    fast: Arc<dyn PriceEstimating>,
    native: Arc<dyn PriceEstimating>,
}

/// Network options needed for creating price estimators.
pub struct Network {
    pub web3: Web3,
    pub simulation_web3: Option<Web3>,
    pub chain: chain::Chain,
    pub native_token: Address,
    pub settlement: Address,
    pub authenticator: Address,
    pub block_stream: CurrentBlockWatcher,
}

/// The shared components needed for creating price estimators.
pub struct Components {
    pub http_factory: HttpClientFactory,
    pub deny_listed_tokens: DenyListedTokens,
    pub tokens: Arc<dyn TokenInfoFetching>,
    pub code_fetcher: Arc<CachedCodeFetcher>,
}

/// A factory for initializing shared price estimators.
pub struct PriceEstimatorFactory<'a> {
    args: &'a PriceEstimation,
    config: &'a NativePriceConfig,
    network: Network,
    components: Components,
    trade_verifier: Option<Arc<dyn TradeVerifying>>,
    estimators: HashMap<String, EstimatorEntry>,
}

impl<'a> PriceEstimatorFactory<'a> {
    pub async fn new(
        args: &'a PriceEstimation,
        config: &'a NativePriceConfig,
        network: Network,
        components: Components,
    ) -> Result<Self> {
        Ok(Self {
            trade_verifier: Self::trade_verifier(args, &network, &components).await?,
            args,
            config,
            network,
            components,
            estimators: HashMap::new(),
        })
    }

    async fn trade_verifier(
        args: &'a PriceEstimation,
        network: &Network,
        components: &Components,
    ) -> Result<Option<Arc<dyn TradeVerifying>>> {
        let Some(web3) = network.simulation_web3.clone() else {
            return Ok(None);
        };
        let web3 = web3.labeled("simulator");

        let balance_overrides = args.balance_overrides.init(web3.clone());

        let tenderly = args.tenderly.as_ref().map(|config| {
            Arc::new(tenderly::TenderlyApi::new_instrumented(
                "price_estimation".to_string(),
                config,
                &components.http_factory,
                network.chain.id().to_string(),
            )) as Arc<dyn tenderly::Api>
        });
        let simulator = SwapSimulator::new(
            balance_overrides.clone(),
            network.settlement,
            network.native_token,
            network.block_stream.clone(),
            web3.clone(),
            args.max_gas_per_tx,
        )
        .await?;

        let verifier = TradeVerifier::new(
            web3,
            tenderly,
            simulator,
            components.code_fetcher.clone(),
            balance_overrides,
            network.settlement,
            args.quote_inaccuracy_limit.clone(),
            args.tokens_without_verification.iter().cloned().collect(),
            args.min_gas_amount_for_unverified_quotes,
            args.max_gas_amount_for_unverified_quotes,
        )
        .await?;
        Ok(Some(Arc::new(verifier)))
    }

    fn native_token_price_estimation_amount(&self) -> Result<NonZeroU256> {
        NonZeroU256::try_from(self.args.amount_to_estimate_prices_with.unwrap_or_else(|| {
            self.network
                .chain
                .default_amount_to_estimate_native_prices_with()
        }))
    }

    fn rate_limiter(&self, name: &str) -> Arc<RateLimiter> {
        Arc::new(RateLimiter::from_strategy(
            self.args
                .price_estimation_rate_limiter
                .clone()
                .unwrap_or_default(),
            format!("{name}_estimator"),
        ))
    }

    fn create_estimator_entry<T>(&self, name: &str, params: T::Params) -> Result<EstimatorEntry>
    where
        T: PriceEstimating + PriceEstimatorCreating,
        T::Params: Clone,
    {
        let estimator = T::init(self, name, params.clone())?;
        let verified = self
            .trade_verifier
            .as_ref()
            .and_then(|trade_verifier| estimator.verified(trade_verifier));

        let fast = instrument(estimator, name);
        let optimal = match verified {
            Some(verified) => instrument(verified, name),
            None => fast.clone(),
        };

        // Eagerly create the native price estimator, even if we don't use it.
        // It just simplifies price estimator creation code and only costs a few
        // extra cycles during initialization. Also note that we intentionally
        // don't share price estimators between optimal/fast and the native
        // price estimator (this is because request sharing isn't benificial),
        // nor do we configure the trade verifier (because external price
        // precision is less critical).
        let native = instrument(T::init(self, name, params)?, name);

        Ok(EstimatorEntry {
            optimal,
            fast,
            native,
        })
    }

    async fn create_native_estimator(
        &mut self,
        source: &NativePriceEstimatorSource,
        weth: &WETH9::Instance,
    ) -> Result<(String, Arc<dyn NativePriceEstimating>)> {
        match source {
            NativePriceEstimatorSource::Forwarder { url } => {
                let name = format!("Forwarder|{}", url);
                Ok((
                    name.clone(),
                    Arc::new(InstrumentedPriceEstimator::new(
                        native::Forwarder::new(self.components.http_factory.create(), url.clone()),
                        name,
                    )),
                ))
            }
            NativePriceEstimatorSource::Driver(driver) => {
                let native_token_price_estimation_amount =
                    self.native_token_price_estimation_amount()?;
                let estimator = self.get_estimator(driver)?.native.clone();
                Ok((
                    driver.name.clone(),
                    Arc::new(InstrumentedPriceEstimator::new(
                        NativePriceEstimator::new(
                            Arc::new(self.sanitized_native_price(estimator)),
                            self.network.native_token,
                            native_token_price_estimation_amount,
                        ),
                        driver.name.to_string(),
                    )),
                ))
            }
            NativePriceEstimatorSource::OneInchSpotPriceApi => {
                let one_inch = self
                    .args
                    .one_inch
                    .as_ref()
                    .context("one-inch config must be set when OneInchSpotPriceApi is used")?;
                let name = "OneInchSpotPriceApi".to_string();
                Ok((
                    name.clone(),
                    Arc::new(InstrumentedPriceEstimator::new(
                        native::OneInch::new(
                            self.components.http_factory.create(),
                            one_inch.url.clone(),
                            Some(one_inch.api_key.clone()),
                            self.network.chain.id(),
                            self.network.block_stream.clone(),
                            self.components.tokens.clone(),
                        ),
                        name,
                    )),
                ))
            }
            NativePriceEstimatorSource::CoinGecko => {
                let coin_gecko_config = self
                    .args
                    .coin_gecko
                    .as_ref()
                    .context("coin-gecko config must be set when CoinGecko is used")?;

                let name = "CoinGecko".to_string();
                let coin_gecko = native::CoinGecko::new(
                    self.components.http_factory.create(),
                    coin_gecko_config.url.clone(),
                    Some(coin_gecko_config.api_key.clone()),
                    &self.network.chain,
                    *weth.address(),
                    self.components.tokens.clone(),
                )
                .await?;

                let coin_gecko: Arc<dyn NativePriceEstimating> =
                    if let Some(buffered_config) = &coin_gecko_config.buffered {
                        let configuration = buffered::Configuration {
                            max_concurrent_requests: Some(
                                coin_gecko
                                    .max_batch_size()
                                    .try_into()
                                    .context("invalid CoinGecko max batch size")?,
                            ),
                            debouncing_time: buffered_config.debouncing_time,
                            result_ready_timeout: self.args.quote_timeout,
                            broadcast_channel_capacity: buffered_config.broadcast_channel_capacity,
                        };

                        Arc::new(InstrumentedPriceEstimator::new(
                            BufferedRequest::with_config(coin_gecko, configuration),
                            name.clone() + "Buffered",
                        ))
                    } else {
                        Arc::new(InstrumentedPriceEstimator::new(coin_gecko, name.clone()))
                    };

                Ok((name, coin_gecko))
            }
        }
    }

    fn get_estimator(&mut self, solver: &ExternalSolver) -> Result<&EstimatorEntry> {
        let params = ExternalEstimatorParams {
            driver: solver.url.clone(),
        };
        if !self.estimators.contains_key(&solver.name) {
            let estimator =
                self.create_estimator_entry::<ExternalPriceEstimator>(&solver.name, params)?;
            self.estimators.insert(solver.name.clone(), estimator);
        }

        Ok(&self.estimators[&solver.name])
    }

    fn get_estimators(
        &mut self,
        solvers: &[ExternalSolver],
        select: impl Fn(&EstimatorEntry) -> &Arc<dyn PriceEstimating>,
    ) -> Result<Vec<(String, Arc<dyn PriceEstimating>)>> {
        solvers
            .iter()
            .map(|solver| {
                Ok((
                    solver.name.clone(),
                    select(self.get_estimator(solver)?).clone(),
                ))
            })
            .collect()
    }

    fn sanitized(&self, estimator: Arc<dyn PriceEstimating>) -> SanitizedPriceEstimator {
        SanitizedPriceEstimator::new(
            estimator,
            self.network.native_token,
            self.components.deny_listed_tokens.clone(),
            false, // not estimating native price
        )
    }

    /// Creates a SanitizedPriceEstimator that is used for native price
    /// estimations
    fn sanitized_native_price(
        &self,
        estimator: Arc<dyn PriceEstimating>,
    ) -> SanitizedPriceEstimator {
        SanitizedPriceEstimator::new(
            estimator,
            self.network.native_token,
            self.components.deny_listed_tokens.clone(),
            true, // estimating native price
        )
    }

    pub fn price_estimator(
        &mut self,
        solvers: &[ExternalSolver],
        native: Arc<dyn NativePriceEstimating>,
        gas: Arc<dyn GasPriceEstimating>,
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(solvers, |entry| &entry.optimal)?;
        let competition_estimator = CompetitionEstimator::new(
            vec![estimators],
            PriceRanking::BestBangForBuck { native, gas },
        )
        .with_verification(self.args.quote_verification);
        Ok(Arc::new(self.sanitized(Arc::new(competition_estimator))))
    }

    pub fn fast_price_estimator(
        &mut self,
        solvers: &[ExternalSolver],
        fast_price_estimation_results_required: NonZeroUsize,
        native: Arc<dyn NativePriceEstimating>,
        gas: Arc<dyn GasPriceEstimating>,
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(solvers, |entry| &entry.fast)?;
        Ok(Arc::new(
            self.sanitized(Arc::new(
                CompetitionEstimator::new(
                    vec![estimators],
                    PriceRanking::BestBangForBuck { native, gas },
                )
                .with_early_return(fast_price_estimation_results_required),
            )),
        ))
    }

    /// Creates a native price estimator from the given sources.
    pub async fn native_price_estimator(
        &mut self,
        native: &[Vec<NativePriceEstimatorSource>],
        results_required: NonZeroUsize,
        weth: &WETH9::Instance,
    ) -> Result<Box<dyn NativePriceEstimating>> {
        let mut estimators = Vec::with_capacity(native.len());
        for stage in native.iter() {
            let mut stages = Vec::with_capacity(stage.len());
            for source in stage {
                stages.push(self.create_native_estimator(source, weth).await?);
            }
            estimators.push(stages);
        }

        let competition_estimator =
            CompetitionEstimator::new(estimators, PriceRanking::MaxOutAmount)
                .with_verification(self.args.quote_verification)
                .with_early_return(results_required);
        Ok(Box::new(competition_estimator))
    }

    /// Creates a [`CachingNativePriceEstimator`] that wraps a native price
    /// estimator with an in-memory cache.
    pub async fn caching_native_price_estimator(
        &mut self,
        native: &[Vec<NativePriceEstimatorSource>],
        results_required: NonZeroUsize,
        weth: &WETH9::Instance,
        cache: native_price_cache::Cache,
    ) -> native_price_cache::CachingNativePriceEstimator {
        let inner = self
            .native_price_estimator(native, results_required, weth)
            .await
            .expect("failed to build native price estimator");
        self.caching_native_price_estimator_from_inner(inner, cache)
            .await
    }

    /// Creates a [`CachingNativePriceEstimator`] from a pre-built inner
    /// estimator.
    pub async fn caching_native_price_estimator_from_inner(
        &mut self,
        inner: Box<dyn NativePriceEstimating>,
        cache: native_price_cache::Cache,
    ) -> native_price_cache::CachingNativePriceEstimator {
        let approximation_tokens = self
            .build_approximation_tokens()
            .await
            .expect("failed to build native price approximation tokens");
        native_price_cache::CachingNativePriceEstimator::new(
            inner,
            cache,
            self.config.cache.concurrent_requests.get(),
            approximation_tokens,
            self.args.quote_timeout,
        )
    }

    /// Builds the approximation tokens mapping with normalization factors based
    /// on decimal differences between token pairs.
    async fn build_approximation_tokens(&self) -> Result<HashMap<Address, ApproximationToken>> {
        let pairs = &self.config.approximation_tokens;
        if pairs.is_empty() {
            return Ok(HashMap::new());
        }

        // Collect all unique addresses to fetch their decimals
        let all_addresses: Vec<Address> = pairs
            .iter()
            .flat_map(|(from, to)| [*from, *to])
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let token_infos = self.components.tokens.get_token_infos(&all_addresses).await;

        let mut approximation_tokens = HashMap::new();
        for (from_token, to_token) in pairs {
            let from_decimals = token_infos
                .get(from_token)
                .and_then(|info| info.decimals)
                .with_context(|| {
                    format!(
                        "could not fetch decimals for approximation source token {from_token:?}"
                    )
                })?;

            let to_decimals = token_infos
                .get(to_token)
                .and_then(|info| info.decimals)
                .with_context(|| {
                    format!("could not fetch decimals for approximation target token {to_token:?}")
                })?;

            approximation_tokens.insert(
                *from_token,
                ApproximationToken::with_normalization((*to_token, to_decimals), from_decimals),
            );
        }

        Ok(approximation_tokens)
    }
}

/// Trait for modelling the initialization of a Price estimator and its verified
/// counter-part. This allows for generic price estimator creation, as well as
/// per-type trade verification configuration.
trait PriceEstimatorCreating: Sized {
    type Params;

    fn init(factory: &PriceEstimatorFactory, name: &str, params: Self::Params) -> Result<Self>;

    fn verified(&self, _: &Arc<dyn TradeVerifying>) -> Option<Self> {
        None
    }
}

#[derive(Debug, Clone)]
struct ExternalEstimatorParams {
    driver: Url,
}

impl PriceEstimatorCreating for ExternalPriceEstimator {
    type Params = ExternalEstimatorParams;

    fn init(factory: &PriceEstimatorFactory, name: &str, params: Self::Params) -> Result<Self> {
        Ok(Self::new(
            params.driver,
            factory.components.http_factory.create(),
            factory.rate_limiter(name),
            factory.network.block_stream.clone(),
        ))
    }

    fn verified(&self, verifier: &Arc<dyn TradeVerifying>) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}

fn instrument<T: PriceEstimating>(
    estimator: T,
    name: impl Into<String>,
) -> Arc<InstrumentedPriceEstimator<T>> {
    Arc::new(InstrumentedPriceEstimator::new(estimator, name.into()))
}
