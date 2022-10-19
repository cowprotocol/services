use super::{
    balancer_sor::BalancerSor,
    baseline::BaselinePriceEstimator,
    competition::{CompetitionPriceEstimator, RacingCompetitionPriceEstimator},
    http::HttpPriceEstimator,
    instrumented::InstrumentedPriceEstimator,
    native::{self, NativePriceEstimating, NativePriceEstimator},
    native_price_cache::CachingNativePriceEstimator,
    oneinch::OneInchPriceEstimator,
    paraswap::ParaswapPriceEstimator,
    sanitized::SanitizedPriceEstimator,
    zeroex::ZeroExPriceEstimator,
    Arguments, PriceEstimating, PriceEstimatorType,
};
use crate::{
    arguments,
    bad_token::BadTokenDetecting,
    balancer_sor_api::DefaultBalancerSorApi,
    baseline_solver::BaseTokens,
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
};
use anyhow::{Context as _, Result};
use ethcontract::{H160, U256};
use gas_estimation::GasPriceEstimating;
use reqwest::Url;
use std::{collections::HashMap, num::NonZeroUsize, sync::Arc};

/// A factory for initializing shared price estimators.
pub struct PriceEstimatorFactory<'a> {
    args: &'a Arguments,
    shared_args: &'a arguments::Arguments,
    network: Network,
    components: Components,
    estimators: HashMap<PriceEstimatorType, Arc<dyn PriceEstimating>>,
}

