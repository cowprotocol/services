/// Wrapper around the gas estimation library.
/// Also allows to add additional tip to the gas price. This is used to
/// increase the chance of a transaction being included in a block, in case
/// private submission networks are used.
use {
    super::Error,
    crate::{
        domain::eth,
        infra::{config::file::GasEstimatorType, mempool},
    },
    ethcontract::dyns::DynWeb3,
    gas_estimation::{GasPriceEstimating, nativegasestimator::NativeGasEstimator},
    std::{sync::Arc, time::Duration},
};

type MaxAdditionalTip = eth::U256;
type AdditionalTipPercentage = f64;
type AdditionalTip = (MaxAdditionalTip, AdditionalTipPercentage);

pub struct GasPriceEstimator {
    //TODO: remove visibility once boundary is removed
    pub(super) gas: Arc<dyn GasPriceEstimating>,
    additional_tip: Option<AdditionalTip>,
    max_fee_per_gas: eth::U256,
    min_priority_fee: eth::U256,
}

impl GasPriceEstimator {
    pub async fn new(
        web3: &DynWeb3,
        gas_estimator_type: &GasEstimatorType,
        mempools: &[mempool::Config],
    ) -> Result<Self, Error> {
        let gas: Arc<dyn GasPriceEstimating> = match gas_estimator_type {
            GasEstimatorType::Native => Arc::new(
                NativeGasEstimator::new(web3.transport().clone(), None)
                    .await
                    .map_err(Error::GasPrice)?,
            ),
            GasEstimatorType::Web3 => Arc::new(web3.clone()),
        };
        let additional_tip = mempools
            .iter()
            .map(|mempool| match mempool.kind {
                mempool::Kind::MEVBlocker {
                    max_additional_tip,
                    additional_tip_percentage,
                    ..
                } => (max_additional_tip, additional_tip_percentage),
                mempool::Kind::Public {
                    max_additional_tip,
                    additional_tip_percentage,
                    ..
                } => (max_additional_tip, additional_tip_percentage),
            })
            .next();
        // Use the lowest max_fee_per_gas of all mempools as the max_fee_per_gas
        let max_fee_per_gas = mempools
            .iter()
            .map(|mempool| mempool.gas_price_cap)
            .min()
            .expect("at least one mempool");

        // Use the highest min_priority_fee of all mempools as the min_priority_fee
        let min_priority_fee = mempools
            .iter()
            .map(|mempool| mempool.min_priority_fee)
            .max()
            .expect("at least one mempool");
        Ok(Self {
            gas,
            additional_tip,
            max_fee_per_gas,
            min_priority_fee,
        })
    }

    /// Estimates the gas price for a transaction.
    /// If additional tip is configured, it will be added to the gas price. This
    /// is to increase the chance of a transaction being included in a block, in
    /// case private submission networks are used.
    pub async fn estimate(&self, time_limit: Option<Duration>) -> Result<eth::GasPrice, Error> {
        self.gas
            .estimate_with_limits(21000., time_limit.unwrap_or(Duration::from_secs(30)))
            .await
            .map(|mut estimate| {
                let estimate = match self.additional_tip {
                    Some((max_additional_tip, additional_tip_percentage)) => {
                        let additional_tip = max_additional_tip
                            .to_f64_lossy()
                            .min(estimate.max_fee_per_gas * additional_tip_percentage);
                        estimate.max_fee_per_gas += additional_tip;
                        estimate.max_priority_fee_per_gas += additional_tip;
                        estimate
                    }
                    None => estimate,
                };
                eth::GasPrice::new(
                    self.max_fee_per_gas.into(),
                    std::cmp::max(
                        self.min_priority_fee.into(),
                        eth::U256::from_f64_lossy(estimate.max_priority_fee_per_gas).into(),
                    ),
                    eth::U256::from_f64_lossy(estimate.base_fee_per_gas).into(),
                )
            })
            .map_err(Error::GasPrice)
    }
}
