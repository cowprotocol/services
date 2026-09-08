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
        primitives::{Address, keccak256},
    },
    anyhow::{Context, anyhow},
    eth_domain_types as eth,
    futures::future::join_all,
    std::collections::HashMap,
    url::Url,
};

/// A builder that stops answering must not hold up the whole broadcast:
/// submission is on the critical path of the block.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Values of the `result` label of the submission metric.
const SUCCESS: &str = "Success";
const REJECTED: &str = "Rejected";

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
        // Create the counter series up front so they read 0 instead of being
        // absent before the first settlement.
        for builder in &builders {
            for result in [SUCCESS, REJECTED] {
                metrics::get().builder_submission.with_label_values(&[
                    &mempool,
                    &builder.name,
                    result,
                ]);
            }
        }

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

    pub fn len(&self) -> usize {
        self.builders.len()
    }

    pub fn names(&self) -> Vec<&str> {
        self.builders
            .iter()
            .map(|builder| builder.name.as_str())
            .collect()
    }

    /// Broadcasts the raw bytes of an already signed settlement to every
    /// builder. Succeeds as long as one builder accepted it.
    pub async fn broadcast(
        &self,
        envelope: &TxEnvelope,
        signer: eth::Address,
    ) -> anyhow::Result<eth::TxId> {
        let hash = eth::TxId(*envelope.tx_hash());

        // Every builder receives the exact same bytes, so one body and one
        // request signature are enough.
        let body = serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendRawTransaction",
            "params": [hex::encode_prefixed(envelope.encoded_2718())],
        }))
        .context("failed to encode the builder request")?;

        // Sent to every builder: the ones that don't authenticate their callers
        // ignore the header. Costs one signing call per submission, which is a
        // second KMS round trip for KMS backed accounts.
        let signature = match self.sign_request(&body, signer).await {
            Ok(signature) => Some(signature),
            Err(err) => {
                // Only BuilderNet insists on the header, so the other builders
                // can still take the tx.
                tracing::warn!(?err, ?signer, "failed to sign the builder request");
                None
            }
        };

        let accepted = join_all(self.builders.iter().map(|builder| {
            let body = &body;
            let signature = signature.as_deref();
            async move {
                let result = match self.post(builder, body, signature).await {
                    Ok(()) => SUCCESS,
                    Err(err) => {
                        tracing::warn!(
                            builder = builder.name,
                            ?err,
                            ?hash,
                            "builder rejected the tx"
                        );
                        REJECTED
                    }
                };
                metrics::get()
                    .builder_submission
                    .with_label_values(&[&self.mempool, &builder.name, result])
                    .inc();
                result == SUCCESS
            }
        }))
        .await
        .into_iter()
        .filter(|accepted| *accepted)
        .count();

        if accepted == 0 {
            return Err(anyhow!("all {} builders rejected the tx", self.len()));
        }

        tracing::debug!(?hash, accepted, total = self.len(), "broadcast tx");
        Ok(hash)
    }

    /// Posts an already encoded JSON-RPC body to a single builder.
    async fn post(
        &self,
        builder: &Builder,
        body: &[u8],
        signature: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut request = self
            .http
            .post(builder.url.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_vec());
        if let Some(signature) = signature {
            request = request.header("X-Flashbots-Signature", signature);
        }
        let response = request
            .send()
            .await
            .context("failed to reach the builder")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read the builder response")?;
        parse_response(status, &body)
    }

    /// Signs the request body with the account that also signs the settlement.
    async fn sign_request(&self, body: &[u8], signer: eth::Address) -> anyhow::Result<String> {
        let account = self
            .signers
            .get(&signer)
            .with_context(|| format!("no account registered for {signer}"))?;
        request_signature(account, body).await
    }
}

/// Builds the `X-Flashbots-Signature` header value. It authenticates the
/// caller, not the transaction, and is required by builders like BuilderNet,
/// which reject every unsigned request.
async fn request_signature(account: &Account, body: &[u8]) -> anyhow::Result<String> {
    // The signed message is the hex string of the body hash, not the hash
    // itself, and it gets the EIP-191 prefix on top.
    let message = hex::encode_prefixed(keccak256(body));
    let signature = account
        .sign_message(message.as_bytes())
        .await
        .context("failed to sign the request body")?;
    Ok(format!(
        "{}:{}",
        TxSigner::address(account),
        hex::encode_prefixed(signature.as_bytes())
    ))
}

