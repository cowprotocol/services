//! Solana JSON-RPC client wrapper.

#[cfg(feature = "test-util")]
pub use solana_rpc_client::mock_sender::{Mocks, MocksMap};
use {
    base64::Engine,
    futures::{TryFutureExt, future},
    itertools::Itertools,
    serde::{Deserialize, Deserializer},
    solana_account_decoder_client_types::UiAccount,
    solana_rpc_client::{
        http_sender::HttpSender,
        nonblocking::rpc_client::RpcClient,
        rpc_client::RpcClientConfig,
        rpc_sender::{RpcSender, RpcTransportStats},
    },
    solana_rpc_client_api::{
        config::{RpcBlockConfig, RpcSendTransactionConfig},
        request::{MAX_MULTIPLE_ACCOUNTS, RpcError},
        response::RpcBlockhash,
    },
    solana_sdk::{
        account::Account,
        hash::Hash,
        pubkey::Pubkey,
        signature::Signature,
        transaction::{TransactionError, VersionedTransaction},
    },
    solana_transaction_status_client_types::TransactionDetails,
    std::{
        collections::HashMap,
        time::{Duration, Instant},
    },
    url::Url,
};
pub use {
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client_api::{
        client_error::{Error, ErrorKind},
        request::RpcRequest,
        response::{
            Response,
            RpcPrioritizationFee,
            RpcSimulateTransactionResult,
            UiTransactionError,
        },
    },
    solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta,
};

/// The `simulateBundle` request, a Jito extension of the RPC API.
pub const SIMULATE_BUNDLE: RpcRequest = RpcRequest::Custom {
    method: "simulateBundle",
};

/// The newest transaction version requested back from the node, which fails
/// a request that would encode a newer one. The driver settles with v1.
const MAX_SUPPORTED_TRANSACTION_VERSION: u8 = 1;

pub struct SolanaRPC {
    inner: RpcClient,
}

impl SolanaRPC {
    /// Signatures requested per page of
    /// [`SolanaRPC::signatures_for_address`].
    pub const SIGNATURES_PAGE: usize = 1000;

    /// Creates a client for the given HTTP URL, request timeout and
    /// commitment level. `component` labels its requests in metrics.
    pub fn new_with_timeout_and_commitment(
        url: &Url,
        request_timeout: Duration,
        commitment: CommitmentConfig,
        component: &str,
    ) -> Self {
        Self {
            inner: RpcClient::new_sender(
                InstrumentedSender {
                    inner: HttpSender::new_with_timeout(url, request_timeout),
                    component: component.to_owned(),
                },
                RpcClientConfig::with_commitment(commitment),
            ),
        }
    }

    /// Creates a client backed by a mocked sender, for tests. The `url` is
    /// interpreted as a mock directive (e.g. `"succeeds"`, `"fails"`).
    #[cfg(feature = "test-util")]
    pub fn new_mock(url: String) -> Self {
        Self {
            inner: RpcClient::new_mock(url),
        }
    }

    /// Creates a client answering from canned per-request responses, for
    /// tests.
    #[cfg(feature = "test-util")]
    pub fn new_mock_with_mocks(mocks: Mocks) -> Self {
        Self {
            inner: RpcClient::new_mock_with_mocks("mock".to_owned(), mocks),
        }
    }

    /// Creates a client answering each request from its queue of canned
    /// responses, in order, for tests.
    #[cfg(feature = "test-util")]
    pub fn new_mock_with_mocks_map(mocks: MocksMap) -> Self {
        Self {
            inner: RpcClient::new_mock_with_mocks_map("mock", mocks),
        }
    }

    /// Creates a client that fails each request with its queued transaction
    /// errors before answering from its queue of canned responses, for
    /// tests.
    #[cfg(feature = "test-util")]
    pub fn new_mock_with_failures(
        mocks: MocksMap,
        failures: impl IntoIterator<Item = (RpcRequest, solana_sdk::transaction::TransactionError)>,
    ) -> Self {
        Self {
            inner: RpcClient::new_sender(
                mock::FailingSender::new(mocks, failures),
                solana_rpc_client::rpc_client::RpcClientConfig::with_commitment(
                    CommitmentConfig::default(),
                ),
            ),
        }
    }

