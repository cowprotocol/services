//! Configurable EIP-1559 gas price estimator.
//!
//! Unlike alloy's default estimator which uses hardcoded values (10 blocks,
//! 20th percentile), this estimator allows configuring:
//! - Number of blocks to look back
//! - Reward percentile to use

use {
    crate::GasPriceEstimating,
    alloy_eips::{BlockNumberOrTag, eip1559::Eip1559Estimation},
    alloy_provider::{Provider, utils::eip1559_default_estimator},
    anyhow::{Context, Result},
    configs::gas_price_estimation::EstimatorConfig,
    ethrpc::{AlloyProvider, block_stream::CurrentBlockWatcher},
    futures::StreamExt as _,
    tokio::sync::watch,
    tracing::instrument,
};

/// A configurable EIP-1559 gas price estimator.
///
/// Uses alloy's default estimation algorithm but with configurable
/// `past_blocks` and `reward_percentile` parameters for the fee history query.
/// This component uses a background task to update the gas price once every
/// block to avoid multiple RPC requests for the same block.
pub struct ConfigurableGasPriceEstimator {
    current_block: CurrentBlockWatcher,
    lastest_gas_price: watch::Receiver<Eip1559Estimation>,
}

impl ConfigurableGasPriceEstimator {
    pub fn new(
        provider: AlloyProvider,
        config: EstimatorConfig,
        current_block: CurrentBlockWatcher,
    ) -> Self {
        // use some reasonable initial value. The exact value doesn't matter much since
        // the background task will update the gas price immediately anyway
        // since the block stream yields the current block immediately
        let base_fee = current_block.borrow().base_fee;
        let init = Eip1559Estimation {
            max_fee_per_gas: u128::from(base_fee * 2),
            max_priority_fee_per_gas: u128::from(base_fee * 2),
        };

        let (sender, receiver) = watch::channel(init);
        let mut current_block_stream = ethrpc::block_stream::into_stream(current_block.clone());
        tokio::task::spawn(async move {
            while current_block_stream.next().await.is_some() {
                let gas_price = match current_gas_price(&provider, &config).await {
                    Ok(gas_price) => gas_price,
                    Err(err) => {
                        tracing::warn!(?err, "failed to computed gas price");
                        continue;
                    }
                };
                if let Err(err) = sender.send(gas_price) {
                    // at this point all [`watch::Receiver`] have been dropped and new ones
                    // could only be created with [`watch::Sender::subscribe()`] but the only
                    // Sender instance lives in this background task and can not be accessed
                    // anymore
                    tracing::debug!(
                        ?err,
                        "all gas price estimators dropped, terminating update loop"
                    );
                    break;
                }
            }
            tracing::debug!("terminating gas price update task");
        });

        Self {
            current_block,
            lastest_gas_price: receiver,
        }
    }
}

async fn current_gas_price(
    provider: &AlloyProvider,
    config: &EstimatorConfig,
) -> Result<Eip1559Estimation> {
    // Fetch fee history with our configured parameters
    let fee_history = provider
        .get_fee_history(
            config.past_blocks,
            BlockNumberOrTag::Latest,
            &[config.reward_percentile],
        )
        .await
        .context("failed to fetch fee history")?;

    // Get base fee: use latest block's base fee, or fall back to fetching
    // latest block directly if fee history is empty
    let base_fee_per_gas = match fee_history.latest_block_base_fee() {
        Some(base_fee) if base_fee != 0 => base_fee,
        _ => {
            // empty response, fetch basefee from latest block directly
            let block = provider
                .get_block_by_number(BlockNumberOrTag::Latest)
                .await
                .context("failed to fetch latest block")?
                .context("latest block not found")?;
            u128::from(
                block
                    .header
                    .base_fee_per_gas
                    .context("base_fee_per_gas not available (eip1559 not supported)")?,
            )
        }
    };

    // Use alloy's default estimation algorithm
    let estimation =
        eip1559_default_estimator(base_fee_per_gas, &fee_history.reward.unwrap_or_default());

    Ok(Eip1559Estimation {
        max_fee_per_gas: estimation.max_fee_per_gas,
        max_priority_fee_per_gas: estimation.max_priority_fee_per_gas,
    })
}

#[async_trait::async_trait]
impl GasPriceEstimating for ConfigurableGasPriceEstimator {
    async fn base_fee(&self) -> Result<Option<u64>> {
        Ok(Some(self.current_block.borrow().base_fee))
    }

    #[instrument(skip_all)]
    async fn estimate(&self) -> Result<Eip1559Estimation> {
        Ok(*self.lastest_gas_price.borrow())
    }
}
