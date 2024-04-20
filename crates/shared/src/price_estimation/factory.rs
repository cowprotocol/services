use {
    super::{
        balancer_sor::BalancerSor,
        baseline::BaselinePriceEstimator,
        competition::CompetitionEstimator,
        external::ExternalPriceEstimator,
        http::{HttpPriceEstimator, HttpTradeFinder},
        instrumented::InstrumentedPriceEstimator,
        native::{self, NativePriceEstimator},
        native_price_cache::CachingNativePriceEstimator,
        oneinch::OneInchPriceEstimator,
        paraswap::ParaswapPriceEstimator,
        sanitized::SanitizedPriceEstimator,
        trade_verifier::{TradeVerifier, TradeVerifying},
        zeroex::ZeroExPriceEstimator,
        Arguments,
        NativePriceEstimator as NativePriceEstimatorSource,
        PriceEstimating,
        PriceEstimator,
        PriceEstimatorKind,
    },
    crate::{
        arguments::{self, CodeSimulatorKind, ExternalSolver, LegacySolver},
        bad_token::BadTokenDetecting,
        balancer_sor_api::DefaultBalancerSorApi,
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        code_simulation::{self, CodeSimulating, TenderlyCodeSimulator},
        ethrpc::Web3,
        http_client::HttpClientFactory,
        http_solver::{DefaultHttpSolverApi, Objective, SolverConfig},
        oneinch_api::OneInchClientImpl,
        paraswap_api::DefaultParaswapApi,
        price_estimation::{competition::PriceRanking, native::NativePriceEstimating},
        sources::{
            balancer_v2::BalancerPoolFetching,
            uniswap_v2::pool_fetching::PoolFetching as UniswapV2PoolFetching,
            uniswap_v3::pool_fetching::PoolFetching as UniswapV3PoolFetching,
        },
        token_info::TokenInfoFetching,
        trade_finding,
        zeroex_api::DefaultZeroExApi,
    },
    anyhow::{anyhow, Context as _, Result},
    ethcontract::H160,
    ethrpc::current_block::CurrentBlockStream,
    gas_estimation::GasPriceEstimating,
    number::nonzero::U256 as NonZeroU256,
    rate_limit::RateLimiter,
    reqwest::Url,
    std::{collections::HashMap, num::NonZeroUsize, sync::Arc},
};

/// A factory for initializing shared price estimators.
pub struct PriceEstimatorFactory<'a> {
    args: &'a Arguments,
    shared_args: &'a arguments::Arguments,
    network: Network,
    components: Components,
    trade_verifier: Option<Arc<dyn TradeVerifying>>,
    estimators: HashMap<String, EstimatorEntry>,
}

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
    pub name: String,
    pub chain_id: u64,
    pub native_token: H160,
    pub settlement: H160,
    pub authenticator: H160,
    pub base_tokens: Arc<BaseTokens>,
    pub block_stream: CurrentBlockStream,
}

/// The shared components needed for creating price estimators.
pub struct Components {
    pub http_factory: HttpClientFactory,
    pub bad_token_detector: Arc<dyn BadTokenDetecting>,
    pub uniswap_v2_pools: Arc<dyn UniswapV2PoolFetching>,
    pub balancer_pools: Option<Arc<dyn BalancerPoolFetching>>,
    pub uniswap_v3_pools: Option<Arc<dyn UniswapV3PoolFetching>>,
    pub tokens: Arc<dyn TokenInfoFetching>,
    pub gas_price: Arc<dyn GasPriceEstimating>,
}

/// The source of the price estimator.
pub enum PriceEstimatorSource {
    Builtin(PriceEstimator),
    External(ExternalSolver),
    Legacy(LegacySolver),
}