    /// Fetch accounts by key. Accounts that do not exist are absent from the
    /// map. Duplicate keys are fetched once, and batches above the server's
    /// per-request cap are split into parallel requests.
    pub async fn multiple_accounts(
        &self,
        keys: impl IntoIterator<Item = Pubkey>,
    ) -> Result<HashMap<Pubkey, Account>, Error> {
        let unique: Vec<Pubkey> = keys.into_iter().unique().collect();
        let fetched = future::try_join_all(unique.chunks(MAX_MULTIPLE_ACCOUNTS).map(|chunk| {
            self.inner
                .get_multiple_accounts(chunk)
                .map_ok(move |accounts| {
                    accounts
                        .into_iter()
                        .zip(chunk)
                        .filter_map(|(account, key)| Some((*key, account?)))
                        .collect::<Vec<_>>()
                })
        }))
        .await?;
        Ok(fetched.into_iter().flatten().collect())
    }

    /// The node's current slot at the client's commitment level.
    pub async fn slot(&self) -> Result<u64, Error> {
        self.inner.get_slot().await
    }

    /// The current block height at the client's commitment level.
    pub async fn block_height(&self) -> Result<BlockHeight, Error> {
        Ok(BlockHeight(self.inner.get_block_height().await?))
    }

    /// The latest confirmed blockhash, the last block height at which it
    /// stays usable, and the slot of its block.
    ///
    /// The method always fetches the blockhash at `confirmed`,
    /// regardless of the client's configured commitment level.
    pub async fn latest_confirmed_blockhash(&self) -> Result<LatestBlockhash, Error> {
        let response: Response<RpcBlockhash> = self
            .inner
            .send(
                RpcRequest::GetLatestBlockhash,
                serde_json::json!([CommitmentConfig::confirmed()]),
            )
            .await?;
        Ok(LatestBlockhash {
            blockhash: parse_hash(&response.value.blockhash)?,
            last_valid_block_height: BlockHeight(response.value.last_valid_block_height),
            slot: response.context.slot,
        })
    }

    /// The slots of the confirmed blocks from `start` to `end`, both
    /// included, in ascending order.
    pub async fn blocks(&self, start: u64, end: u64) -> Result<Vec<u64>, Error> {
        self.inner
            .get_blocks_with_commitment(start, Some(end), CommitmentConfig::confirmed())
            .await
    }

    /// The hash and height of the confirmed block at `slot`.
    pub async fn block(&self, slot: u64) -> Result<(Hash, BlockHeight), Error> {
        let config = RpcBlockConfig {
            transaction_details: Some(TransactionDetails::None),
            rewards: Some(false),
            commitment: Some(CommitmentConfig::confirmed()),
            // Agave checks no version without transaction details, but a
            // provider that does would fail every block holding a newer one.
            max_supported_transaction_version: Some(MAX_SUPPORTED_TRANSACTION_VERSION),
            ..Default::default()
        };
        let block = self.inner.get_block_with_config(slot, config).await?;
        let height = block
            .block_height
            .ok_or_else(|| ErrorKind::from(RpcError::ParseError("block height".to_string())))?;
        Ok((parse_hash(&block.blockhash)?, BlockHeight(height)))
    }

    /// Whether each signature exists on the node's chain. The lookup falls
    /// back to the ledger history when a signature is past the recent-status
    /// cache, so `false` means the transaction never landed or was rolled
    /// back, not merely that it aged out. A status of any commitment level
    /// counts as existing: the question here is presence, not finality.
    pub async fn known_signatures(&self, signatures: &[Signature]) -> Result<Vec<bool>, Error> {
        // getSignatureStatuses rejects calls with more than 256 signatures.
        const CHUNK: usize = 256;
        let mut known = Vec::with_capacity(signatures.len());
        for chunk in signatures.chunks(CHUNK) {
            let response = self
                .inner
                .get_signature_statuses_with_history(chunk)
                .await?;
            known.extend(response.value.into_iter().map(|status| status.is_some()));
        }
        Ok(known)
    }

