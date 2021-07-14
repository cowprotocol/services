use anyhow::{Context, Result};
use ethcontract::transport::DynTransport;
use primitive_types::{H160, U256};
use serde::Deserialize;
use web3::Transport;

#[derive(Debug, Deserialize)]
struct Block {
    transactions: Vec<Transaction>,
}

// Not using rust-web3's transaction struct because it doesn't handle EIP-1559. Only contains the
// fields we are currently using.
// https://github.com/ethereum/eth1.0-apis
#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub from: H160,
    pub nonce: U256,
    #[serde(flatten)]
    pub fee: Fee,
}

#[derive(Debug, Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum Fee {
    #[serde(rename_all = "camelCase")]
    Legacy { gas_price: U256 },
    #[serde(rename_all = "camelCase")]
    Eip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
    },
}

// Get the pending transactions from a node.
//
// This works better than rust-web3's `block_with_txs` because some nodes set the `miner` field to
// `null`. This does not follow the ethrpc specification and fails deserialization in rust-web3.
pub async fn pending_transactions(transport: &DynTransport) -> Result<Vec<Transaction>> {
    let params = vec!["pending".into(), true.into()];
    let response = transport
        .execute("eth_getBlockByNumber", params)
        .await
        .context("transport failed")?;
    let block: Block = serde_json::from_value(response).context("deserialize failed")?;
    Ok(block.transactions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // cargo test -p solver pending_transactions::tests::real_node -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn real_node() {
        let transport = DynTransport::new(
            web3::transports::Http::new(&std::env::var("NODE_URL").unwrap()).unwrap(),
        );
        let transactions = pending_transactions(&transport).await.unwrap();
        dbg!(transactions.as_slice());
        assert!(!transactions.is_empty());
    }

    #[test]
    fn deserialize_eip_1559_fee() {
        let json = json!({
            "from": "0x0000000000000000000000000000000000000001",
            "nonce": "0x02",
            "maxFeePerGas": "0x03",
            "maxPriorityFeePerGas": "0x04",
        });
        let transaction: Transaction = serde_json::from_value(json).unwrap();
        assert_eq!(transaction.from, H160::from_low_u64_be(1));
        assert_eq!(transaction.nonce, 2.into());
        match transaction.fee {
            Fee::Legacy { .. } => unreachable!(),
            Fee::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            } => {
                assert_eq!(max_fee_per_gas, 3.into());
                assert_eq!(max_priority_fee_per_gas, 4.into());
            }
        };
    }

    #[test]
    fn deserialize_legacy_fee() {
        let json = json!({
            "from": "0x0000000000000000000000000000000000000001",
            "nonce": "0x02",
            "gasPrice": "0x03",
        });
        let transaction: Transaction = serde_json::from_value(json).unwrap();
        assert_eq!(transaction.from, H160::from_low_u64_be(1));
        assert_eq!(transaction.nonce, 2.into());
        match transaction.fee {
            Fee::Legacy { gas_price } => assert_eq!(gas_price, 3.into()),
            Fee::Eip1559 { .. } => unreachable!(),
        };
    }
}
