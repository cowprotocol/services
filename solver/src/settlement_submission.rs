mod gas_price_stream;
mod retry;

use self::retry::{CancelSender, SettlementSender};
use crate::settlement::Settlement;
use anyhow::{anyhow, Context, Result};
use contracts::GPv2Settlement;
use gas_estimation::GasPriceEstimating;
use gas_price_stream::gas_price_stream;
use primitive_types::{H160, U256};
use std::time::Duration;
use transaction_retry::RetryResult;

const MAX_GAS: u32 = 8_000_000;
const GAS_PRICE_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

// Submit a settlement to the contract, updating the transaction with gas prices if they increase.
pub async fn submit(
    contract: &GPv2Settlement,
    gas: &dyn GasPriceEstimating,
    target_confirm_time: Duration,
    gas_price_cap: f64,
    settlement: Settlement,
) -> Result<()> {
    let nonce = transaction_count(contract)
        .await
        .context("failed to get transaction_count")?;
    let settlement = encode_settlement(&settlement)?;
    // Check that a simulation of the transaction works before submitting it.
    simulate_settlement(&settlement, contract).await?;

    let settlement_sender = SettlementSender {
        contract,
        nonce,
        settlement,
    };
    // We never cancel.
    let cancel_future = std::future::pending::<CancelSender>();
    let stream = gas_price_stream(target_confirm_time, gas_price_cap, gas);

    match transaction_retry::retry(settlement_sender, cancel_future, stream).await {
        Some(RetryResult::Submitted(result)) => {
            tracing::info!("completed settlement submission");
            result.0.context("settlement transaction failed")
        }
        _ => unreachable!(),
    }
}

async fn transaction_count(contract: &GPv2Settlement) -> Result<U256> {
    let defaults = contract.defaults();
    let address = defaults.from.as_ref().unwrap().address();
    let web3 = contract.raw_instance().web3();
    let count = web3.eth().transaction_count(address, None).await?;
    Ok(count)
}

#[derive(Clone)]
pub struct EncodedSettlement {
    tokens: Vec<H160>,
    clearing_prices: Vec<U256>,
    encoded_trades: Vec<u8>,
    encoded_interactions: Vec<u8>,
}

fn encode_settlement(settlement: &Settlement) -> Result<EncodedSettlement> {
    Ok(EncodedSettlement {
        tokens: settlement.tokens(),
        clearing_prices: settlement.clearing_prices(),
        encoded_interactions: settlement
            .encode_interactions()
            .context("interaction encoding failed")?,
        encoded_trades: settlement
            .encode_trades()
            .ok_or_else(|| anyhow!("trade encoding failed"))?,
    })
}

// Simulate the settlement using a web3 `call`.
async fn simulate_settlement(
    settlement: &EncodedSettlement,
    contract: &GPv2Settlement,
) -> Result<()> {
    let method = retry::settle_method_builder(contract, settlement.clone());
    let data = method.tx.data.as_ref().expect("no data").0.as_slice();
    tracing::info!("Settlement call: {}", hex::encode(data));
    method.call().await.context("settle simulation failed")?;
    Ok(())
}
