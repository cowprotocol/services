//! Direct submission of settlements to block builders.
//!
//! Builders neither agree on the shape of a successful
//! `eth_sendRawTransaction` response nor on whether they authenticate their
//! callers, so the requests are built by hand instead of through an RPC client.

use {
    crate::infra::{observe::metrics, solver::Account},
    alloy::{
        consensus::TxEnvelope,
        eips::eip2718::Encodable2718,
        hex,
        network::TxSigner,
        primitives::{Address, eip191_hash_message, keccak256},
    },
    anyhow::{Context, anyhow},
    bytes::Bytes,
    eth_domain_types as eth,
    futures::future::join_all,
    std::collections::HashMap,
    url::Url,
};

/// A builder that stops answering must not hold up the whole broadcast:
/// submission is on the critical path of the block.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// A block builder that accepts settlement transactions directly.
#[derive(Debug, Clone)]
pub struct Builder {
    pub name: String,
    pub url: Url,
}

/// Broadcasts settlements to all the block builders of one mempool.
#[derive(Debug, Clone)]
pub struct Builders {
    /// Name of the mempool these builders belong to, for logs and metrics.
    mempool: String,
    builders: Vec<Builder>,
    http: reqwest::Client,
    /// The accounts that sign the settlements, by address. Also used to sign
    /// the requests for builders that authenticate their callers.
    signers: HashMap<Address, Account>,
}

impl Builders {
    pub fn new(
        mempool: String,
        builders: Vec<Builder>,
        signers: HashMap<Address, Account>,
    ) -> Self {
        Self {
            mempool,
            builders,
            http: reqwest::ClientBuilder::new()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build the builder http client"),
            signers,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.builders.is_empty()
    }

    /// Broadcasts an already signed settlement to every builder. Succeeds as
    /// long as one builder accepted it.
    pub async fn broadcast(
        &self,
        envelope: &TxEnvelope,
        signer: eth::Address,
    ) -> anyhow::Result<eth::TxId> {
        let hash = eth::TxId(*envelope.tx_hash());
        // Every builder receives the exact same bytes, so one body and one
        // request signature are enough.
        let body = build_body(envelope)?;
        let signature = self.sign_request(signer, &body).await?;

        let accepted = join_all(self.builders.iter().map(|builder| {
            let body = body.clone();
            let signature = signature.as_str();
            async move {
                let result = self.post(builder, body, signature).await;
                self.observe_result(builder, &hash, &result);
                result.is_ok()
            }
        }))
        .await
        .into_iter()
        .filter(|accepted| *accepted)
        .count();

        if accepted == 0 {
            return Err(anyhow!(
                "all {} builders rejected the tx",
                self.builders.len()
            ));
        }

        tracing::debug!(?hash, accepted, total = self.builders.len(), "broadcast tx");
        Ok(hash)
    }

    /// Builds the `X-Flashbots-Signature` value for the request body. It is
    /// sent to every builder: the ones that don't authenticate their callers
    /// ignore the header. Costs one signing call per submission, which is a
    /// second KMS round trip for KMS backed accounts.
    async fn sign_request(&self, signer: eth::Address, body: &[u8]) -> anyhow::Result<String> {
        let account = self
            .signers
            .get(&signer)
            .with_context(|| format!("no account registered for {signer}"))?;
        signature_header(account, body).await
    }

    /// Posts an already encoded JSON-RPC body to a single builder and returns
    /// the response body when the builder accepted the tx.
    async fn post(
        &self,
        builder: &Builder,
        body: Bytes,
        signature: &str,
    ) -> anyhow::Result<String> {
        let response = self
            .http
            .post(builder.url.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("X-Flashbots-Signature", signature)
            .body(body)
            .send()
            .await
            .context("failed to reach the builder")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read the builder response")?;
        check_response(status, &body)?;
        Ok(body)
    }

    /// Logs and counts the outcome of a single builder submission.
    fn observe_result(&self, builder: &Builder, hash: &eth::TxId, result: &anyhow::Result<String>) {
        let label = match result {
            Ok(body) => {
                tracing::debug!(
                    builder = builder.name,
                    ?hash,
                    body,
                    "builder accepted the tx"
                );
                "Success"
            }
            Err(err) => {
                tracing::warn!(
                    builder = builder.name,
                    ?err,
                    ?hash,
                    "builder rejected the tx"
                );
                "Rejected"
            }
        };
        metrics::get()
            .builder_submission
            .with_label_values(&[&self.mempool, &builder.name, label])
            .inc();
    }
}

/// Encodes the `eth_sendRawTransaction` request body for a signed settlement.
fn build_body(envelope: &TxEnvelope) -> anyhow::Result<Bytes> {
    Ok(Bytes::from(serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_sendRawTransaction",
        "params": [hex::encode_prefixed(envelope.encoded_2718())],
    }))?))
}

/// Builds the `X-Flashbots-Signature` header value. It authenticates the
/// caller, not the transaction, and is required by builders like BuilderNet,
/// which reject every unsigned request.
async fn signature_header(account: &Account, body: &[u8]) -> anyhow::Result<String> {
    // The signed message is the hex string of the body hash, not the hash
    // itself, and it gets the EIP-191 prefix on top.
    let message = hex::encode_prefixed(keccak256(body));
    let signature = account
        .sign_hash(&eip191_hash_message(message))
        .await
        .context("failed to sign the request body")?;
    Ok(format!(
        "{}:{}",
        TxSigner::address(account),
        hex::encode_prefixed(signature.as_bytes())
    ))
}

/// Builders disagree on the body of an acceptance (a tx hash, `null`, the
/// number 200), so only a failing status or a JSON-RPC `error` object counts
/// as a rejection.
fn check_response(status: reqwest::StatusCode, body: &str) -> anyhow::Result<()> {
    if !status.is_success() {
        return Err(anyhow!("builder returned status {status}: {body}"));
    }
    let error = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|response| response.get("error").cloned())
        .filter(|error| !error.is_null());
    match error {
        Some(error) => Err(anyhow!("builder returned error: {error}")),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::signers::local::PrivateKeySigner, reqwest::StatusCode};

    #[test]
    fn acceptance_bodies_are_not_parsed() {
        for body in [
            r#"{"jsonrpc":"2.0","id":1,"result":"0x1234"}"#,
            r#"{"jsonrpc":"2.0","id":1,"result":null}"#,
            r#"{"result":200,"error":null,"id":1}"#,
            "",
        ] {
            check_response(StatusCode::OK, body).unwrap();
        }
    }

    #[test]
    fn rejections_are_detected() {
        check_response(
            StatusCode::OK,
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"insufficient funds"}}"#,
        )
        .unwrap_err();
        check_response(StatusCode::BAD_GATEWAY, "<html>bad gateway</html>").unwrap_err();
        check_response(StatusCode::TOO_MANY_REQUESTS, "").unwrap_err();
    }

    #[tokio::test]
    async fn signature_header_recovers_to_the_signer() {
        let key = PrivateKeySigner::from_bytes(
            &"0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
                .parse()
                .unwrap(),
        )
        .unwrap();
        let address = key.address();
        let body =
            br#"{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["0x02"]}"#;

        let header = signature_header(&Account::PrivateKey(key), body)
            .await
            .unwrap();

        let (signer, signature) = header.split_once(':').unwrap();
        assert_eq!(signer, address.to_string());
        let signature: alloy::primitives::Signature = signature.parse().unwrap();
        let message = hex::encode_prefixed(keccak256(body));
        assert_eq!(
            signature
                .recover_address_from_msg(message.as_bytes())
                .unwrap(),
            address
        );
    }
}