    /// One page of an address's transaction signatures, starting below
    /// `before` when given. The node serves deep history, so repeated calls
    /// walk arbitrarily far back.
    pub async fn signatures_for_address(
        &self,
        address: &Pubkey,
        before: Option<Signature>,
    ) -> Result<SignaturesPage, Error> {
        // Pinned to confirmed like `slot` and `transaction`: the node's
        // default is finalized, which would hide the confirmed tip's last
        // ~32 slots from the scan.
        let config = solana_rpc_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
            before,
            limit: Some(Self::SIGNATURES_PAGE),
            commitment: Some(CommitmentConfig::confirmed()),
            ..Default::default()
        };
        let page = self
            .inner
            .get_signatures_for_address_with_config(address, config)
            .await?;
        // Keyed on the raw length: parse failures below must not read as an
        // exhausted history.
        let full = page.len() == Self::SIGNATURES_PAGE;
        let entries = page
            .into_iter()
            .filter_map(|status| match status.signature.parse() {
                Ok(signature) => Some((signature, status.slot)),
                // The node returned a malformed signature. There is nothing
                // to fetch or dead-letter without one, so skip it loudly.
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        signature = %status.signature,
                        "skipping an unparsable signature"
                    );
                    None
                }
            })
            .collect();
        Ok(SignaturesPage { entries, full })
    }

    /// One confirmed transaction with its metadata, base64-encoded.
    pub async fn transaction(
        &self,
        signature: &Signature,
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta, Error> {
        self.inner
            .get_transaction_with_config(
                signature,
                solana_rpc_client_api::config::RpcTransactionConfig {
                    encoding: Some(
                        solana_transaction_status_client_types::UiTransactionEncoding::Base64,
                    ),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(MAX_SUPPORTED_TRANSACTION_VERSION),
                },
            )
            .await
    }

    /// Simulate a versioned transaction without sending it. The value holds
    /// the simulation result including logs and any error; the context carries
    /// the slot it ran against.
    pub async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Response<RpcSimulateTransactionResult>, Error> {
        self.inner.simulate_transaction(transaction).await
    }

    /// The prioritization fee, in micro-lamports per compute unit, that
    /// landed a transaction locking `addresses` as writable in each of the
    /// node's recent slots (up to 150), or the slot's lowest fee when none
    /// did.
    pub async fn recent_prioritization_fees(
        &self,
        addresses: &[Pubkey],
    ) -> Result<Vec<RpcPrioritizationFee>, Error> {
        self.inner.get_recent_prioritization_fees(addresses).await
    }

    /// Simulate the transactions as one atomic bundle on the confirmed bank
    /// without sending them, signatures unverified and blockhashes replaced by
    /// the node's latest, so partially signed and stale transactions still
    /// simulate. The value holds one result per executed transaction, in
    /// order, ending at the first failure; the context carries the slot it
    /// simulated against. `accounts` lists, per transaction, the accounts to
    /// report before and after it, a missing list reporting none. A node
    /// without the Jito extension answers with an error.
    pub async fn simulate_bundle(
        &self,
        transactions: &[VersionedTransaction],
        accounts: &[Vec<Pubkey>],
    ) -> Result<Response<Vec<RpcSimulateBundleTransactionResult>>, Error> {
        let encoded = transactions
            .iter()
            .map(|transaction| {
                wincode::serialize(transaction)
                    .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| ErrorKind::Custom(format!("serialize transaction: {err}")))?;
        // The account config lists are mandatory: one entry per transaction.
        let accounts: Vec<serde_json::Value> = (0..encoded.len())
            .map(|index| match accounts.get(index) {
                Some(addresses) if !addresses.is_empty() => serde_json::json!({
                    "encoding": "base64",
                    "addresses": addresses.iter().map(ToString::to_string).collect::<Vec<_>>(),
                }),
                _ => serde_json::Value::Null,
            })
            .collect();
        let params = serde_json::json!([
            { "encodedTransactions": encoded },
            {
                "preExecutionAccountsConfigs": accounts,
                "postExecutionAccountsConfigs": accounts,
                "transactionEncoding": "base64",
                "skipSigVerify": true,
                "replaceRecentBlockhash": true,
                // Jito defaults to the confirmed bank, but another provider
                // may default to processed or tip and disagree with this
                // crate's confirmed reads.
                "simulationBank": { "commitment": CommitmentConfig::confirmed() },
            },
        ]);
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Value {
            transaction_results: Vec<RpcSimulateBundleTransactionResult>,
        }
        let response: Response<Value> = self.inner.send(SIMULATE_BUNDLE, params).await?;
        Ok(Response {
            context: response.context,
            value: response.value.transaction_results,
        })
    }

    /// The largest token accounts of `mint`, up to 20, largest first. Needs a
    /// node with the account index, which most refuse.
    pub async fn token_largest_accounts(&self, mint: &Pubkey) -> Result<Vec<TokenHolding>, Error> {
        let holdings = self.inner.get_token_largest_accounts(mint).await?;
        Ok(holdings
            .into_iter()
            .filter_map(|holding| {
                Some(TokenHolding {
                    account: holding.address.parse().ok()?,
                    amount: holding.amount.amount.parse().ok()?,
                })
            })
            .collect())
    }

    /// Whether the blockhash is still usable for a new transaction at
    /// confirmed commitment.
    pub async fn is_blockhash_valid(&self, blockhash: &Hash) -> Result<bool, Error> {
        self.inner
            .is_blockhash_valid(blockhash, CommitmentConfig::confirmed())
            .await
    }

    /// Send a versioned transaction once, after the preflight simulation at
    /// the client's commitment level. The node keeps no copy to rebroadcast,
    /// so the caller resends it. A rejected preflight is an
    /// [`ErrorKind::RpcError`].
    pub async fn send_transaction_once(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.inner
            .send_transaction_with_config(
                transaction,
                RpcSendTransactionConfig {
                    preflight_commitment: Some(self.inner.commitment().commitment),
                    // A node drops every resend of a transaction it holds to
                    // rebroadcast.
                    max_retries: Some(0),
                    ..RpcSendTransactionConfig::default()
                },
            )
            .await
    }

    /// The outcome of a transaction at the client's commitment level, `None`
    /// until it lands. An error means the transaction landed and failed, so it
    /// cannot land again.
    pub async fn signature_status(
        &self,
        signature: &Signature,
    ) -> Result<Option<Result<(), TransactionError>>, Error> {
        self.inner.get_signature_status(signature).await
    }

    /// Send a versioned transaction without waiting for it to land.
    pub async fn send_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.inner.send_transaction(transaction).await
    }

    /// Send a transaction again, without the preflight simulation and
    /// without the node's own rebroadcasts. A copy of a transaction the
    /// cluster already processed is dropped.
    pub async fn resend_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.inner
            .send_transaction_with_config(
                transaction,
                RpcSendTransactionConfig {
                    skip_preflight: true,
                    max_retries: Some(0),
                    ..RpcSendTransactionConfig::default()
                },
            )
            .await
    }
}

