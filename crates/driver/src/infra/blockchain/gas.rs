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

pub struct GasPriceEstimator {
    pub(super) gas: Arc<dyn GasPriceEstimating>,
    adjustments: GasPriceParameters,
}

impl GasPriceEstimator {
    pub async fn new(
        provider: &AlloyProvider,
        current_block: &CurrentBlockWatcher,
        gas_estimator_type: &GasEstimatorType,
        adjustments: GasPriceParameters,
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
            let tip_percentage_as_bps = self.adjustments.additional_tip_factor * 10000.0;
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

/// Set of tweaks to fine tune the computed gas price.
pub struct GasPriceParameters {
    /// Maximum priority_fee_per_gas added on top of the price suggested by the
    /// estimator.
    max_additional_tip: eth::U256,
    /// The max_priority_fee_per_gas suggested by the gas price estimator gets
    /// multiplied with this factor and get capped at `max_additional_tip`.
    additional_tip_factor: f64,
    /// Highest `max_fee_per_gas` at which we still want to submit a tx.
    max_fee_per_gas: eth::U256,
    /// We'll always tip at least this value for `max_priority_fee_per_gas`.
    min_priority_fee: eth::U256,
}

pub fn adjustments(mempools: &[mempool::Config]) -> GasPriceParameters {
    // TODO: make configuration of those parameters more obvious by moving them
    // into a separate config field instead of deriving them from the mempool
    // configs
    let mut max_additional_tip = eth::U256::ZERO;
    let mut additional_tip_percentage = 0.0f64;
    let mut max_fee_per_gas = eth::U256::MAX;
    let mut min_priority_fee = eth::U256::ZERO;

    for mempool in mempools {
        max_additional_tip = max_additional_tip.max(mempool.max_additional_tip);
        additional_tip_percentage =
            additional_tip_percentage.max(mempool.additional_tip_percentage);
        max_fee_per_gas = max_fee_per_gas.min(mempool.gas_price_cap);
        min_priority_fee = min_priority_fee.max(mempool.min_priority_fee);
    }

    GasPriceParameters {
        max_additional_tip,
        additional_tip_factor: additional_tip_percentage,
        max_fee_per_gas,
        min_priority_fee,
    }
}
