//! https://docs.edennetwork.io/for-traders/getting-started

use crate::{
    settlement::{Revertable, Settlement},
    settlement_submission::{
        submitter::{
            common::PrivateNetwork, AdditionalTip, Strategy, SubmissionLoopStatus,
            TransactionHandle, TransactionSubmitting,
        },
        GlobalTxPool,
    },
};

use anyhow::{bail, Context, Result};
use ethcontract::{
    transaction::{Transaction, TransactionBuilder},
    H160, H256, U256,
};
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::types::Value;
use reqwest::{Client, IntoUrl, Url};
use serde::Deserialize;
use shared::{transport::http::HttpTransport, Web3, Web3Transport};
use web3::{helpers, types::Bytes};

const ALREADY_KNOWN_TRANSACTION: &[&str] = &[
    "already known",
    "nonce too low",
    "replacement transaction underpriced",
];

#[derive(Clone)]
pub struct EdenApi {
    client: Client,
    url: Url,
    rpc: Web3,
    global_tx_pool: GlobalTxPool,
}

#[derive(Debug, Clone, Deserialize)]
struct EdenSuccess {
    result: H256,
}

#[derive(Debug, Clone, Deserialize)]
struct MultipleEdenSuccesses {
    result: Vec<EdenSuccess>,
}

fn biggest_public_nonce(global_tx_pool: &GlobalTxPool, address: H160) -> Option<U256> {
    let pools = global_tx_pool.pools.lock().unwrap();
    pools
        .iter()
        .filter(|sub_pool| matches!(sub_pool.strategy, Strategy::CustomNodes))
        .flat_map(|sub_pool| sub_pool.pools.keys())
        .filter(|(sender, _)| *sender == address)
        .map(|(_, nonce)| *nonce)
        .max()
}

impl EdenApi {
    pub fn new(client: Client, url: impl IntoUrl, global_tx_pool: GlobalTxPool) -> Result<Self> {
        let url = url.into_url().context("bad eden url")?;
        let transport = Web3Transport::new(HttpTransport::new(
            client.clone(),
            url.clone(),
            "eden".to_owned(),
        ));
        let rpc = Web3::new(transport);

        Ok(Self {
            client,
            url,
            rpc,
            global_tx_pool,
        })
    }

    // When using `eth_sendSlotTxs` method, we must use native Client because the response for this method
    // is a non-standard json that can't be automatically deserialized when `Transport` is used.
    async fn submit_slot_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        let (raw_signed_transaction, tx_hash) = match tx.build().now_or_never().unwrap().unwrap() {
            Transaction::Request(_) => unreachable!("verified offline account was used"),
            Transaction::Raw { bytes, hash } => (bytes.0, hash),
        };
        let params =
            serde_json::to_value(Bytes(raw_signed_transaction)).context("failed to serialize")?;
        let request =
            helpers::build_request(1, "eth_sendSlotTxs", vec![Value::Array(vec![params])]);
        tracing::debug!(?request, "sending Eden API request");

        let response = self
            .client
            .post(self.url.clone())
            .json(&request)
            .send()
            .await
            .context("failed sending request")?
            .text()
            .await
            .context("failed converting to text")?;
        tracing::debug!(%response, "response from eden");
        let response = serde_json::from_str::<MultipleEdenSuccesses>(&response)
            .context("failed to deserialize")?;
        let handle = response
            .result
            .first()
            .context("response did not contain a result")?
            .result;

