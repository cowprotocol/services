use {
    crate::{
        domain::{competition::solution::Settlement, eth},
        infra::{self, observe, solver::Solver},
    },
    futures::{future::select_ok, FutureExt},
    gas_estimation::GasPriceEstimating,
    thiserror::Error,
};

/// The mempools used to execute settlements.
#[derive(Debug, Clone)]
pub struct Mempools(Vec<infra::Mempool>);

impl Mempools {
    pub fn new(mempools: Vec<infra::Mempool>) -> Result<Self, NoMempools> {
        if mempools.is_empty() {
            Err(NoMempools)
        } else {
            Ok(Self(mempools))
        }
    }

    /// Publish a settlement to the mempools. Wait until it is confirmed in the
    /// background.
    pub fn execute(&self, solver: &Solver, settlement: &Settlement) {
        tokio::spawn(select_ok(self.0.iter().cloned().map(|mempool| {
            let solver = solver.clone();
            let settlement = settlement.clone();
            async move {
                let result = mempool.execute(&solver, settlement.clone()).await;
                observe::mempool_executed(&mempool, &settlement, &result);
                result
            }
            .boxed()
        })));
    }

    /// Get gas price that is used for executing the settlement.
    /// Since there are several mempools with different gas price estimators, we
    /// will get them all and then pick the highest one.
    pub async fn gas_price(&self, settlement: &Settlement) -> anyhow::Result<eth::GasPrice> {
        let mut gas_prices = Vec::new();
        for mempool in &self.0 {
            let gas_price_estimator = mempool.gas_price_estimator(settlement);
            let gas_price = gas_price_estimator.estimate().await?;
            gas_prices.push(gas_price);
        }

        gas_prices.sort_unstable_by(|a, b| {
            match a
                .max_priority_fee_per_gas
                .total_cmp(&b.max_priority_fee_per_gas)
            {
                std::cmp::Ordering::Equal => a.max_fee_per_gas.total_cmp(&b.max_fee_per_gas),
                ordering => ordering,
            }
        });

        gas_prices
            .last()
            .map(|gas_price| eth::GasPrice {
                max: eth::U256::from_f64_lossy(gas_price.max_fee_per_gas).into(),
                tip: eth::U256::from_f64_lossy(gas_price.max_priority_fee_per_gas).into(),
                base: eth::U256::from_f64_lossy(gas_price.base_fee_per_gas).into(),
            })
            .ok_or(anyhow::anyhow!("no gas price estimators"))
    }
}

#[derive(Debug, Error)]
#[error("no mempools configured, cannot execute settlements")]
pub struct NoMempools;
