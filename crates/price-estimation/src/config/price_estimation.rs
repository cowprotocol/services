use {
    balance_overrides::StateOverriding,
    configs::price_estimation::BalanceOverridesConfig,
    std::sync::Arc,
};

pub trait BalanceOverridesConfigExt {
    fn init(&self, provider: ethrpc::AlloyProvider) -> Arc<dyn StateOverriding>;
}

impl BalanceOverridesConfigExt for BalanceOverridesConfig {
    fn init(&self, provider: ethrpc::AlloyProvider) -> Arc<dyn StateOverriding> {
        Arc::new(balance_overrides::StateOverrides::with_config(
            provider,
            self.probing_depth,
            self.detection_timeout,
            self.cache_size,
        ))
    }
}