/// One transaction's execution inside a simulated bundle.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateBundleTransactionResult {
    pub err: Option<UiTransactionError>,
    pub logs: Option<Vec<String>>,
    /// Compute units the transaction consumed, `None` when the endpoint does
    /// not report them.
    pub units_consumed: Option<u64>,
    /// The requested accounts as they were before the transaction, in request
    /// order, a missing account as an empty one. Empty when none were
    /// requested.
    #[serde(default, deserialize_with = "deserialize_accounts")]
    pub pre_execution_accounts: Vec<Account>,
    /// The requested accounts as the transaction left them, in request order,
    /// a missing account as an empty one. Empty when none were requested.
    #[serde(default, deserialize_with = "deserialize_accounts")]
    pub post_execution_accounts: Vec<Account>,
}

/// A token account and its balance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenHolding {
    pub account: Pubkey,
    pub amount: u64,
}

/// Accounts the node encoded, an undecodable one as empty.
fn deserialize_accounts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Account>, D::Error> {
    let accounts: Option<Vec<UiAccount>> = Deserialize::deserialize(deserializer)?;
    Ok(accounts
        .unwrap_or_default()
        .iter()
        .map(|account| account.to_account().unwrap_or_default())
        .collect())
}

/// One page of an address's transaction history.
pub struct SignaturesPage {
    /// Parsed (signature, slot) entries, newest first. Signatures the node
    /// returned malformed are dropped.
    pub entries: Vec<(Signature, u64)>,
    /// The raw page hit the request limit, so older history may remain
    /// below it.
    pub full: bool,
}

/// A Solana block height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockHeight(u64);

impl BlockHeight {
    /// The height as a plain number.
    pub const fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<u64> for BlockHeight {
    fn from(height: u64) -> Self {
        Self(height)
    }
}

impl From<BlockHeight> for u64 {
    fn from(height: BlockHeight) -> Self {
        height.0
    }
}

/// An HTTP sender that records each request in metrics.
struct InstrumentedSender {
    inner: HttpSender,
    component: String,
}

#[async_trait::async_trait]
impl RpcSender for InstrumentedSender {
    async fn send(
        &self,
        request: RpcRequest,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let _request = RequestMetrics::start(&self.component, request.as_str());
        self.inner.send(request, params).await
    }

