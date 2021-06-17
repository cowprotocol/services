use anyhow::{Context, Result};
use ethcontract::transport::DynTransport;
use serde::Deserialize;
use web3::{types::Transaction, Transport};

#[derive(Deserialize)]
struct Block {
    transactions: Vec<Transaction>,
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

    // cargo test -p solver pending_transactions::tests::mainnet -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        let transport = DynTransport::new(
            web3::transports::Http::new(&std::env::var("NODE_URL").unwrap()).unwrap(),
        );
        let transactions = pending_transactions(&transport).await.unwrap();
        dbg!(transactions.as_slice());
        assert!(!transactions.is_empty());
    }
}
