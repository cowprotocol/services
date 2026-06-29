/// Wrapper around the gas estimation library.
/// Also allows to add additional tip to the gas price. This is used to
/// increase the chance of a transaction being included in a block, in case
/// private submission networks are used.
use {
    super::Error,
    crate::infra::{config::file::GasEstimatorType, mempool},
    alloy::eips::eip1559::Eip1559Estimation,
    anyhow::anyhow,
    eth_domain_types as eth,
    ethrpc::{AlloyProvider, block_stream::CurrentBlockWatcher},
    gas_price_estimation::{
        GasPriceEstimating,
        configurable_alloy::ConfigurableGasPriceEstimator,
        eth_node::NodeGasPriceEstimator,
    },
    std::sync::Arc,
};

/// Set of tweaks to fine tune the computed gas price.
pub struct ManualAdjustments {
    max_additional_tip: eth::U256,
    additional_tip_percentage: f64,
    max_fee_per_gas: eth::U256,
    min_priority_fee: eth::U256,
}

pub struct GasPriceEstimator {
    pub(super) gas: Arc<dyn GasPriceEstimating>,
    adjustments: ManualAdjustments,
}

impl GasPriceEstimator {
    pub async fn new(
        provider: &AlloyProvider,
        current_block: &CurrentBlockWatcher,
        gas_estimator_type: &GasEstimatorType,
        adjustments: ManualAdjustments,
    ) -> Result<Self, Error> {
        let gas: Arc<dyn GasPriceEstimating> = match gas_estimator_type {
            GasEstimatorType::Web3 => Arc::new(NodeGasPriceEstimator::new(provider.clone())),
            GasEstimatorType::Alloy {
                past_blocks,
                reward_percentile,
            } => Arc::new(ConfigurableGasPriceEstimator::new(
                provider.clone(),
                configs::gas_price_estimation::EstimatorConfig {
                    past_blocks: *past_blocks,
                    reward_percentile: *reward_percentile,
                },
                current_block.clone(),
            )),
        };
        Ok(Self { gas, adjustments })
    }

    /// Estimates the gas price for a transaction.
    /// If additional tip is configured, it will be added to the gas price. This
    /// is to increase the chance of a transaction being included in a block, in
    /// case private submission networks are used.
    pub async fn estimate(&self) -> Result<Eip1559Estimation, Error> {
        let estimate = self.gas.estimate().await.map_err(Error::GasPrice)?;

        let max_priority_fee_per_gas = {
            // Calculate additional tip in integer space to avoid precision loss
            // Convert percentage to basis points (multiply by 10000) to maintain precision
            // e.g., additional_tip_percentage = 0.125 (12.5%) becomes 1250
            let overflow_err = || {
                Error::GasPrice(anyhow!(
                    "overflow on multiplication (max_priority_fee_per_gas * tip_percentage_as_bps)"
                ))
            };
            let tip_percentage_as_bps = self.adjustments.additional_tip_percentage * 10000.0;
            let calculated_tip = eth::U256::from(estimate.max_priority_fee_per_gas)
                .checked_mul(eth::U256::from(tip_percentage_as_bps))
                .ok_or_else(overflow_err)?
                / eth::U256::from(10000u128);

            let additional_tip = self.adjustments.max_additional_tip.min(calculated_tip);

            // make sure we tip at least some configurable minimum amount
            std::cmp::max(
                self.adjustments.min_priority_fee,
                eth::U256::from(estimate.max_priority_fee_per_gas) + additional_tip,
            )
        };

        // make sure the used max fee per gas is at least big enough to cover the tip -
        // otherwise the tx will be rejected by the node immediately
        let suggested_max_fee_per_gas = eth::U256::from(estimate.max_fee_per_gas);
        let suggested_max_fee_per_gas =
            std::cmp::max(suggested_max_fee_per_gas, max_priority_fee_per_gas);
        if suggested_max_fee_per_gas > self.adjustments.max_fee_per_gas {
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

#[async_trait::async_trait]
impl GasPriceEstimating for GasPriceEstimator {
    async fn estimate(&self) -> ::anyhow::Result<Eip1559Estimation> {
        GasPriceEstimator::estimate(self).await.map_err(Into::into)
    }

    async fn base_fee(&self) -> ::anyhow::Result<Option<u64>> {
        self.gas.base_fee().await
    }
}

pub fn adjustments(mempools: &[mempool::Config]) -> ManualAdjustments {
    // TODO: simplify logic by moving gas price adjustments out of the individual
    // mempool configs
    let (max_additional_tip, additional_tip_percentage) = mempools
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

    ManualAdjustments {
        max_additional_tip,
        additional_tip_percentage,
        max_fee_per_gas,
        min_priority_fee,
    }
}
