use {
    crate::trade_verifier::tenderly_api::{Instrumented, TenderlyApi, TenderlyHttpApi},
    anyhow::Result,
    balance_overrides::BalanceOverriding,
    configs::price_estimation::{BalanceOverridesConfig, TenderlyConfig},
    http_client::HttpClientFactory,
    std::sync::Arc,
};

pub trait BalanceOverridesConfigExt {
    fn init(&self, web3: ethrpc::Web3) -> Arc<dyn BalanceOverriding>;
}

impl BalanceOverridesConfigExt for BalanceOverridesConfig {
    fn init(&self, web3: ethrpc::Web3) -> Arc<dyn BalanceOverriding> {
        Arc::new(balance_overrides::BalanceOverrides {
            hardcoded: self.token_overrides.clone().into_inner(),
            detector: self.autodetect.then(|| {
                (
                    balance_overrides::detector::Detector::new(web3, self.probing_depth),
                    std::sync::Mutex::new(cached::SizedCache::with_size(self.cache_size)),
                )
            }),
        })
    }
}

pub trait TenderlyConfigExt {
    fn get_api_instance(
        &self,
        http_factory: &HttpClientFactory,
        name: String,
    ) -> Result<Option<Arc<dyn TenderlyApi>>>;
}

impl TenderlyConfigExt for TenderlyConfig {
    fn get_api_instance(
        &self,
        http_factory: &HttpClientFactory,
        name: String,
    ) -> Result<Option<Arc<dyn TenderlyApi>>> {
        Some(())
            .and_then(|_| {
                Some(
                    TenderlyHttpApi::new(
                        http_factory,
                        self.user.as_deref()?,
                        self.project.as_deref()?,
                        self.api_key.as_deref()?,
                    )
                    .map(|inner| Arc::new(Instrumented { inner, name }) as _),
                )
            })
            .transpose()
    }
}
