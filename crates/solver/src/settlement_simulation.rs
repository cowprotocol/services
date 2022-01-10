use crate::{encoding::EncodedSettlement, settlement::Settlement};
use anyhow::{Error, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    batch::CallBatch,
    contract::MethodBuilder,
    dyns::{DynMethodBuilder, DynTransport},
    errors::ExecutionError,
    transaction::TransactionBuilder,
    Account,
};
use futures::FutureExt;
use gas_estimation::EstimatedGasPrice;
use primitive_types::U256;
use shared::Web3;
use web3::types::{BlockId, CallRequest};

const SIMULATE_BATCH_SIZE: usize = 10;

/// The maximum amount the base gas fee can increase from one block to the other.
///
/// This is derived from [EIP-1559](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1559.md):
/// ```text
/// BASE_FEE_MAX_CHANGE_DENOMINATOR = 8
/// base_fee_per_gas_delta = max(parent_base_fee_per_gas * gas_used_delta // parent_gas_target // BASE_FEE_MAX_CHANGE_DENOMINATOR, 1)
/// ```
///
/// Because the elasticity factor is 2, this means that the highes possible `gas_used_delta == parent_gas_target`.
/// Therefore, the highest possible `base_fee_per_gas_delta` is `parent_base_fee_per_gas / 8`.
///
/// Example of this in action:
/// [Block 12998225](https://etherscan.io/block/12998225) with base fee of `43.353224173` and ~100% over the gas target.
/// Next [block 12998226](https://etherscan.io/block/12998226) has base fee of `48.771904644` which is an increase of ~12.5%.
const MAX_BASE_GAS_FEE_INCREASE: f64 = 1.125;

pub async fn simulate_and_estimate_gas_at_current_block(
    settlements: impl Iterator<Item = (Account, Settlement)>,
    contract: &GPv2Settlement,
    web3: &Web3,
    gas_price: EstimatedGasPrice,
) -> Result<Vec<Result<U256, ExecutionError>>> {
    // Collect into Vec to not rely on Itertools::chunk which would make this future !Send.
    let settlements: Vec<_> = settlements.collect();

    // Needed because sending an empty batch request gets an empty response which doesn't
    // deserialize correctly.
    if settlements.is_empty() {
        return Ok(Vec::new());
    }

    let web3 = web3::Web3::new(web3::transports::Batch::new(web3.transport().clone()));
    let mut results = Vec::new();
    for chunk in settlements.chunks(SIMULATE_BATCH_SIZE) {
        let calls = chunk
            .iter()
            .map(|(account, settlement)| {
                let tx = settle_method(gas_price, contract, settlement.clone(), account.clone()).tx;
                let resolved_gas_price = tx
                    .gas_price
                    .map(|gas_price| gas_price.resolve_for_transaction())
                    .unwrap_or_default();
                let call_request = CallRequest {
                    from: tx.from.map(|account| account.address()),
                    to: tx.to,
                    gas: None,
                    gas_price: resolved_gas_price.gas_price,
                    value: tx.value,
                    data: tx.data,
                    transaction_type: resolved_gas_price.transaction_type,
                    access_list: None,
                    max_fee_per_gas: resolved_gas_price.max_fee_per_gas,
                    max_priority_fee_per_gas: resolved_gas_price.max_priority_fee_per_gas,
                };
                web3.eth().estimate_gas(call_request, None)
            })
            .collect::<Vec<_>>();
        web3.transport().submit_batch().await?;
        for call in calls {
            results.push(call.await.map_err(ExecutionError::from));
        }
    }
    Ok(results)
}

