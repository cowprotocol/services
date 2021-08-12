use crate::encoding::EncodedSettlement;
use anyhow::{Error, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    batch::CallBatch, dyns::DynTransport, transaction::TransactionBuilder, GasPrice,
};
use futures::FutureExt;
use primitive_types::U256;
use shared::Web3;
use web3::types::{BlockId, BlockNumber};

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

pub enum Block {
    // Simulate the transactions at this block and attach a tenderly link to errors.
    FixedWithTenderly(u64),
    // Simulate the transactions at the latest block and do not attach a tenderly link.
    LatestWithoutTenderly,
}

/// Simulate the settlement using a web3 `call`.
// Clippy claims we don't need to collect `futures` but we do or the lifetimes with `join!` don't
// work out.
#[allow(clippy::needless_collect)]
pub async fn simulate_settlements(
    settlements: impl Iterator<Item = EncodedSettlement>,
    contract: &GPv2Settlement,
    web3: &Web3,
    network_id: &str,
    block: Block,
    gas_price: f64,
) -> Result<Vec<Result<()>>> {
    let mut batch = CallBatch::new(web3.transport());
    let futures = settlements
        .map(|settlement| {
            // Increase the gas price by the highest possible base gas fee increase. This
            // is done because the between retrieving the gas price and executing the simulation,
            // a block may have been mined that increases the base gas fee and causes the
            // `eth_call` simulation to fail with `max fee per gas less than block base fee`.
            let gas_price =
                GasPrice::Value(U256::from_f64_lossy(gas_price * MAX_BASE_GAS_FEE_INCREASE));
            let method =
                crate::settlement_submission::retry::settle_method_builder(contract, settlement)
                    .gas_price(gas_price);
            let transaction_builder = method.tx.clone();
            let view = method
                .view()
                .block(match block {
                    Block::FixedWithTenderly(block) => BlockId::Number(block.into()),
                    Block::LatestWithoutTenderly => BlockId::Number(BlockNumber::Latest),
                })
                // Since we now supply the gas price for the simulation, make sure to also
                // set a gas limit so we don't get failed simulations because of insufficient
                // solver balance for the default ~150M gas limit. Limit to around the
                // block gas limit (since we can't fit more anyway).
                .gas(15_000_000.into());
            (view.batch_call(&mut batch), transaction_builder)
        })
        .collect::<Vec<_>>();
    batch.execute_all(SIMULATE_BATCH_SIZE).await;

    Ok(futures
        .into_iter()
        .map(|(future, transaction_builder)| {
            future.now_or_never().unwrap().map(|_| ()).map_err(|err| {
                let err = Error::new(err);
                match block {
                    Block::FixedWithTenderly(block) => {
                        err.context(tenderly_link(block, network_id, transaction_builder))
                    }
                    Block::LatestWithoutTenderly => err,
                }
            })
        })
        .collect())
}

// Creates a simulation link in the gp-v2 tenderly workspace
pub fn tenderly_link(
    current_block: u64,
    network_id: &str,
    tx: TransactionBuilder<DynTransport>,
) -> String {
    format!(
        "https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block={}&blockIndex=0&from={:#x}&gas=8000000&gasPrice=0&value=0&contractAddress={:#x}&network={}&rawFunctionInput=0x{}",
        current_block,
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
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let block = web3.eth().block_number().await.unwrap().as_u64();
        let network_id = web3.net().version().await.unwrap();
        let mut contract = GPv2Settlement::deployed(&web3).await.unwrap();
        contract.defaults_mut().from = Some(Account::Offline(
            PrivateKey::from_raw([1; 32]).unwrap(),
            None,
        ));
        let settlements = vec![
            EncodedSettlement {
                tokens: Default::default(),
                clearing_prices: Default::default(),
                trades: vec![crate::encoding::encode_trade(
                    &Default::default(),
                    0,
                    0,
                    &0.into(),
                )],
                interactions: Default::default(),
            },
            EncodedSettlement::default(),
        ];
        let result = simulate_settlements(
            settlements.into_iter(),
            &contract,
            &web3,
            network_id.as_str(),
            Block::FixedWithTenderly(block),
            0.0,
        )
        .await;
        let _ = dbg!(result);
    }
}
