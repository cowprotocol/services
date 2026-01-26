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
    ethrpc::Web3,
    shared::gas_price_estimation::{
        GasPriceEstimating,
        configurable_alloy::{ConfigurableGasPriceEstimator, EstimatorConfig},
        eth_node::NodeGasPriceEstimator,
    },
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
        mempools: &[mempool::Config],
    ) -> Result<Self, Error> {
        let gas: Arc<dyn GasPriceEstimating> = match gas_estimator_type {
            GasEstimatorType::Web3 => Arc::new(NodeGasPriceEstimator::new(web3.alloy.clone())),
            GasEstimatorType::Alloy {
                past_blocks,
                reward_percentile,
            } => Arc::new(ConfigurableGasPriceEstimator::new(
                web3.alloy.clone(),
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
    pub async fn estimate(&self) -> Result<eth::GasPrice, Error> {
        let estimate = self.gas.estimate().await.map_err(Error::GasPrice)?;

        let max_priority_fee_per_gas = {
            // the driver supports tweaking the tx gas price tip in case the gas
            // price estimator is systematically too low => compute configured tip bump
            let (max_additional_tip, tip_percentage_increase) = self.additional_tip;
            let additional_tip = f64::from(max_additional_tip)
                .min(estimate.max_priority_fee_per_gas * tip_percentage_increase);

            // make sure we tip at least some configurable minimum amount
            std::cmp::max(
                self.min_priority_fee,
                eth::U256::from(estimate.max_priority_fee_per_gas + additional_tip),
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

        Ok(eth::GasPrice::new(
            suggested_max_fee_per_gas.into(),
            max_priority_fee_per_gas.into(),
            eth::U256::from(estimate.base_fee_per_gas).into(),
        ))
    }
}
