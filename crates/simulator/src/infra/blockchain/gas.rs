/// Wrapper around the gas estimation library.
/// Also allows to add additional tip to the gas price. This is used to
/// increase the chance of a transaction being included in a block, in case
/// private submission networks are used.
use {
    super::Error,
    crate::infra::config::{GasEstimatorType, MempoolConfig},
    alloy::eips::eip1559::Eip1559Estimation,
    anyhow::anyhow,
    ethrpc::Web3,
    shared::gas_price_estimation::{
        GasPriceEstimating,
        configurable_alloy::{ConfigurableGasPriceEstimator, EstimatorConfig},
        eth_node::NodeGasPriceEstimator,
    },
    shared::domain::eth,
    std::sync::Arc,
};

type MaxAdditionalTip = eth::U256;
type AdditionalTipPercentage = f64;
type AdditionalTip = (MaxAdditionalTip, AdditionalTipPercentage);

pub struct GasPriceEstimator {
    gas: Arc<dyn GasPriceEstimating>,
    additional_tip: AdditionalTip,
    max_fee_per_gas: eth::U256,
    min_priority_fee: eth::U256,
}

impl GasPriceEstimator {
    pub async fn new(
        web3: &Web3,
        gas_estimator_type: &GasEstimatorType,
        mempools: &[MempoolConfig],
    ) -> Result<Self, Error> {
        let gas: Arc<dyn GasPriceEstimating> = match gas_estimator_type {
            GasEstimatorType::Web3 => Arc::new(NodeGasPriceEstimator::new(web3.provider.clone())),
            GasEstimatorType::Alloy {
                past_blocks,
                reward_percentile,
            } => Arc::new(ConfigurableGasPriceEstimator::new(
                web3.provider.clone(),
                EstimatorConfig {
                    past_blocks: *past_blocks,
                    reward_percentile: *reward_percentile,
                },
            )),
        };
        // TODO: simplify logic by moving gas price adjustments out of the individual
        // mempool configs
        let additional_tip = mempools
            .iter()
            .map(|mempool| {
                (
                    mempool.max_additional_tip,
                    mempool.additional_tip_percentage,
                )
            })
            .next()
            .unwrap_or((eth::U256::ZERO, 0.));
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
    pub async fn estimate(&self) -> Result<Eip1559Estimation, Error> {
        let estimate = self.gas.estimate().await.map_err(Error::GasPrice)?;

        let max_priority_fee_per_gas = {
            // the driver supports tweaking the tx gas price tip in case the gas
            // price estimator is systematically too low => compute configured tip bump
            let (max_additional_tip, tip_percentage_increase) = self.additional_tip;

            // Calculate additional tip in integer space to avoid precision loss
            // Convert percentage to basis points (multiply by 10000) to maintain precision
            // e.g., tip_percentage_increase = 0.125 (12.5%) becomes 1250
            let overflow_err = || {
                Error::GasPrice(anyhow!(
                    "overflow on multiplication (max_priority_fee_per_gas * tip_percentage_as_bps)"
                ))
            };
            let tip_percentage_as_bps = tip_percentage_increase * 10000.0;
            let calculated_tip = eth::U256::from(estimate.max_priority_fee_per_gas)
                .checked_mul(eth::U256::from(tip_percentage_as_bps))
                .ok_or_else(overflow_err)?
                / eth::U256::from(10000u128);

            let additional_tip = max_additional_tip.min(calculated_tip);

            // make sure we tip at least some configurable minimum amount
            std::cmp::max(
                self.min_priority_fee,
                eth::U256::from(estimate.max_priority_fee_per_gas) + additional_tip,
            )
        };

        // make sure the used max fee per gas is at least big enough to cover the tip -
        // otherwise the tx will be rejected by the node immediately
        let suggested_max_fee_per_gas = eth::U256::from(estimate.max_fee_per_gas);
        let suggested_max_fee_per_gas =
            std::cmp::max(suggested_max_fee_per_gas, max_priority_fee_per_gas);
        if suggested_max_fee_per_gas > self.max_fee_per_gas {
            return Err(Error::GasPrice(anyhow::anyhow!(
                "suggested gas price is higher than maximum allowed gas price (network is too \
                 congested)"
            )));
        }

        Ok(Eip1559Estimation {
            max_fee_per_gas: u128::try_from(suggested_max_fee_per_gas)
                .map_err(|err| Error::GasPrice(err.into()))?,
            max_priority_fee_per_gas: u128::try_from(max_priority_fee_per_gas)
                .map_err(|err| Error::GasPrice(err.into()))?,
        })
    }
}
