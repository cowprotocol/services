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
        // use some reasonable initial value. The exact value doesn't matter much since the background
        // task will update the gas price immediately anyway since the block stream yields
        // the current block immediately
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
                        tracing::warn!(?err, "failed to compute gas price");
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_provider::{Provider, ProviderBuilder, mock::Asserter},
        ethrpc::block_stream::{BlockInfo, mock_single_block},
        tokio::{sync::watch, time::Duration},
    };

    fn mock_provider(asserter: Asserter) -> ethrpc::AlloyProvider {
        ProviderBuilder::new()
            .connect_mocked_client(asserter)
            .erased()
    }

    fn default_config() -> EstimatorConfig {
        EstimatorConfig {
            past_blocks: 5,
            reward_percentile: 50.0,
        }
    }

    fn block_with_base_fee(base_fee: u64) -> BlockInfo {
        BlockInfo {
            base_fee,
            ..Default::default()
        }
    }

    // Pushes a fee_history JSON response where `latest_block_base_fee()` returns
    // `base_fee` and the reward is `tip`. The resulting estimate will be:
    //   max_priority_fee_per_gas = tip
    //   max_fee_per_gas          = base_fee * 2 + tip
    fn push_fee_history(asserter: &Asserter, base_fee: u128, tip: u128) {
        asserter.push_success(&serde_json::json!({
            // baseFeePerGas[len-2] is the latest block base fee,
            // baseFeePerGas[len-1] is the projected next block base fee
            "baseFeePerGas": [format!("0x{base_fee:x}"), format!("0x{:x}", base_fee * 2)],
            "gasUsedRatio": [0.5],
            "oldestBlock": "0x1",
            "reward": [[format!("0x{tip:x}")]]
        }));
    }

    #[tokio::test]
    async fn base_fee_reads_from_block_watcher_not_rpc() {
        let base_fee: u64 = 5_000_000_000;
        let current_block = mock_single_block(block_with_base_fee(base_fee));
        let asserter = Asserter::new();
        // Queue one response for the background task's initial fee history fetch.
        push_fee_history(&asserter, base_fee as u128, 1_000_000_000);
        let provider = mock_provider(asserter);

        let estimator =
            ConfigurableGasPriceEstimator::new(provider, default_config(), current_block);

        // base_fee() reads from the block watcher directly — no RPC call needed.
        let result = estimator.base_fee().await.unwrap();
        assert_eq!(result, Some(base_fee));
    }

    #[tokio::test]
    async fn estimate_caches_result_between_blocks() {
        let base_fee: u128 = 10_000_000_000;
        let tip: u128 = 1_000_000_000;

        // Keep the sender alive so the background task does not terminate.
        let (_block_sender, block_receiver) = watch::channel(block_with_base_fee(1));
        let asserter = Asserter::new();
        // Only one response is queued — the estimator must not call RPC more than once.
        push_fee_history(&asserter, base_fee, tip);
        let provider = mock_provider(asserter.clone());

        let estimator =
            ConfigurableGasPriceEstimator::new(provider, default_config(), block_receiver);

        // Give the background task time to process the initial block.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let first = estimator.estimate().await.unwrap();
        let second = estimator.estimate().await.unwrap();

        assert_eq!(first, second, "estimate should return the same cached value");
        assert!(
            asserter.read_q().is_empty(),
            "no additional RPC calls should have been made between estimate() calls"
        );
    }

    #[tokio::test]
    async fn estimate_updates_when_new_block_arrives() {
        let base_fee_1: u128 = 10_000_000_000;
        let base_fee_2: u128 = 20_000_000_000;
        let tip: u128 = 1_000_000_000;

        let (block_sender, block_receiver) = watch::channel(block_with_base_fee(1));
        let asserter = Asserter::new();
        push_fee_history(&asserter, base_fee_1, tip);
        let provider = mock_provider(asserter.clone());

        let estimator =
            ConfigurableGasPriceEstimator::new(provider, default_config(), block_receiver);

        tokio::time::sleep(Duration::from_millis(10)).await;

        let estimate_after_block_1 = estimator.estimate().await.unwrap();
        assert_eq!(
            estimate_after_block_1.max_fee_per_gas,
            base_fee_1 * 2 + tip,
            "max_fee after block 1"
        );
        assert_eq!(
            estimate_after_block_1.max_priority_fee_per_gas,
            tip,
            "max_priority after block 1"
        );

        // Queue the response for the second block and trigger the update.
        push_fee_history(&asserter, base_fee_2, tip);
        block_sender.send(block_with_base_fee(2)).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        let estimate_after_block_2 = estimator.estimate().await.unwrap();
        assert_eq!(
            estimate_after_block_2.max_fee_per_gas,
            base_fee_2 * 2 + tip,
            "max_fee after block 2"
        );
        assert!(
            asserter.read_q().is_empty(),
            "both fee_history responses should have been consumed"
        );
    }
}

