/// Wrapper around the gas estimation library.
/// Also allows to add additional tip to the gas price. This is used to
/// increase the chance of a transaction being included in a block, in case
/// private submission networks are used.
use {
    super::Error,
    crate::{domain::eth, infra::mempool},
    ethcontract::dyns::DynWeb3,
    gas_estimation::{nativegasestimator::NativeGasEstimator, GasPriceEstimating},
};

type MaxAdditionalTip = f64;
type AdditionalTipPercentage = f64;
type AdditionalTip = (MaxAdditionalTip, AdditionalTipPercentage);

pub struct GasPriceEstimator {
    gas: NativeGasEstimator,
    additional_tip: Option<AdditionalTip>,
}

impl GasPriceEstimator {
    pub async fn new(web3: &DynWeb3, mempools: &[mempool::Config]) -> Result<Self, Error> {
        let gas = NativeGasEstimator::new(web3.transport().clone(), None)
            .await
            .map_err(Error::Gas)?;
        let additional_tip = mempools
            .iter()
            .find(|mempool| matches!(mempool.kind, mempool::Kind::MEVBlocker { .. }))
            .map(|mempool| {
                (
                    match mempool.kind {
                        mempool::Kind::MEVBlocker {
                            max_additional_tip, ..
                        } => max_additional_tip,
                        _ => unreachable!(),
                    },
                    mempool.additional_tip_percentage,
                )
            });
        Ok(Self {
            gas,
            additional_tip,
        })
    }

    /// Estimates the gas price for a transaction.
    /// If additional tip is configured, it will be added to the gas price. This
    /// is to increase the chance of a transaction being included in a block, in
    /// case private submission networks are used.
    pub async fn estimate(&self) -> Result<eth::GasPrice, Error> {
        self.gas
            .estimate()
            .await
            .map(|mut estimate| {
                let estimate = match self.additional_tip {
                    Some((max_additional_tip, additional_tip_percentage)) => {
                        let additional_tip = max_additional_tip
                            .min(estimate.max_fee_per_gas * additional_tip_percentage);
                        estimate.max_fee_per_gas += additional_tip;
                        estimate.max_priority_fee_per_gas += additional_tip;
                        estimate = estimate.ceil();
                        estimate
                    }
                    None => estimate,
                };
                eth::GasPrice {
                    max: eth::U256::from_f64_lossy(estimate.max_fee_per_gas).into(),
                    tip: eth::U256::from_f64_lossy(estimate.max_priority_fee_per_gas).into(),
                    base: eth::U256::from_f64_lossy(estimate.base_fee_per_gas).into(),
                }
            })
            .map_err(Error::Gas)
    }
}
