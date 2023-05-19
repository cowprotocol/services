use {
    super::{
        balancer_sor::BalancerSor,
        baseline::BaselinePriceEstimator,
        competition::{CompetitionPriceEstimator, RacingCompetitionPriceEstimator},
        external::ExternalPriceEstimator,
        http::HttpPriceEstimator,
        instrumented::InstrumentedPriceEstimator,
        native::{self, NativePriceEstimator},
        native_price_cache::CachingNativePriceEstimator,
        oneinch::OneInchPriceEstimator,
        paraswap::ParaswapPriceEstimator,
        sanitized::SanitizedPriceEstimator,
        trade_finder::TradeVerifier,
        zeroex::ZeroExPriceEstimator,
        Arguments,
        PriceEstimating,
        PriceEstimatorType,
        TradeValidatorKind,
    },
    crate::{
        arguments::{self, Driver},
        bad_token::BadTokenDetecting,
        balancer_sor_api::DefaultBalancerSorApi,
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        code_simulation::{self, CodeSimulating, TenderlyCodeSimulator},
        ethrpc::Web3,
        http_client::HttpClientFactory,
        http_solver::{DefaultHttpSolverApi, Objective, SolverConfig},
        oneinch_api::OneInchClient,
        paraswap_api::DefaultParaswapApi,
        rate_limiter::RateLimiter,
        sources::{
            balancer_v2::BalancerPoolFetching,
            uniswap_v2::pool_fetching::PoolFetching as UniswapV2PoolFetching,
            uniswap_v3::pool_fetching::PoolFetching as UniswapV3PoolFetching,
        },
        token_info::TokenInfoFetching,
        zeroex_api::ZeroExApi,
    },
    anyhow::{Context as _, Result},
    ethcontract::{H160, U256},
    gas_estimation::GasPriceEstimating,
    reqwest::Url,
    std::{collections::HashMap, num::NonZeroUsize, sync::Arc},
};

