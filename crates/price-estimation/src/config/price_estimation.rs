use {
    balance_overrides::BalanceOverriding,
    configs::price_estimation::BalanceOverridesConfig,
    std::sync::Arc,
};

pub trait BalanceOverridesConfigExt {
    fn init(&self, web3: ethrpc::Web3) -> Arc<dyn BalanceOverriding>;
}

impl BalanceOverridesConfigExt for BalanceOverridesConfig {
    fn init(&self, web3: ethrpc::Web3) -> Arc<dyn BalanceOverriding> {
        Arc::new(balance_overrides::BalanceOverrides {
            hardcoded: self.token_overrides.inner().clone(),
            detector: self.autodetect.then(|| {
                (
                    balance_overrides::detector::Detector::new(web3, self.probing_depth),
                    std::sync::Mutex::new(cached::SizedCache::with_size(self.cache_size)),
                )
            }),
        })
    }
}