    fn get_transport_stats(&self) -> RpcTransportStats {
        self.inner.get_transport_stats()
    }

    fn url(&self) -> String {
        self.inner.url()
    }
}

/// A request in flight. Dropping it records the request as complete, so a
/// cancelled request counts too.
struct RequestMetrics<'a> {
    labels: [&'a str; 2],
    start: Instant,
}

impl<'a> RequestMetrics<'a> {
    fn start(component: &'a str, method: &'a str) -> Self {
        let labels = [component, method];
        metrics().requests_inflight.with_label_values(&labels).inc();
        Self {
            labels,
            start: Instant::now(),
        }
    }
}

impl Drop for RequestMetrics<'_> {
    fn drop(&mut self) {
        let metrics = metrics();
        metrics
            .requests_inflight
            .with_label_values(&self.labels)
            .dec();
        metrics
            .requests_complete
            .with_label_values(&self.labels)
            .inc();
        metrics
            .requests_duration_seconds
            .with_label_values(&self.labels)
            .observe(self.start.elapsed().as_secs_f64());
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "rpc")]
struct Metrics {
    /// Requests in flight, by component and method.
    #[metric(labels("component", "method"))]
    requests_inflight: prometheus::IntGaugeVec,
    /// Completed requests, by component and method.
    #[metric(labels("component", "method"))]
    requests_complete: prometheus::IntCounterVec,
    /// Request durations, by component and method.
    #[metric(labels("component", "method"))]
    requests_duration_seconds: prometheus::HistogramVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

#[cfg(feature = "test-util")]
mod mock {
    use {
        super::*,
        solana_rpc_client::{
            mock_sender::MockSender,
            rpc_sender::{RpcSender, RpcTransportStats},
        },
        solana_sdk::transaction::TransactionError,
        std::{collections::VecDeque, sync::Mutex},
    };

    /// A mock sender that fails a request with its queued transaction errors
    /// before answering it from the canned responses.
    pub struct FailingSender {
        inner: MockSender,
        failures: Mutex<HashMap<RpcRequest, VecDeque<TransactionError>>>,
    }

    impl FailingSender {
        pub fn new(
            mocks: MocksMap,
            failures: impl IntoIterator<Item = (RpcRequest, TransactionError)>,
        ) -> Self {
            let mut queued: HashMap<RpcRequest, VecDeque<TransactionError>> = HashMap::new();
            for (request, err) in failures {
                queued.entry(request).or_default().push_back(err);
            }
            Self {
                inner: MockSender::new_with_mocks_map("mock", mocks),
                failures: Mutex::new(queued),
            }
        }
    }

    #[async_trait::async_trait]
    impl RpcSender for FailingSender {
        async fn send(
            &self,
            request: RpcRequest,
            params: serde_json::Value,
        ) -> Result<serde_json::Value, Error> {
            let failure = self
                .failures
                .lock()
                .unwrap()
                .get_mut(&request)
                .and_then(VecDeque::pop_front);
            match failure {
                Some(err) => Err(err.into()),
                None => self.inner.send(request, params).await,
            }
        }

        fn get_transport_stats(&self) -> RpcTransportStats {
            self.inner.get_transport_stats()
        }

        fn url(&self) -> String {
            self.inner.url()
        }
    }
}

/// The latest confirmed blockhash, the last block height at which it
/// stays usable, and the slot of its block.
#[derive(Debug)]
pub struct LatestBlockhash {
    /// The blockhash to sign transactions with.
    pub blockhash: Hash,
    /// The last block height at which transactions signed with this
    /// blockhash as `recent_blockhash` remain valid.
    pub last_valid_block_height: BlockHeight,
    /// The slot of the block the blockhash belongs to.
    pub slot: u64,
}

fn parse_hash(hash: &str) -> Result<Hash, Error> {
    hash.parse()
        .map_err(|_| ErrorKind::from(RpcError::ParseError("Hash".to_string())).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_request_counts_as_complete() {
        let labels = ["test", "getSlot"];
        let metrics = metrics();
        let request = RequestMetrics::start("test", "getSlot");
        assert_eq!(
            metrics.requests_inflight.with_label_values(&labels).get(),
            1
        );
        drop(request);
        assert_eq!(
            metrics.requests_inflight.with_label_values(&labels).get(),
            0
        );
        assert_eq!(
            metrics.requests_complete.with_label_values(&labels).get(),
            1
        );
    }
}
