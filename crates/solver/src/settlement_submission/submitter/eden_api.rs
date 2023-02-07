//! https://docs.edennetwork.io/for-traders/getting-started

use {
    crate::{
        settlement::{Revertable, Settlement},
        settlement_submission::submitter::{
            common::PrivateNetwork,
            AdditionalTip,
            Strategy,
            SubmissionLoopStatus,
            TransactionHandle,
            TransactionSubmitting,
        },
    },
    anyhow::{bail, Context, Result},
    ethcontract::{
        transaction::{Transaction, TransactionBuilder},
        H256,
    },
    futures::{FutureExt, TryFutureExt},
    jsonrpc_core::types::Value,
    reqwest::{Client, IntoUrl, Url},
    serde::Deserialize,
    shared::ethrpc::{http::HttpTransport, Web3, Web3Transport},
    web3::{helpers, types::Bytes},
};

#[derive(Clone)]
pub struct EdenApi {
    client: Client,
    url: Url,
    rpc: Web3,
}

#[derive(Debug, Clone, Deserialize)]
struct EdenSuccess {
    result: H256,
}

#[derive(Debug, Clone, Deserialize)]
struct MultipleEdenSuccesses {
    result: Vec<EdenSuccess>,
}

impl EdenApi {
    pub fn new(client: Client, url: impl IntoUrl) -> Result<Self> {
        let url = url.into_url().context("bad eden url")?;
        let transport = Web3Transport::new(HttpTransport::new(
            client.clone(),
            url.clone(),
            "eden".to_owned(),
        ));
        let rpc = Web3::new(transport);

        Ok(Self { client, url, rpc })
    }

    // When using `eth_sendSlotTxs` method, we must use native Client because the
    // response for this method is a non-standard json that can't be
    // automatically deserialized when `Transport` is used.
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
}

#[async_trait::async_trait]
impl TransactionSubmitting for EdenApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        match tx.clone().build().now_or_never() {
            Some(Ok(Transaction::Raw { .. })) => (),
            _ => bail!("Eden submission requires fully built raw transactions"),
        };

        // try to submit with slot method
        self.submit_slot_transaction(tx.clone())
            .or_else(|err| async move {
                // fallback to standard `eth_sendRawTransaction` if `eth_sendSlotTx` fails
                // which can happens when we don't have a slot.
                tracing::debug!(?err, "fallback to eth_sendRawTransaction");
                self.rpc
                    .api::<PrivateNetwork>()
                    .submit_raw_transaction(tx)
                    .await
            })
            .await as Result<TransactionHandle>
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
        // disable strategy if there is a high possibility for a transaction to be
        // reverted (check done only for mainnet)
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
    use {super::*, std::str::FromStr};

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
}
