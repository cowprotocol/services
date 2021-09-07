use anyhow::{Context, Result};
use ethcontract::transport::DynTransport;
use primitive_types::{H160, U256};
use serde::Deserialize;
use std::collections::HashMap;
use web3::Transport;

#[derive(Debug, Deserialize)]
struct Block {
    transactions: Vec<Transaction>,
}

#[derive(Debug, Deserialize)]
struct TxPool {
    pending: HashMap<String, HashMap<String, Transaction>>,
    queued: HashMap<String, HashMap<String, Transaction>>,
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

// Get pending transactions from a mempool.
pub async fn pending_transactions(transport: &DynTransport) -> Result<Vec<Transaction>> {
    match transport
        .execute("txpool_content", Default::default())
        .await
    {
        Ok(response) => {
            let txpool: TxPool = serde_json::from_value(response).context("deserialize failed")?;

            let transactions = txpool
                .pending
                .into_iter()
                .chain(txpool.queued)
                .collect::<HashMap<String, HashMap<String, Transaction>>>();

            Ok(transactions
                .into_values()
                .flatten()
                .map(|value| value.1)
                .collect())
        }
        Err(_) => {
            //fallback to eth_getBlockByNumber
            let params = vec!["pending".into(), true.into()];
            let response = transport
                .execute("eth_getBlockByNumber", params)
                .await
                .context("transport failed")?;
            let block: Block = serde_json::from_value(response).context("deserialize failed")?;

            Ok(block.transactions)
        }
    }
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
    fn deserialize_txpool() {
        let json = json!({
          "pending": {
            "0x0216d5032f356960cd3749c31ab34eeff21b3395": {
              "806": {
                "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "blockNumber": "null",
                "from": "0x0216d5032f356960cd3749c31ab34eeff21b3395",
                "gas": "0x5208",
                "gasPrice": "0xba43b7400",
                "hash": "0xaf953a2d01f55cfe080c0c94150a60105e8ac3d51153058a1f03dd239dd08586",
                "input": "0x",
                "nonce": "0x326",
                "to": "0x7f69a91a3cf4be60020fb58b893b7cbb65376db8",
                "transactionIndex": "null",
                "value": "0x19a99f0cf456000"
              }
            },
            "0x24d407e5a0b506e1cb2fae163100b5de01f5193c": {
              "34": {
                "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "blockNumber": "null",
                "from": "0x24d407e5a0b506e1cb2fae163100b5de01f5193c",
                "gas": "0x44c72",
                "gasPrice": "0x4a817c800",
                "hash": "0xb5b8b853af32226755a65ba0602f7ed0e8be2211516153b75e9ed640a7d359fe",
                "input": "0xb61d27f600000000000000000000000024d407e5a0b506e1cb2fae163100b5de01f5193c00000000000000000000000000000000000000000000000053444835ec580000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "nonce": "0x22",
                "to": "0x7320785200f74861b69c49e4ab32399a71b34f1a",
                "transactionIndex": "null",
                "value": "0x0"
              }
            }
          },
          "queued": {
            "0x976a3fc5d6f7d259ebfb4cc2ae75115475e9867c": {
              "3": {
                "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "blockNumber": "null",
                "from": "0x976a3fc5d6f7d259ebfb4cc2ae75115475e9867c",
                "gas": "0x15f90",
                "gasPrice": "0x4a817c800",
                "hash": "0x57b30c59fc39a50e1cba90e3099286dfa5aaf60294a629240b5bbec6e2e66576",
                "input": "0x",
                "nonce": "0x3",
                "to": "0x346fb27de7e7370008f5da379f74dd49f5f2f80f",
                "transactionIndex": "null",
                "value": "0x1f161421c8e0000"
              }
            },
            "0x9b11bf0459b0c4b2f87f8cebca4cfc26f294b63a": {
              "2": {
                "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "blockNumber": "null",
                "from": "0x9b11bf0459b0c4b2f87f8cebca4cfc26f294b63a",
                "gas": "0x15f90",
                "gasPrice": "0xba43b7400",
                "hash": "0x3a3c0698552eec2455ed3190eac3996feccc806970a4a056106deaf6ceb1e5e3",
                "input": "0x",
                "nonce": "0x2",
                "to": "0x24a461f25ee6a318bdef7f33de634a67bb67ac9d",
                "transactionIndex": "null",
                "value": "0xebec21ee1da40000"
              },
              "6": {
                "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "blockNumber": "null",
                "from": "0x9b11bf0459b0c4b2f87f8cebca4cfc26f294b63a",
                "gas": "0x15f90",
                "gasPrice": "0x4a817c800",
                "hash": "0xbbcd1e45eae3b859203a04be7d6e1d7b03b222ec1d66dfcc8011dd39794b147e",
                "input": "0x",
                "nonce": "0x6",
                "to": "0x6368f3f8c2b42435d6c136757382e4a59436a681",
                "transactionIndex": "null",
                "value": "0xf9a951af55470000"
              }
            }
          }
        });

        let txpool: TxPool = serde_json::from_value(json).unwrap();

        assert_eq!(txpool.pending.len(), 2);
        assert_eq!(txpool.queued.len(), 2);
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
