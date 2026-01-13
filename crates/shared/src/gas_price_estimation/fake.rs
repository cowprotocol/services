use {
    crate::gas_price_estimation::{GasPriceEstimating, price::GasPrice1559},
    anyhow::Result,
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
    async fn estimate(&self) -> Result<GasPrice1559> {
        Ok(self.0)
    }
}
