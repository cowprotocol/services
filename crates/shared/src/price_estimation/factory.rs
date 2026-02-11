use {
    super::{
        Arguments,
        NativePriceEstimator as NativePriceEstimatorSource,
        PriceEstimating,
        competition::CompetitionEstimator,
        external::ExternalPriceEstimator,
        instrumented::InstrumentedPriceEstimator,
        native::{self, NativePriceEstimator},
        native_price_cache::{ApproximationToken, CachingNativePriceEstimator},
        sanitized::SanitizedPriceEstimator,
        trade_verifier::{TradeVerifier, TradeVerifying},
    },
    crate::{
        arguments,
        bad_token::BadTokenDetecting,
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        gas_price_estimation::GasPriceEstimating,
        http_client::HttpClientFactory,
        price_estimation::{
            ExternalSolver,
            buffered::{self, BufferedRequest, NativePriceBatchFetching},
            competition::PriceRanking,
            native::NativePriceEstimating,
        },
        tenderly_api::TenderlyCodeSimulator,
        token_info::TokenInfoFetching,
        web3::Web3,
    },
    alloy::primitives::Address,
    anyhow::{Context as _, Result},
    contracts::alloy::WETH9,
    ethrpc::{alloy::ProviderLabelingExt, block_stream::CurrentBlockWatcher},
    number::nonzero::NonZeroU256,
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
    pub native_token: Address,
    pub settlement: Address,
    pub authenticator: Address,
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
        let web3 = web3.labeled("simulator");

        let tenderly = shared_args
            .tenderly
            .get_api_instance(&components.http_factory, "price_estimation".to_owned())
            .unwrap()
            .map(|t| Arc::new(TenderlyCodeSimulator::new(t, network.chain.id())));

        let balance_overrides = args.balance_overrides.init(web3.clone());

        let verifier = TradeVerifier::new(
            web3,
            tenderly,
            components.code_fetcher.clone(),
            balance_overrides,
            network.block_stream.clone(),
            network.settlement,
            network.native_token,
            args.quote_inaccuracy_limit.clone(),
            args.tokens_without_verification.iter().cloned().collect(),
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
            NativePriceEstimatorSource::Forwarder(url) => {
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
                let name = "OneInchSpotPriceApi".to_string();
                Ok((
                    name.clone(),
                    Arc::new(InstrumentedPriceEstimator::new(
                        native::OneInch::new(
                            self.components.http_factory.create(),
                            self.args.one_inch_url.clone(),
                            self.args.one_inch_api_key.clone(),
                            self.network.chain.id(),
                            self.network.block_stream.clone(),
                            self.components.tokens.clone(),
                        ),
                        name,
                    )),
                ))
            }
            NativePriceEstimatorSource::CoinGecko => {
                anyhow::ensure!(
                    self.args.coin_gecko.coin_gecko_api_key.is_some(),
                    "coin_gecko_api_key must be set when CoinGecko is used as native price                     estimator"
                );

                let name = "CoinGecko".to_string();
                let coin_gecko = native::CoinGecko::new(
                    self.components.http_factory.create(),
                    self.args.coin_gecko.coin_gecko_url.clone(),
                    self.args.coin_gecko.coin_gecko_api_key.clone(),
                    &self.network.chain,
                    *weth.address(),
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
                            result_ready_timeout: self.args.quote_timeout,
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
            self.components.bad_token_detector.clone(),
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

    pub async fn native_price_estimator(
        &mut self,
        native: &[Vec<NativePriceEstimatorSource>],
        results_required: NonZeroUsize,
        weth: WETH9::Instance,
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
        let approximation_tokens = self.build_approximation_tokens().await.context(
            "failed to build native price approximation tokens with normalization factors",
        )?;

        let native_estimator = Arc::new(CachingNativePriceEstimator::new(
            Box::new(competition_estimator),
            self.args.native_price_cache_max_age,
            self.args.native_price_cache_refresh,
            Some(self.args.native_price_cache_max_update_size),
            self.args.native_price_prefetch_time,
            self.args.native_price_cache_concurrent_requests,
            approximation_tokens,
            self.args.quote_timeout,
        ));
        Ok(native_estimator)
    }

    /// Builds the approximation tokens mapping with normalization factors based
    /// on decimal differences between token pairs.
    async fn build_approximation_tokens(&self) -> Result<HashMap<Address, ApproximationToken>> {
        let pairs = &self.args.native_price_approximation_tokens;
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
