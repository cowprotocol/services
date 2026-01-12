use {
    crate::gas_price_estimation::GasPriceEstimating,
    anyhow::Result,
    crate::gas_price_estimation::price::GasPrice1559,
};

#[derive(Default)]
pub struct FakeGasPriceEstimator(pub GasPrice1559);

impl FakeGasPriceEstimator {
    pub fn new(gas_price: GasPrice1559) -> Self {
        Self(gas_price)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for FakeGasPriceEstimator {
    async fn estimate_with_limits(&self, _: f64, _: std::time::Duration) -> Result<GasPrice1559> {
        Ok(self.0)
    }
}