/// Interprets a builder's `eth_sendRawTransaction` response. Builders disagree
/// on what acceptance looks like: some echo the tx hash, Titan answers with a
/// null result, btcs and ultrasound answer with the number 200, and some do
/// not wrap the answer in a JSON-RPC envelope at all. The body therefore only
/// gets to veto: an explicit JSON-RPC error is a rejection, and otherwise the
/// HTTP status decides. We never read the tx hash out of it, because the hash
/// is computed locally from the signed envelope.
fn parse_response(status: reqwest::StatusCode, body: &str) -> anyhow::Result<()> {
    #[derive(serde::Deserialize)]
    struct Response {
        #[serde(default)]
        error: Option<Error>,
    }

    #[derive(serde::Deserialize)]
    struct Error {
        code: i64,
        message: String,
    }

    // A body that is not a JSON-RPC object carries no verdict, so it leaves
    // the decision to the status rather than counting as a rejection.
    let error = serde_json::from_str::<Response>(body)
        .ok()
        .and_then(|response| response.error);

    match error {
        Some(error) => Err(anyhow!(
            "builder returned error {}: {}",
            error.code,
            error.message
        )),
        None if status.is_success() => Ok(()),
        None => Err(anyhow!("builder returned status {status}: {body}")),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::signers::local::PrivateKeySigner, reqwest::StatusCode};

    /// Titan accepts a tx without echoing its hash.
    #[test]
    fn null_result_is_accepted() {
        parse_response(StatusCode::OK, r#"{"jsonrpc":"2.0","id":1,"result":null}"#).unwrap();
    }

    #[test]
    fn hash_result_is_accepted() {
        parse_response(
            StatusCode::OK,
            r#"{"jsonrpc":"2.0","id":1,"result":"0x46dbd4bf9c6711fb8f4a4a8041122402cb3ee6e95901a972bee5baf4c14fbee2"}"#,
        )
        .unwrap();
    }

    /// `ultrasound` answers a number, not a hash.
    #[test]
    fn non_hash_result_is_accepted() {
        parse_response(StatusCode::OK, r#"{"jsonrpc":"2.0","id":1,"result":200}"#).unwrap();
    }

    /// What BuilderNet answers to an unsigned request.
    #[test]
    fn json_rpc_error_is_rejected() {
        let err = parse_response(
            StatusCode::OK,
            r#"{"id":null,"jsonrpc":"2.0","error":{"code":-32603,"message":"Invalid signature"}}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Invalid signature"), "{err}");
    }

    #[test]
    fn non_json_error_body_is_rejected() {
        parse_response(StatusCode::BAD_GATEWAY, "<html>bad gateway</html>").unwrap_err();
        parse_response(StatusCode::TOO_MANY_REQUESTS, "").unwrap_err();
    }

    /// Builders that answer outside a JSON-RPC envelope still accepted the tx.
    /// Only the status can refuse when the body carries no explicit error.
    #[test]
    fn non_json_rpc_success_body_is_accepted() {
        parse_response(StatusCode::OK, "200").unwrap();
        parse_response(StatusCode::OK, "OK").unwrap();
        parse_response(StatusCode::OK, "").unwrap();
    }

    /// The same bodies are still rejected when the status says so.
    #[test]
    fn non_json_rpc_body_follows_the_status() {
        parse_response(StatusCode::BAD_GATEWAY, "200").unwrap_err();
        parse_response(StatusCode::INTERNAL_SERVER_ERROR, "OK").unwrap_err();
    }

    /// A JSON-RPC error wins even when the transport reports success.
    #[test]
    fn json_rpc_error_beats_a_success_status() {
        parse_response(
            StatusCode::OK,
            r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"nonce too low"}}"#,
        )
        .unwrap_err();
    }

    #[tokio::test]
    async fn request_signature_recovers_to_the_signer() {
        let key = PrivateKeySigner::from_bytes(
            &"0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
                .parse()
                .unwrap(),
        )
        .unwrap();
        let address = key.address();
        let body =
            br#"{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["0x02"]}"#;

        let header = request_signature(&Account::PrivateKey(key), body)
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

    #[tokio::test]
    async fn address_only_accounts_cannot_sign_requests() {
        let account = Account::Address(Address::repeat_byte(1));

        request_signature(&account, b"{}").await.unwrap_err();
    }
}