/// A factory for initializing shared price estimators.
pub struct PriceEstimatorFactory<'a> {
    args: &'a Arguments,
    shared_args: &'a arguments::Arguments,
    network: Network,
    components: Components,
    trade_verifier: Option<TradeVerifier>,
    estimators: HashMap<PriceEstimatorType, EstimatorEntry>,
    external_estimators: HashMap<String, EstimatorEntry>,
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
    pub zeroex: Arc<dyn ZeroExApi>,
    pub oneinch: Option<Arc<dyn OneInchClient>>,
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
            .map(|kind| -> Result<TradeVerifier> {
                let web3_simulator = || {
                    network
                        .simulation_web3
                        .clone()
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
                    TradeValidatorKind::Web3 => {
                        Arc::new(web3_simulator()?) as Arc<dyn CodeSimulating>
                    }
                    TradeValidatorKind::Tenderly => Arc::new(
                        tenderly_simulator()?
                            .save(false, args.tenderly_save_failed_trade_simulations),
                    ),
                    TradeValidatorKind::Web3ThenTenderly => {
                        Arc::new(code_simulation::Web3ThenTenderly::new(
                            web3_simulator()?,
                            tenderly_simulator()?,
                        ))
                    }
                };
                let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(network.web3.clone())));

                Ok(TradeVerifier::new(
                    simulator,
                    code_fetcher,
                    network.authenticator,
                ))
            })
            .transpose()?;

        Ok(Self {
            args,
            shared_args,
            network,
            components,
            trade_verifier,
            estimators: HashMap::new(),
            external_estimators: Default::default(),
        })
    }

    fn native_token_price_estimation_amount(&self) -> Result<U256> {
        self.args
            .amount_to_estimate_prices_with
            .or_else(|| {
                native::default_amount_to_estimate_native_prices_with(self.network.chain_id)
            })
            .context("No amount to estimate prices with set.")
    }

    fn rate_limiter(&self, name: &str) -> Arc<RateLimiter> {
        Arc::new(RateLimiter::from_strategy(
            self.args
                .price_estimation_rate_limiter
                .clone()
                .unwrap_or_default(),
            format!("{name:?}_estimator"),
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
            Some(verified) => instrument(verified, format!("{name:?}_verified")),
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

    fn create_estimator(&self, kind: PriceEstimatorType) -> Result<EstimatorEntry> {
        let name = kind.name();
        match kind {
            PriceEstimatorType::Baseline => {
                self.create_estimator_entry::<BaselinePriceEstimator>(&name, ())
            }
            PriceEstimatorType::Paraswap => {
                self.create_estimator_entry::<ParaswapPriceEstimator>(&name, ())
            }
            PriceEstimatorType::ZeroEx => {
                self.create_estimator_entry::<ZeroExPriceEstimator>(&name, ())
            }
            PriceEstimatorType::Quasimodo => self.create_estimator_entry::<HttpPriceEstimator>(
                &name,
                HttpPriceEstimatorParams {
                    base: self
                        .args
                        .quasimodo_solver_url
                        .clone()
                        .context("quasimodo solver url not specified")?,
                    solve_path: "solve".to_owned(),
                    use_liquidity: true,
                },
            ),
            PriceEstimatorType::OneInch => {
                self.create_estimator_entry::<OneInchPriceEstimator>(&name, ())
            }
            PriceEstimatorType::Yearn => self.create_estimator_entry::<HttpPriceEstimator>(
                &name,
                HttpPriceEstimatorParams {
                    base: self
                        .args
                        .yearn_solver_url
                        .clone()
                        .context("yearn solver url not specified")?,
                    solve_path: self.args.yearn_solver_path.clone(),
                    use_liquidity: false,
                },
            ),
            PriceEstimatorType::BalancerSor => {
                self.create_estimator_entry::<BalancerSor>(&name, ())
            }
        }
    }

    fn get_estimator(&mut self, kind: PriceEstimatorType) -> Result<&EstimatorEntry> {
        #[allow(clippy::map_entry)]
        if !self.estimators.contains_key(&kind) {
            self.estimators.insert(kind, self.create_estimator(kind)?);
        }
        Ok(&self.estimators[&kind])
    }

    fn get_external_estimator(&mut self, driver: &Driver) -> Result<&EstimatorEntry> {
        #[allow(clippy::map_entry)]
        if !self.external_estimators.contains_key(&driver.name) {
            self.external_estimators.insert(
                driver.name.clone(),
                self.create_estimator_entry::<ExternalPriceEstimator>(
                    &driver.name,
                    ExternalEstimatorParams {
                        driver: driver.url.clone(),
                    },
                )?,
            );
        }
        Ok(&self.external_estimators[&driver.name])
    }

    fn get_estimators(
        &mut self,
        kinds: &[PriceEstimatorType],
        select: impl Fn(&EstimatorEntry) -> &Arc<dyn PriceEstimating>,
    ) -> Result<Vec<(String, Arc<dyn PriceEstimating>)>> {
        kinds
            .iter()
            .copied()
            .map(|kind| Ok((kind.name(), select(self.get_estimator(kind)?).clone())))
            .collect()
    }

    fn get_external_estimators(
        &mut self,
        drivers: &[Driver],
        select: impl Fn(&EstimatorEntry) -> &Arc<dyn PriceEstimating>,
    ) -> Result<Vec<(String, Arc<dyn PriceEstimating>)>> {
        drivers
            .iter()
            .cloned()
            .map(|driver| {
                Ok((
                    driver.name.clone(),
                    select(self.get_external_estimator(&driver)?).clone(),
                ))
            })
            .collect()
    }

    fn sanitized(&self, estimator: impl PriceEstimating) -> SanitizedPriceEstimator {
        SanitizedPriceEstimator::new(
            Box::new(estimator),
            self.network.native_token,
            self.components.bad_token_detector.clone(),
        )
    }

    pub fn price_estimator(
        &mut self,
        kinds: &[PriceEstimatorType],
        drivers: &[Driver],
    ) -> Result<Arc<dyn PriceEstimating>> {
        let mut estimators = self.get_estimators(kinds, |entry| &entry.optimal)?;
        estimators.append(&mut self.get_external_estimators(drivers, |entry| &entry.optimal)?);
        let competition_estimator = CompetitionPriceEstimator::new(estimators);
        Ok(Arc::new(self.sanitized(
            match self.args.enable_quote_predictions {
                true => {
                    competition_estimator.with_predictions(self.args.quote_prediction_confidence)
                }
                false => competition_estimator,
            },
        )))
    }

    pub fn fast_price_estimator(
        &mut self,
        kinds: &[PriceEstimatorType],
        fast_price_estimation_results_required: NonZeroUsize,
        drivers: &[Driver],
    ) -> Result<Arc<dyn PriceEstimating>> {
        let mut estimators = self.get_estimators(kinds, |entry| &entry.fast)?;
        estimators.append(&mut self.get_external_estimators(drivers, |entry| &entry.fast)?);
        Ok(Arc::new(self.sanitized(
            RacingCompetitionPriceEstimator::new(
                estimators,
                fast_price_estimation_results_required,
            ),
        )))
    }

    pub fn native_price_estimator(
        &mut self,
        kinds: &[PriceEstimatorType],
        drivers: &[Driver],
    ) -> Result<Arc<CachingNativePriceEstimator>> {
        anyhow::ensure!(
            self.args.native_price_cache_max_age_secs > self.args.native_price_prefetch_time_secs,
            "price cache prefetch time needs to be less than price cache max age"
        );
        let mut estimators = self.get_estimators(kinds, |entry| &entry.native)?;
        estimators.append(&mut self.get_external_estimators(drivers, |entry| &entry.native)?);
        let competition_estimator = CompetitionPriceEstimator::new(estimators);
        let native_estimator = Arc::new(CachingNativePriceEstimator::new(
            Box::new(NativePriceEstimator::new(
                Arc::new(
                    self.sanitized(match self.args.enable_quote_predictions {
                        true => competition_estimator
                            .with_predictions(self.args.quote_prediction_confidence),
                        false => competition_estimator,
                    }),
                ),
                self.network.native_token,
                self.native_token_price_estimation_amount()?,
            )),
            self.args.native_price_cache_max_age_secs,
            self.args.native_price_cache_refresh_secs,
            Some(self.args.native_price_cache_max_update_size),
            self.args.native_price_prefetch_time_secs,
            self.args.native_price_cache_concurrent_requests,
        ));
        Ok(native_estimator)
    }
}

/// Trait for modelling the initialization of a Price estimator and its verified
/// counter-part. This allows for generic price estimator creation, as well as
/// per-type trade verification configuration.
trait PriceEstimatorCreating: Sized {
    type Params;

    fn init(factory: &PriceEstimatorFactory, name: &str, params: Self::Params) -> Result<Self>;

    fn verified(&self, _: &TradeVerifier) -> Option<Self> {
        None
    }
}

impl PriceEstimatorCreating for BaselinePriceEstimator {
    type Params = ();

    fn init(factory: &PriceEstimatorFactory, name: &str, _: Self::Params) -> Result<Self> {
        Ok(BaselinePriceEstimator::new(
            factory.components.uniswap_v2_pools.clone(),
            factory.components.gas_price.clone(),
            factory.network.base_tokens.clone(),
            factory.network.native_token,
            factory.native_token_price_estimation_amount()?,
            factory.rate_limiter(name),
        ))
    }
}
impl PriceEstimatorCreating for ParaswapPriceEstimator {
    type Params = ();

    fn init(factory: &PriceEstimatorFactory, name: &str, _: Self::Params) -> Result<Self> {
        Ok(ParaswapPriceEstimator::new(
            Arc::new(DefaultParaswapApi {
                client: factory.components.http_factory.create(),
                partner: factory
                    .shared_args
                    .paraswap_partner
                    .clone()
                    .unwrap_or_default(),
            }),
            factory.components.tokens.clone(),
            factory.shared_args.disabled_paraswap_dexs.clone(),
            factory.rate_limiter(name),
            factory.network.settlement,
        ))
    }

    fn verified(&self, verifier: &TradeVerifier) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}
impl PriceEstimatorCreating for ZeroExPriceEstimator {
    type Params = ();

    fn init(factory: &PriceEstimatorFactory, name: &str, _: Self::Params) -> Result<Self> {
        Ok(ZeroExPriceEstimator::new(
            factory.components.zeroex.clone(),
            factory.shared_args.disabled_zeroex_sources.clone(),
            factory.rate_limiter(name),
            factory.network.settlement,
        ))
    }

    fn verified(&self, verifier: &TradeVerifier) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}

impl PriceEstimatorCreating for OneInchPriceEstimator {
    type Params = ();

    fn init(factory: &PriceEstimatorFactory, name: &str, _: Self::Params) -> Result<Self> {
        Ok(OneInchPriceEstimator::new(
            factory
                .components
                .oneinch
                .clone()
                .context("1Inch API not supported for network")?,
            factory.shared_args.disabled_one_inch_protocols.clone(),
            factory.rate_limiter(name),
            factory.network.settlement,
            factory.shared_args.one_inch_referrer_address,
        ))
    }

    fn verified(&self, verifier: &TradeVerifier) -> Option<Self> {
        Some(self.verified(verifier.clone()))
    }
}

impl PriceEstimatorCreating for BalancerSor {
    type Params = ();

    fn init(factory: &PriceEstimatorFactory, name: &str, _: Self::Params) -> Result<Self> {
        Ok(BalancerSor::new(
            Arc::new(DefaultBalancerSorApi::new(
                factory.components.http_factory.create(),
                factory
                    .args
                    .balancer_sor_url
                    .clone()
                    .context("balancer SOR url not specified")?,
                factory.network.chain_id,
            )?),
            factory.rate_limiter(name),
            factory.components.gas_price.clone(),
        ))
    }
}

#[derive(Debug, Clone)]
struct HttpPriceEstimatorParams {
    base: Url,
    solve_path: String,
    use_liquidity: bool,
}

impl PriceEstimatorCreating for HttpPriceEstimator {
    type Params = HttpPriceEstimatorParams;

    fn init(factory: &PriceEstimatorFactory, name: &str, params: Self::Params) -> Result<Self> {
        Ok(HttpPriceEstimator::new(
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
        ))
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
            factory.network.settlement,
        ))
    }

    fn verified(&self, verifier: &TradeVerifier) -> Option<Self> {
        Some(self.verified(verifier.clone()))
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
