use {
    super::{
        competition::{CompetitionPriceEstimator, RacingCompetitionPriceEstimator},
        external::ExternalPriceEstimator,
        instrumented::InstrumentedPriceEstimator,
        native::{self, NativePriceEstimator},
        native_price_cache::CachingNativePriceEstimator,
        sanitized::SanitizedPriceEstimator,
        trade_finder::TradeVerifier,
        Arguments,
        PriceEstimating,
    },
    crate::{
        arguments::{self, CodeSimulatorKind, ExternalSolver},
        bad_token::BadTokenDetecting,
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        code_simulation::{self, CodeSimulating, TenderlyCodeSimulator},
        ethrpc::Web3,
        http_client::HttpClientFactory,
        rate_limiter::RateLimiter,
    },
    anyhow::{Context as _, Result},
    ethcontract::{H160, U256},
    reqwest::Url,
    std::{collections::HashMap, num::NonZeroUsize, sync::Arc},
};

/// A factory for initializing shared price estimators.
pub struct PriceEstimatorFactory<'a> {
    args: &'a Arguments,
    network: Network,
    components: Components,
    trade_verifier: Option<TradeVerifier>,
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
}

/// The shared components needed for creating price estimators.
pub struct Components {
    pub http_factory: HttpClientFactory,
    pub bad_token_detector: Arc<dyn BadTokenDetecting>,
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
                    CodeSimulatorKind::Web3 => {
                        Arc::new(web3_simulator()?) as Arc<dyn CodeSimulating>
                    }
                    CodeSimulatorKind::Tenderly => Arc::new(
                        tenderly_simulator()?
                            .save(false, args.tenderly_save_failed_trade_simulations),
                    ),
                    CodeSimulatorKind::Web3ThenTenderly => {
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
                    network.settlement,
                    network.native_token,
                ))
            })
            .transpose()?;

        Ok(Self {
            args,
            network,
            components,
            trade_verifier,
            estimators: HashMap::new(),
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

    fn get_estimator(&mut self, solver: &ExternalSolver) -> Result<&EstimatorEntry> {
        let name = &solver.name;

        if !self.estimators.contains_key(name) {
            let estimator =
                self.create_estimator_entry::<ExternalPriceEstimator>(&solver.name, solver.into())?;
            self.estimators.insert(name.clone(), estimator);
        }

        Ok(&self.estimators[name])
    }

    fn get_estimators(
        &mut self,
        sources: &[ExternalSolver],
        select: impl Fn(&EstimatorEntry) -> &Arc<dyn PriceEstimating>,
    ) -> Result<Vec<(String, Arc<dyn PriceEstimating>)>> {
        sources
            .iter()
            .map(|source| {
                Ok((
                    source.name.clone(),
                    select(self.get_estimator(source)?).clone(),
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
        sources: &[ExternalSolver],
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(sources, |entry| &entry.optimal)?;
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
        sources: &[ExternalSolver],
        fast_price_estimation_results_required: NonZeroUsize,
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(sources, |entry| &entry.fast)?;
        Ok(Arc::new(self.sanitized(
            RacingCompetitionPriceEstimator::new(
                estimators,
                fast_price_estimation_results_required,
            ),
        )))
    }

    pub fn native_price_estimator(
        &mut self,
        sources: &[ExternalSolver],
    ) -> Result<Arc<CachingNativePriceEstimator>> {
        anyhow::ensure!(
            self.args.native_price_cache_max_age_secs > self.args.native_price_prefetch_time_secs,
            "price cache prefetch time needs to be less than price cache max age"
        );
        let estimators = self.get_estimators(sources, |entry| &entry.native)?;
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
        ))
    }

    fn verified(&self, verifier: &TradeVerifier) -> Option<Self> {
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