        Ok(TransactionHandle { tx_hash, handle })
    }

    fn track_submission_success(
        &self,
        result: &Result<TransactionHandle>,
        sender: H160,
        nonce: U256,
        tx_hash: H256,
    ) {
        match result {
            Ok(_) => {
                super::track_submission_success("eden", true);
            }
            Err(e) => {
                let error = e.to_string();

                // The eden mempool also contains tx from the public mempool. If our in-memory
                // tx pool already knows that we successfully submitted this or a more recent tx
                // to the public mempool we discard those "already known" errors as false positives.
                let supposedly_already_known = ALREADY_KNOWN_TRANSACTION
                    .iter()
                    .any(|message| error.contains(message));

                let publicly_submitted_bigger_nonce =
                    biggest_public_nonce(&self.global_tx_pool, sender).unwrap_or_default() >= nonce;

                if supposedly_already_known && publicly_submitted_bigger_nonce {
                    tracing::debug!(?tx_hash, "transaction already known");
                } else {
                    tracing::warn!(?error, "transaction submission error");
                    super::track_submission_success("eden", false);
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for EdenApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        let sender = tx.from.as_ref().expect("sender has to be set").address();
        let nonce = *tx.nonce.as_ref().expect("nonce has to be set");

        let tx_hash = match tx.clone().build().now_or_never() {
            Some(Ok(Transaction::Raw { hash, .. })) => hash,
            _ => bail!("Eden submission requires fully built raw transactions"),
        };

        // try to submit with slot method
        let result = self
            .submit_slot_transaction(tx.clone())
            .or_else(|err| async move {
                // fallback to standard `eth_sendRawTransaction` if `eth_sendSlotTx` fails
                // which can happens when we don't have a slot.
                tracing::debug!(?err, "fallback to eth_sendRawTransaction");
                self.rpc
                    .api::<PrivateNetwork>()
                    .submit_raw_transaction(tx)
                    .await
            })
            .await as Result<TransactionHandle>;

        self.track_submission_success(&result, sender, nonce, tx_hash);

        result
    }

    async fn cancel_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        self.rpc
            .api::<PrivateNetwork>()
            .submit_raw_transaction(tx)
            .await
    }

    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus {
        // disable strategy if there is a high possibility for a transaction to be reverted (check done only for mainnet)
        if shared::gas_price_estimation::is_mainnet(network_id) {
            if let Revertable::NoRisk = settlement.revertable() {
                return SubmissionLoopStatus::Enabled(AdditionalTip::Off);
            }
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::On)
    }

    fn name(&self) -> Strategy {
        Strategy::Eden
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn eden_success_response() {
        let response = serde_json::json!({
            "code": 200i64,
            "result": "0x41df922fd0d4766fcc02e161f8295ec28522f329ae487f14d811e4b64c8d6e31",
        });
        let deserialized = serde_json::from_value::<EdenSuccess>(response).unwrap();
        assert_eq!(
            deserialized.result,
            H256::from_str("41df922fd0d4766fcc02e161f8295ec28522f329ae487f14d811e4b64c8d6e31")
                .unwrap()
        );
    }

    #[test]
    fn deserializes_send_slot_txs_response() {
        // based on these docs: https://docs.edennetwork.io/for-traders/eden-relay/slot-transactions#example-response
        let response = serde_json::json!({
            "result": [{
                "code": 200i64,
                "result": "0x41df922fd0d4766fcc02e161f8295ec28522f329ae487f14d811e4b64c8d6e31",
            }]
        });
        let deserialized = serde_json::from_value::<MultipleEdenSuccesses>(response).unwrap();
        assert_eq!(
            deserialized.result[0].result,
            H256::from_str("41df922fd0d4766fcc02e161f8295ec28522f329ae487f14d811e4b64c8d6e31")
                .unwrap()
        );
    }

    #[test]
    fn finds_biggest_publicly_submitted_nonce_for_sender() {
        let sender = H160([1; 20]);
        let global = GlobalTxPool::default();
        let pub_pool = global.add_sub_pool(Strategy::CustomNodes);
        let flashbots_pool = global.add_sub_pool(Strategy::Flashbots);
        let eden_pool = global.add_sub_pool(Strategy::Eden);

        pub_pool.update(sender, 1.into(), vec![]);
        pub_pool.update(sender, 2.into(), vec![]);

        // is public but has the wrong sender
        pub_pool.update(H160([2; 20]), 1_000.into(), vec![]);

        // right sender but not in public mempool
        flashbots_pool.update(sender, 1_000.into(), vec![]);
        eden_pool.update(sender, 1_000.into(), vec![]);

        assert_eq!(Some(2.into()), biggest_public_nonce(&global, sender));
    }
}