#[allow(clippy::needless_collect)]
pub async fn simulate_and_error_with_tenderly_link(
    settlements: impl Iterator<Item = (Account, Settlement)>,
    contract: &GPv2Settlement,
    web3: &Web3,
    gas_price: EstimatedGasPrice,
    network_id: &str,
    block: u64,
) -> Vec<Result<()>> {
    let mut batch = CallBatch::new(web3.transport());
    let futures = settlements
        .map(|(account, settlement)| {
            let method = settle_method(gas_price, contract, settlement, account);
            let transaction_builder = method.tx.clone();
            let view = method
                .view()
                .block(BlockId::Number(block.into()))
                // Since we now supply the gas price for the simulation, make sure to also
                // set a gas limit so we don't get failed simulations because of insufficient
                // solver balance. Currently, a good trade off seems to be 7m,
                // as our biggest tx ever had only consumed 3M gas.
                .gas(7_000_000.into());
            (view.batch_call(&mut batch), transaction_builder)
        })
        .collect::<Vec<_>>();
    batch.execute_all(SIMULATE_BATCH_SIZE).await;

    futures
        .into_iter()
        .map(|(future, transaction_builder)| {
            future.now_or_never().unwrap().map(|_| ()).map_err(|err| {
                Error::new(err).context(tenderly_link(block, network_id, transaction_builder))
            })
        })
        .collect()
}

fn settle_method(
    gas_price: EstimatedGasPrice,
    contract: &GPv2Settlement,
    settlement: Settlement,
    account: Account,
) -> MethodBuilder<DynTransport, ()> {
    // Increase the gas price by the highest possible base gas fee increase. This
    // is done because the between retrieving the gas price and executing the simulation,
    // a block may have been mined that increases the base gas fee and causes the
    // `eth_call` simulation to fail with `max fee per gas less than block base fee`.
    let gas_price = gas_price.bump(MAX_BASE_GAS_FEE_INCREASE);
    let gas_price = if let Some(eip1559) = gas_price.eip1559 {
        (eip1559.max_fee_per_gas, eip1559.max_priority_fee_per_gas).into()
    } else {
        gas_price.legacy.into()
    };
    settle_method_builder(contract, settlement.into(), account).gas_price(gas_price)
}

pub fn settle_method_builder(
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
    from: Account,
) -> DynMethodBuilder<()> {
    contract
        .settle(
            settlement.tokens,
            settlement.clearing_prices,
            settlement.trades,
            settlement.interactions,
        )
        .from(from)
}

// Creates a simulation link in the gp-v2 tenderly workspace
pub fn tenderly_link(
    current_block: u64,
    network_id: &str,
    tx: TransactionBuilder<DynTransport>,
) -> String {
    // Tenderly simulates transactions for block N at transaction index 0, while
    // `eth_call` simulates transactions "on top" of the block (i.e. after the
    // last transaction index). Therefore, in order for the Tenderly simulation
    // to be as close as possible to the `eth_call`, we want to create it on the
    // next block (since `block_N{tx_last} ~= block_(N+1){tx_0}`).
    let next_block = current_block + 1;
    format!(
        "https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block={}&blockIndex=0&from={:#x}&gas=8000000&gasPrice=0&value=0&contractAddress={:#x}&network={}&rawFunctionInput=0x{}",
        next_block,
        tx.from.unwrap().address(),
        tx.to.unwrap(),
        network_id,
        hex::encode(tx.data.unwrap().0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::{Account, PrivateKey};
    use shared::transport::create_env_test_transport;

    // cargo test -p solver settlement_simulation::tests::mainnet -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        // Create some bogus settlements to see that the simulation returns an error.
        shared::tracing::initialize("solver=debug,shared=debug", tracing::Level::ERROR.into());
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let block = web3.eth().block_number().await.unwrap().as_u64();
        let network_id = web3.net().version().await.unwrap();
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        let settlements = vec![
            (
                account.clone(),
                Settlement::with_trades(Default::default(), vec![Default::default()]),
            ),
            (account.clone(), Settlement::new(Default::default())),
        ];
        let result = simulate_and_error_with_tenderly_link(
            settlements.iter().cloned(),
            &contract,
            &web3,
            Default::default(),
            network_id.as_str(),
            block,
        )
        .await;
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            &web3,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            std::iter::empty(),
            &contract,
            &web3,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    // cargo test -p solver settlement_simulation::tests::mainnet_chunked -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_chunked() {
        shared::tracing::initialize("solver=debug,shared=debug", tracing::Level::ERROR.into());
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        // 12 so that we hit more than one chunk.
        let settlements =
            vec![(account.clone(), Settlement::new(Default::default())); SIMULATE_BATCH_SIZE + 2];
        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            &web3,
            EstimatedGasPrice {
                legacy: 0.0,
                eip1559: None,
            },
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }
}
