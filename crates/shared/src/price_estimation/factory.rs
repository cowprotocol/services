use {
    super::{
        competition::CompetitionEstimator,
        external::ExternalPriceEstimator,
        instrumented::InstrumentedPriceEstimator,
        native::{self, NativePriceEstimator},
        native_price_cache::CachingNativePriceEstimator,
        sanitized::SanitizedPriceEstimator,
        trade_verifier::{TradeVerifier, TradeVerifying},
        Arguments,
        NativePriceEstimator as NativePriceEstimatorSource,
        PriceEstimating,
    },
    crate::{
        arguments::{self, ExternalSolver},
        bad_token::BadTokenDetecting,
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        code_simulation::{self, CodeSimulating, TenderlyCodeSimulator},
        ethrpc::Web3,
        http_client::HttpClientFactory,
        price_estimation::{
            buffered::{self, BufferedRequest, NativePriceBatchFetching},
            competition::PriceRanking,
            native::NativePriceEstimating,
        },
        token_info::TokenInfoFetching,
    },
    anyhow::{Context as _, Result},
    ethcontract::H160,
    ethrpc::block_stream::CurrentBlockWatcher,
    gas_estimation::GasPriceEstimating,
    number::nonzero::U256 as NonZeroU256,
    rate_limit::RateLimiter,
    reqwest::Url,
    std::{collections::HashMap, num::NonZeroUsize, sync::Arc},
};

/// A factory for initializing shared price estimators.
pub struct PriceEstimatorFactory<'a> {
    args: &'a Arguments,
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
    pub chain: chain::Chain,
    pub native_token: H160,
    pub settlement: H160,
    pub authenticator: H160,
    pub base_tokens: Arc<BaseTokens>,
    pub block_stream: CurrentBlockWatcher,
}

/// The shared components needed for creating price estimators.
pub struct Components {
    pub http_factory: HttpClientFactory,
    pub bad_token_detector: Arc<dyn BadTokenDetecting>,
    pub tokens: Arc<dyn TokenInfoFetching>,
    pub code_fetcher: Arc<CachedCodeFetcher>,
}

impl<'a> PriceEstimatorFactory<'a> {
    pub async fn new(
        args: &'a Arguments,
        shared_args: &'a arguments::Arguments,
        network: Network,
        components: Components,
    ) -> Result<Self> {
        Ok(Self {
            trade_verifier: Self::trade_verifier(args, shared_args, &network, &components).await?,
            args,
            network,
            components,
            estimators: HashMap::new(),
        })
    }

    async fn trade_verifier(
        args: &'a Arguments,
        shared_args: &arguments::Arguments,
        network: &Network,
        components: &Components,
    ) -> Result<Option<Arc<dyn TradeVerifying>>> {
        let Some(web3) = network.simulation_web3.clone() else {
            return Ok(None);
        };
        let web3 = ethrpc::instrumented::instrument_with_label(&web3, "simulator".into());

        let tenderly = shared_args
            .tenderly
            .get_api_instance(&components.http_factory, "price_estimation".to_owned())
            .unwrap()
            .map(|t| TenderlyCodeSimulator::new(t, network.chain.id()));

        let simulator: Arc<dyn CodeSimulating> = match tenderly {
            Some(tenderly) => Arc::new(code_simulation::Web3ThenTenderly::new(
                web3.clone(),
                tenderly,
            )),
            None => Arc::new(web3.clone()),
        };

        let balance_overrides = args.balance_overrides.init(simulator.clone());

        let verifier = TradeVerifier::new(
            web3,
            simulator,
            components.code_fetcher.clone(),
            balance_overrides,
            network.block_stream.clone(),
            network.settlement,
            network.native_token,
            args.quote_inaccuracy_limit.clone(),
        )
        .await?;
        Ok(Some(Arc::new(verifier)))
    }

