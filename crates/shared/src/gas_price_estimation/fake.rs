use {
    anyhow::Result,
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    std::sync::{Arc, Mutex},
};

#[derive(Default)]
pub struct FakeGasPriceEstimator(pub Arc<Mutex<GasPrice1559>>);

impl FakeGasPriceEstimator {
    pub fn new(gas_price: GasPrice1559) -> Self {
        Self(Arc::new(Mutex::new(gas_price)))
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for FakeGasPriceEstimator {
    async fn estimate_with_limits(&self, _: f64, _: std::time::Duration) -> Result<GasPrice1559> {
        Ok(*self.0.lock().unwrap())
    }
}