impl PriceEstimatorSource {
    pub fn for_args(
        builtin: &[PriceEstimator],
        external: &[ExternalSolver],
        legacy: &[LegacySolver],
    ) -> Vec<Self> {
        std::iter::empty()
            .chain(builtin.iter().copied().map(PriceEstimatorSource::Builtin))
            .chain(external.iter().cloned().map(PriceEstimatorSource::External))
            .chain(legacy.iter().cloned().map(PriceEstimatorSource::Legacy))
            .collect()
    }
}

impl<'a> PriceEstimatorFactory<'a> {
    pub fn new(
        args: &'a Arguments,
        shared_args: &'a arguments::Arguments,
        network: Network,
        components: Components,
    ) -> Result<Self> {
        let trade_verifier = args
            .trade_simulator
            .map(|kind| -> Result<Arc<dyn TradeVerifying>> {
                let web3_simulator = || {
                    network
                        .simulation_web3
                        .clone()
                        .map(|web3| {
                            ethrpc::instrumented::instrument_with_label(&web3, "simulator".into())
                        })
                        .context("missing simulation node configuration")
                };
                let tenderly_simulator = || -> anyhow::Result<_> {
                    let tenderly_api = shared_args
                        .tenderly
                        .get_api_instance(&components.http_factory, "price_estimation".to_owned())?
                        .context("missing Tenderly configuration")?;
                    let simulator = TenderlyCodeSimulator::new(tenderly_api, network.chain_id);
                    Ok(simulator)
                };

                let simulator = match kind {
                    CodeSimulatorKind::Web3 => {
                        Arc::new(web3_simulator()?) as Arc<dyn CodeSimulating>
                    }
                    CodeSimulatorKind::Tenderly => Arc::new(tenderly_simulator()?.save(
                        args.tenderly_save_successful_trade_simulations,
                        args.tenderly_save_failed_trade_simulations,
                    )),
                    CodeSimulatorKind::Web3ThenTenderly => {
                        Arc::new(code_simulation::Web3ThenTenderly::new(
                            web3_simulator()?,
                            tenderly_simulator()?,
                        ))
                    }
                };
                let code_fetcher = ethrpc::instrumented::instrument_with_label(
                    &network.web3,
                    "codeFetching".into(),
                );
                let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(code_fetcher)));

                Ok(Arc::new(TradeVerifier::new(
                    simulator,
                    code_fetcher,
                    network.block_stream.clone(),
                    network.settlement,
                    network.native_token,
                    args.quote_inaccuracy_limit,
                )))
            })
            .transpose()?;

        Ok(Self {
            args,
            shared_args,
            network,
            components,
            trade_verifier,
            estimators: HashMap::new(),
        })
    }

    fn native_token_price_estimation_amount(&self) -> Result<NonZeroU256> {
        NonZeroU256::try_from(
            self.args
                .amount_to_estimate_prices_with
                .or_else(|| {
                    native::default_amount_to_estimate_native_prices_with(self.network.chain_id)
                })
                .context("No amount to estimate prices with set.")?,
        )
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
            Some(verified) => instrument(verified, format!("{name}_verified")),
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

    fn create_estimator(&self, estimator: PriceEstimator) -> Result<EstimatorEntry> {
        let name = estimator.name();
        match estimator.kind {
            PriceEstimatorKind::Baseline => {
                self.create_estimator_entry::<BaselinePriceEstimator>(&name, estimator.address)
            }
            PriceEstimatorKind::Paraswap => {
                self.create_estimator_entry::<ParaswapPriceEstimator>(&name, estimator.address)
            }
            PriceEstimatorKind::ZeroEx => {
                self.create_estimator_entry::<ZeroExPriceEstimator>(&name, estimator.address)
            }
            PriceEstimatorKind::OneInch => {
                self.create_estimator_entry::<OneInchPriceEstimator>(&name, estimator.address)
            }
            PriceEstimatorKind::BalancerSor => {
                self.create_estimator_entry::<BalancerSor>(&name, estimator.address)
            }
        }
    }

    fn create_native_estimator(
        &mut self,
        source: NativePriceEstimatorSource,
        external: &[PriceEstimatorSource],
    ) -> Result<(String, Arc<dyn NativePriceEstimating>)> {
        match source {
            NativePriceEstimatorSource::GenericPriceEstimator(estimator) => {
                let native_token_price_estimation_amount =
                    self.native_token_price_estimation_amount()?;
                self.get_estimators(external, |entry| &entry.native)?
                    .into_iter()
                    .map(
                        |(name, estimator)| -> (String, Arc<dyn NativePriceEstimating>) {
                            (
                                name,
                                Arc::new(NativePriceEstimator::new(
                                    Arc::new(self.sanitized(estimator)),
                                    self.network.native_token,
                                    native_token_price_estimation_amount,
                                )),
                            )
                        },
                    )
                    .find(|external| external.0 == estimator)
                    .ok_or(anyhow!(
                        "Couldn't find generic price estimator with name {} to instantiate native \
                         estimator",
                        estimator
                    ))
            }
            NativePriceEstimatorSource::OneInchSpotPriceApi => Ok((
                "OneInchSpotPriceApi".into(),
                Arc::new(native::OneInch::new(
                    self.components.http_factory.create(),
                    self.args.one_inch_url.clone(),
                    self.args.one_inch_api_key.clone(),
                    self.network.chain_id,
                    self.network.block_stream.clone(),
                    self.components.tokens.clone(),
                )),
            )),
        }
    }

    fn get_estimator(&mut self, source: &PriceEstimatorSource) -> Result<&EstimatorEntry> {
        let name = source.name();

        if !self.estimators.contains_key(&name) {
            let estimator = match source {
                PriceEstimatorSource::Builtin(builtin) => self.create_estimator(*builtin)?,
                PriceEstimatorSource::External(driver) => self
                    .create_estimator_entry::<ExternalPriceEstimator>(
                        &driver.name,
                        driver.into(),
                    )?,
                PriceEstimatorSource::Legacy(solver) => {
                    self.create_estimator_entry::<HttpPriceEstimator>(&solver.name, solver.into())?
                }
            };
            self.estimators.insert(name.clone(), estimator);
        }

        Ok(&self.estimators[&name])
    }

    fn get_estimators(
        &mut self,
        sources: &[PriceEstimatorSource],
        select: impl Fn(&EstimatorEntry) -> &Arc<dyn PriceEstimating>,
    ) -> Result<Vec<(String, Arc<dyn PriceEstimating>)>> {
        sources
            .iter()
            .map(|source| Ok((source.name(), select(self.get_estimator(source)?).clone())))
            .collect()
    }

    fn sanitized(&self, estimator: Arc<dyn PriceEstimating>) -> SanitizedPriceEstimator {
        SanitizedPriceEstimator::new(
            estimator,
            self.network.native_token,
            self.components.bad_token_detector.clone(),
        )
    }

    pub fn price_estimator(
        &mut self,
        sources: &[PriceEstimatorSource],
        native: Arc<dyn NativePriceEstimating>,
        gas: Arc<dyn GasPriceEstimating>,
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(sources, |entry| &entry.optimal)?;
        let competition_estimator = CompetitionEstimator::new(
            vec![estimators],
            PriceRanking::BestBangForBuck { native, gas },
        )
        .prefer_verified_estimates(self.args.prefer_verified_quotes);
        Ok(Arc::new(self.sanitized(Arc::new(competition_estimator))))
    }

    pub fn fast_price_estimator(
        &mut self,
        sources: &[PriceEstimatorSource],
        fast_price_estimation_results_required: NonZeroUsize,
        native: Arc<dyn NativePriceEstimating>,
        gas: Arc<dyn GasPriceEstimating>,
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(sources, |entry| &entry.fast)?;
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

    pub fn native_price_estimator(
        &mut self,
        native: &[Vec<NativePriceEstimatorSource>],
        external: &[PriceEstimatorSource],
        results_required: NonZeroUsize,
    ) -> Result<Arc<CachingNativePriceEstimator>> {
        anyhow::ensure!(
            self.args.native_price_cache_max_age > self.args.native_price_prefetch_time,
            "price cache prefetch time needs to be less than price cache max age"
        );

        let estimators = native
            .iter()
            .map(|stage| {
                stage
                    .iter()
                    .map(|source| self.create_native_estimator(source.clone(), external))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<Vec<_>>>>()?;

        let competition_estimator =
            CompetitionEstimator::new(estimators, PriceRanking::MaxOutAmount)
                .prefer_verified_estimates(self.args.prefer_verified_quotes)
                .with_early_return(results_required);
        let native_estimator = Arc::new(CachingNativePriceEstimator::new(
            Box::new(competition_estimator),
            self.args.native_price_cache_max_age,
            self.args.native_price_cache_refresh,
            Some(self.args.native_price_cache_max_update_size),
            self.args.native_price_prefetch_time,
            self.args.native_price_cache_concurrent_requests,
        ));
        Ok(native_estimator)
    }
}

impl PriceEstimatorSource {
    fn name(&self) -> String {
        match self {
            Self::Builtin(builtin) => builtin.name(),
            Self::External(solver) => solver.name.clone(),
            Self::Legacy(solver) => solver.name.clone(),
        }
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

impl PriceEstimatorCreating for BaselinePriceEstimator {
    type Params = H160;

    fn init(factory: &PriceEstimatorFactory, _name: &str, solver: Self::Params) -> Result<Self> {
        Ok(BaselinePriceEstimator::new(
            factory.components.uniswap_v2_pools.clone(),
            factory.components.gas_price.clone(),
            factory.network.base_tokens.clone(),
            factory.network.native_token,
            factory.native_token_price_estimation_amount()?,
            solver,
        ))
    }
}
impl PriceEstimatorCreating for ParaswapPriceEstimator {
    type Params = H160;

    fn init(factory: &PriceEstimatorFactory, name: &str, solver: Self::Params) -> Result<Self> {
        Ok(ParaswapPriceEstimator::new(
            Arc::new(DefaultParaswapApi {
                client: factory.components.http_factory.configure(|builder| {
                    builder.timeout(
                        trade_finding::time_limit()
                            .to_std()
                            .expect("negative time limit"),
                    )
                }),
                base_url: factory.shared_args.paraswap_api_url.clone(),
                partner: factory
                    .shared_args
                    .paraswap_partner
                    .clone()
                    .unwrap_or_default(),
                block_stream: factory.network.block_stream.clone(),
            }),
            factory.components.tokens.clone(),
            factory.shared_args.disabled_paraswap_dexs.clone(),
            factory.rate_limiter(name),
            solver,
        ))
    }

    fn verified(&self, verifier: &Arc<dyn TradeVerifying>) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}
impl PriceEstimatorCreating for ZeroExPriceEstimator {
    type Params = H160;

    fn init(factory: &PriceEstimatorFactory, name: &str, solver: Self::Params) -> Result<Self> {
        let client_builder = factory.components.http_factory.builder().timeout(
            trade_finding::time_limit()
                .to_std()
                .expect("negative time limit"),
        );
        let api = DefaultZeroExApi::new(
            client_builder,
            factory
                .shared_args
                .zeroex_url
                .as_deref()
                .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
            factory.shared_args.zeroex_api_key.clone(),
            factory.network.block_stream.clone(),
        )
        .unwrap();
        Ok(ZeroExPriceEstimator::new(
            Arc::new(api),
            factory.shared_args.disabled_zeroex_sources.clone(),
            factory.rate_limiter(name),
            factory.args.zeroex_only_estimate_buy_queries,
            solver,
        ))
    }

    fn verified(&self, verifier: &Arc<dyn TradeVerifying>) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}

impl PriceEstimatorCreating for OneInchPriceEstimator {
    type Params = H160;

    fn init(factory: &PriceEstimatorFactory, name: &str, solver: Self::Params) -> Result<Self> {
        let api = OneInchClientImpl::new(
            factory.args.one_inch_url.clone(),
            factory.components.http_factory.configure(|builder| {
                builder.timeout(
                    trade_finding::time_limit()
                        .to_std()
                        .expect("negative time limit"),
                )
            }),
            factory.network.chain_id,
            factory.network.block_stream.clone(),
        )
        .context("1Inch API not supported for network")?;
        Ok(OneInchPriceEstimator::new(
            Arc::new(api),
            factory.shared_args.disabled_one_inch_protocols.clone(),
            factory.rate_limiter(name),
            factory.shared_args.one_inch_referrer_address,
            solver,
            factory.network.settlement,
        ))
    }

    fn verified(&self, verifier: &Arc<dyn TradeVerifying>) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}

impl PriceEstimatorCreating for BalancerSor {
    type Params = H160;

    fn init(factory: &PriceEstimatorFactory, name: &str, solver: Self::Params) -> Result<Self> {
        Ok(BalancerSor::new(
            Arc::new(DefaultBalancerSorApi::new(
                factory.components.http_factory.configure(|builder| {
                    builder.timeout(
                        trade_finding::time_limit()
                            .to_std()
                            .expect("negative time limit"),
                    )
                }),
                factory
                    .args
                    .balancer_sor_url
                    .clone()
                    .context("balancer SOR url not specified")?,
                factory.network.chain_id,
            )?),
            factory.rate_limiter(name),
            factory.components.gas_price.clone(),
            solver,
        ))
    }
}

#[derive(Debug, Clone)]
struct HttpPriceEstimatorParams {
    base: Url,
    solve_path: String,
    use_liquidity: bool,
    solver: H160,
}

impl PriceEstimatorCreating for HttpPriceEstimator {
    type Params = HttpPriceEstimatorParams;

    fn init(factory: &PriceEstimatorFactory, name: &str, params: Self::Params) -> Result<Self> {
        Ok(HttpPriceEstimator::new(
            name.to_string(),
            HttpTradeFinder::new(
                Arc::new(DefaultHttpSolverApi {
                    name: name.to_string(),
                    network_name: factory.network.name.clone(),
                    chain_id: factory.network.chain_id,
                    base: params.base,
                    solve_path: params.solve_path,
                    client: factory.components.http_factory.create(),
                    gzip_requests: false,
                    config: SolverConfig {
                        use_internal_buffers: Some(factory.shared_args.use_internal_buffers),
                        objective: Some(Objective::SurplusFeesCosts),
                        ..Default::default()
                    },
                }),
                factory.components.uniswap_v2_pools.clone(),
                factory.components.balancer_pools.clone(),
                factory.components.uniswap_v3_pools.clone(),
                factory.components.tokens.clone(),
                factory.components.gas_price.clone(),
                factory.network.native_token,
                factory.network.base_tokens.clone(),
                factory.network.name.clone(),
                factory.rate_limiter(name),
                params.use_liquidity,
                params.solver,
            ),
            factory.rate_limiter(name),
        ))
    }

    fn verified(&self, verifier: &Arc<dyn TradeVerifying>) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}

impl From<&LegacySolver> for HttpPriceEstimatorParams {
    fn from(solver: &LegacySolver) -> Self {
        let (base, solve_path) = crate::url::split_at_path(&solver.url).unwrap();
        Self {
            base,
            solve_path,
            use_liquidity: solver.use_liquidity,
            solver: solver.address,
        }
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

impl From<&ExternalSolver> for ExternalEstimatorParams {
    fn from(solver: &ExternalSolver) -> Self {
        Self {
            driver: solver.url.clone(),
        }
    }
}

fn instrument(
    estimator: impl PriceEstimating,
    name: impl Into<String>,
) -> Arc<InstrumentedPriceEstimator> {
    Arc::new(InstrumentedPriceEstimator::new(
        Box::new(estimator),
        name.into(),
    ))
}