/// Network options needed for creating price estimators.
pub struct Network {
    pub name: String,
    pub chain_id: u64,
    pub native_token: H160,
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
    ) -> Self {
        Self {
            args,
            shared_args,
            network,
            components,
            estimators: HashMap::new(),
        }
    }

    fn native_token_price_estimation_amount(&self) -> Result<U256> {
        self.args
            .amount_to_estimate_prices_with
            .or_else(|| {
                native::default_amount_to_estimate_native_prices_with(self.network.chain_id)
            })
            .context("No amount to estimate prices with set.")
    }

    fn rate_limiter(&self, kind: PriceEstimatorType) -> Arc<RateLimiter> {
        Arc::new(RateLimiter::from_strategy(
            self.args
                .price_estimation_rate_limiter
                .clone()
                .unwrap_or_default(),
            format!("{kind:?}_estimator"),
        ))
    }

    fn create_http_estimator(
        &self,
        kind: PriceEstimatorType,
        base: Url,
    ) -> Box<dyn PriceEstimating> {
        Box::new(HttpPriceEstimator::new(
            Arc::new(DefaultHttpSolverApi {
                name: kind.name(),
                network_name: self.network.name.clone(),
                chain_id: self.network.chain_id,
                base,
                client: self.components.http_factory.create(),
                config: SolverConfig {
                    use_internal_buffers: Some(self.shared_args.quasimodo_uses_internal_buffers),
                    objective: Some(Objective::SurplusFeesCosts),
                    ..Default::default()
                },
            }),
            self.components.uniswap_v2_pools.clone(),
            self.components.balancer_pools.clone(),
            self.components.uniswap_v3_pools.clone(),
            self.components.tokens.clone(),
            self.components.gas_price.clone(),
            self.network.native_token,
            self.network.base_tokens.clone(),
            self.network.name.clone(),
            self.rate_limiter(kind),
        ))
    }

    fn create_estimator(&self, kind: PriceEstimatorType) -> Result<InstrumentedPriceEstimator> {
        let estimator: Box<dyn PriceEstimating> =
            match kind {
                PriceEstimatorType::Baseline => Box::new(BaselinePriceEstimator::new(
                    self.components.uniswap_v2_pools.clone(),
                    self.components.gas_price.clone(),
                    self.network.base_tokens.clone(),
                    self.network.native_token,
                    self.native_token_price_estimation_amount()?,
                    self.rate_limiter(kind),
                )),
                PriceEstimatorType::Paraswap => Box::new(ParaswapPriceEstimator::new(
                    Arc::new(DefaultParaswapApi {
                        client: self.components.http_factory.create(),
                        partner: self
                            .shared_args
                            .paraswap_partner
                            .clone()
                            .unwrap_or_default(),
                        rate_limiter: self.shared_args.paraswap_rate_limiter.clone().map(
                            |strategy| RateLimiter::from_strategy(strategy, "paraswap_api".into()),
                        ),
                    }),
                    self.components.tokens.clone(),
                    self.shared_args.disabled_paraswap_dexs.clone(),
                    self.rate_limiter(kind),
                )),
                PriceEstimatorType::ZeroEx => Box::new(ZeroExPriceEstimator::new(
                    self.components.zeroex.clone(),
                    self.shared_args.disabled_zeroex_sources.clone(),
                    self.rate_limiter(kind),
                )),
                PriceEstimatorType::Quasimodo => self.create_http_estimator(
                    kind,
                    self.args
                        .quasimodo_solver_url
                        .clone()
                        .context("quasimodo solver url not specified")?,
                ),
                PriceEstimatorType::OneInch => Box::new(OneInchPriceEstimator::new(
                    self.components
                        .oneinch
                        .clone()
                        .context("1Inch API not supported for network")?,
                    self.shared_args.disabled_one_inch_protocols.clone(),
                    self.rate_limiter(kind),
                    self.shared_args.one_inch_referrer_address,
                )),
                PriceEstimatorType::Yearn => self.create_http_estimator(
                    kind,
                    self.args
                        .yearn_solver_url
                        .clone()
                        .context("yearn solver url not specified")?,
                ),
                PriceEstimatorType::BalancerSor => Box::new(BalancerSor::new(
                    Arc::new(DefaultBalancerSorApi::new(
                        self.components.http_factory.create(),
                        self.args
                            .balancer_sor_url
                            .clone()
                            .context("balancer SOR url not specified")?,
                        self.network.chain_id,
                    )?),
                    self.rate_limiter(kind),
                    self.components.gas_price.clone(),
                )),
            };

        Ok(InstrumentedPriceEstimator::new(estimator, kind.name()))
    }

    fn get_estimator(&mut self, kind: PriceEstimatorType) -> Result<Arc<dyn PriceEstimating>> {
        if let Some(existing) = self.estimators.get(&kind) {
            return Ok(existing.clone());
        }

        let estimator = Arc::new(self.create_estimator(kind)?);
        self.estimators.insert(kind, estimator.clone());
        Ok(estimator)
    }

    fn get_estimators(
        &mut self,
        kinds: &[PriceEstimatorType],
    ) -> Result<Vec<(String, Arc<dyn PriceEstimating>)>> {
        kinds
            .iter()
            .copied()
            .map(|kind| Ok((kind.name(), self.get_estimator(kind)?)))
            .collect()
    }

    fn sanitized(&self, estimator: Box<dyn PriceEstimating>) -> SanitizedPriceEstimator {
        SanitizedPriceEstimator::new(
            estimator,
            self.network.native_token,
            self.components.bad_token_detector.clone(),
        )
    }

    pub fn price_estimator(
        &mut self,
        kinds: &[PriceEstimatorType],
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(kinds)?;
        Ok(Arc::new(self.sanitized(Box::new(
            CompetitionPriceEstimator::new(estimators),
        ))))
    }

    pub fn fast_price_estimator(
        &mut self,
        kinds: &[PriceEstimatorType],
        fast_price_estimation_results_required: NonZeroUsize,
    ) -> Result<Arc<dyn PriceEstimating>> {
        let estimators = self.get_estimators(kinds)?;
        Ok(Arc::new(self.sanitized(Box::new(
            RacingCompetitionPriceEstimator::new(
                estimators,
                fast_price_estimation_results_required,
            ),
        ))))
    }

    pub fn native_price_estimator(
        &mut self,
        kinds: &[PriceEstimatorType],
    ) -> Result<Arc<dyn NativePriceEstimating>> {
        let estimator = Arc::new(CachingNativePriceEstimator::new(
            Box::new(NativePriceEstimator::new(
                Arc::new(
                    self.sanitized(Box::new(CompetitionPriceEstimator::new(
                        kinds
                            .iter()
                            .copied()
                            .map(|kind| {
                                Ok((
                                    kind.name(),
                                    Arc::new(self.create_estimator(kind)?)
                                        as Arc<dyn PriceEstimating>,
                                ))
                            })
                            .collect::<Result<_>>()?,
                    ))),
                ),
                self.network.native_token,
                self.native_token_price_estimation_amount()?,
            )),
            self.args.native_price_cache_max_age_secs,
        ));

        estimator.spawn_maintenance_task(
            self.args.native_price_cache_refresh_secs,
            Some(self.args.native_price_cache_max_update_size),
        );
        Ok(estimator)
    }
}