    fn native_token_price_estimation_amount(&self) -> Result<NonZeroU256> {
        NonZeroU256::try_from(
            self.args
                .amount_to_estimate_prices_with
                .or_else(|| {
                    Some(
                        self.network
                            .chain
                            .default_amount_to_estimate_native_prices_with(),
                    )
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
        weth: &contracts::WETH9,
    ) -> Result<(String, Arc<dyn NativePriceEstimating>)> {
        match source {
            NativePriceEstimatorSource::Driver(driver) => {
                let native_token_price_estimation_amount =
                    self.native_token_price_estimation_amount()?;
                let estimator = self.get_estimator(driver)?.native.clone();
                Ok((
                    driver.name.clone(),
                    Arc::new(InstrumentedPriceEstimator::new(
                        NativePriceEstimator::new(
                            Arc::new(self.sanitized(estimator)),
                            self.network.native_token,
                            native_token_price_estimation_amount,
                        ),
                        driver.name.to_string(),
                    )),
                ))
            }
            NativePriceEstimatorSource::OneInchSpotPriceApi => {
                let name = "OneInchSpotPriceApi".to_string();
                Ok((
                    name.clone(),
                    Arc::new(InstrumentedPriceEstimator::new(
                        native::OneInch::new(
                            self.components.http_factory.create(),
                            self.args.one_inch_url.clone(),
                            self.args.one_inch_api_key.clone(),
                            self.network.chain.id().into(),
                            self.network.block_stream.clone(),
                            self.components.tokens.clone(),
                        ),
                        name,
                    )),
                ))
            }
            NativePriceEstimatorSource::CoinGecko => {
                let name = "CoinGecko".to_string();
                let coin_gecko = native::CoinGecko::new(
                    self.components.http_factory.create(),
                    self.args.coin_gecko.coin_gecko_url.clone(),
                    self.args.coin_gecko.coin_gecko_api_key.clone(),
                    &self.network.chain,
                    weth.address(),
                    self.components.tokens.clone(),
                )
                .await?;

                let coin_gecko: Arc<dyn NativePriceEstimating> =
                    if let Some(coin_gecko_buffered_configuration) =
                        &self.args.coin_gecko.coin_gecko_buffered
                    {
                        let configuration = buffered::Configuration {
                            max_concurrent_requests: Some(
                                coin_gecko
                                    .max_batch_size()
                                    .try_into()
                                    .context("invalid CoinGecko max batch size")?,
                            ),
                            debouncing_time: coin_gecko_buffered_configuration
                                .coin_gecko_debouncing_time
                                .unwrap(),
                            result_ready_timeout: coin_gecko_buffered_configuration
                                .coin_gecko_result_ready_timeout
                                .unwrap(),
                            broadcast_channel_capacity: coin_gecko_buffered_configuration
                                .coin_gecko_broadcast_channel_capacity
                                .unwrap(),
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
            timeout: self.args.quote_timeout,
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
            self.components.bad_token_detector.clone(),
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

    pub async fn native_price_estimator(
        &mut self,
        native: &[Vec<NativePriceEstimatorSource>],
        results_required: NonZeroUsize,
        weth: contracts::WETH9,
    ) -> Result<Arc<CachingNativePriceEstimator>> {
        anyhow::ensure!(
            self.args.native_price_cache_max_age > self.args.native_price_prefetch_time,
            "price cache prefetch time needs to be less than price cache max age"
        );

        let mut estimators = Vec::with_capacity(native.len());
        for stage in native.iter() {
            let mut stages = Vec::with_capacity(stage.len());
            for source in stage {
                stages.push(self.create_native_estimator(source, &weth).await?);
            }
            estimators.push(stages);
        }

        let competition_estimator =
            CompetitionEstimator::new(estimators, PriceRanking::MaxOutAmount)
                .with_verification(self.args.quote_verification)
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
    timeout: std::time::Duration,
}

impl PriceEstimatorCreating for ExternalPriceEstimator {
    type Params = ExternalEstimatorParams;

    fn init(factory: &PriceEstimatorFactory, name: &str, params: Self::Params) -> Result<Self> {
        Ok(Self::new(
            params.driver,
            factory.components.http_factory.create(),
            factory.rate_limiter(name),
            factory.network.block_stream.clone(),
            params.timeout,
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
