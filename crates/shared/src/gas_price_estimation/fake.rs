use {
    crate::gas_price_estimation::GasPriceEstimating, alloy::eips::eip1559::Eip1559Estimation,
    anyhow::Result,
};

pub struct FakeGasPriceEstimator(pub Eip1559Estimation);

impl Default for FakeGasPriceEstimator {
    fn default() -> Self {
        Self(Eip1559Estimation {
            max_fee_per_gas: Default::default(),
            max_priority_fee_per_gas: Default::default(),
        })
    }
}

impl FakeGasPriceEstimator {
    pub fn new(gas_price: Eip1559Estimation) -> Self {
        Self(gas_price)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for FakeGasPriceEstimator {
    async fn estimate(&self) -> Result<Eip1559Estimation> {
        Ok(self.0)
    }
}
