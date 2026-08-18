mod buffering;
pub mod errors;
mod evm_ext;
mod instrumentation;
mod rpc_headers;
mod wallet;

use {
    crate::{AlloyProvider, Config},
    alloy_provider::{Provider, ProviderBuilder},
    alloy_rpc_client::{ClientBuilder, RpcClient},
    buffering::BatchCallLayer,
    instrumentation::{InstrumentationLayer, LabelingLayer},
    rpc_headers::{RpcHeadersLayer, TracingRequestIdLayer},
};
pub use {evm_ext::EvmProviderExt, instrumentation::ProviderLabelingExt, wallet::MutWallet};

/// Creates an [`RpcClient`] from the given URL with [`LabelingLayer`],
/// [`InstrumentationLayer`], [`TracingRequestIdLayer`], [`BatchCallLayer`] and
/// [`RpcHeadersLayer`].
///
/// [`TracingRequestIdLayer`] is installed *before* [`BatchCallLayer`] so it
/// runs on the caller's task and can capture the tracing request id from the
/// current span. [`RpcHeadersLayer`] is installed last so it runs innermost —
/// it must observe the packet after [`BatchCallLayer`] has coalesced calls into
/// a batch.
fn rpc(url: &str, config: Config, label: Option<&str>) -> RpcClient {
    ClientBuilder::default()
        .layer(LabelingLayer {
            label: label.unwrap_or("main").into(),
        })
        .layer(InstrumentationLayer)
        .layer(TracingRequestIdLayer)
        .layer(BatchCallLayer::new(config))
        .layer(RpcHeadersLayer)
        .http(url.parse().unwrap())
}

/// Creates an unbuffered [`RpcClient`] from the given URL with
/// [`LabelingLayer`] and [`InstrumentationLayer`] but WITHOUT
/// [`BatchCallLayer`].
///
/// This is useful for components that need to avoid batching (e.g., block
/// stream polling on high-frequency chains).
fn unbuffered_rpc(url: &str, label: Option<&str>) -> RpcClient {
    ClientBuilder::default()
        .layer(LabelingLayer {
            label: label.unwrap_or("main_unbuffered").into(),
        })
        .layer(InstrumentationLayer)
        .layer(TracingRequestIdLayer)
        .layer(RpcHeadersLayer)
        .http(url.parse().unwrap())
}

/// Creates an unbuffered provider for the given URL and label.
///
/// Unlike [`provider()`], this does not include batching.
/// Useful for read-only operations like block polling.
///
/// Returns a copy of the [`MutWallet`] so the caller can modify it later.
pub fn unbuffered_provider(url: &str, label: Option<&str>) -> (AlloyProvider, MutWallet) {
    let rpc = unbuffered_rpc(url, label);
    let wallet = MutWallet::default();
    let provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .with_simple_nonce_management()
        .connect_client(rpc)
        .erased();

    (provider, wallet)
}

/// Creates a provider with the provided URL and an empty [`MutWallet`].
///
/// Returns a copy of the [`MutWallet`] so the caller can modify it later.
pub fn provider(url: &str, config: Config, label: Option<&str>) -> (AlloyProvider, MutWallet) {
    let rpc = rpc(url, config, label);
    let wallet = MutWallet::default();
    let provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        // will query the node for the nonce every time that it is needed
        // adds overhead but makes working with alloy at the same time much simpler
        .with_simple_nonce_management()
        .connect_client(rpc)
        .erased();

    (provider, wallet)
}

/// Extension to simplify using random IDs when instantiating [`RpcClient`].
pub trait RpcClientRandomIdExt {
    fn with_random_id(t: impl IntoBoxTransport, is_local: bool) -> Self;
}

impl RpcClientRandomIdExt for RpcClient {
    /// Creates a new [`RpcClient`] with a random request ID.
    fn with_random_id(t: impl IntoBoxTransport, is_local: bool) -> Self {
        // The random ID mitigates the possibility of duplicate request IDs between
        // providers when batching; furthemore, since we're using a uniform distribution
        // we need to be aware that we might get a value close enough to u64::MAX to
        // overflow after a couple requests, to solve that we generate a u32 first and
        // convert it to u64 to ensure we have plenty space.
        let id = rand::random::<u32>().into();
        let inner = RpcClientInner::new(t, is_local).with_id(id);
        Self::from_inner(inner)
    }
}

pub trait ProviderSignerExt {
    /// Creates a new provider without any signers.
    /// This is only ever useful if you configured
    /// anvil to impersonate some account and want
    /// to avoid alloy complaining that it doesn't
    /// have the private key for the requested signer.
    fn without_wallet(&self) -> Self;
}

impl ProviderSignerExt for AlloyProvider {
    fn without_wallet(&self) -> Self {
        let is_local = self.client().is_local();
        let transport = self.client().transport().clone();
        let client = RpcClient::with_random_id(transport, is_local);

        ProviderBuilder::new()
            .with_simple_nonce_management()
            .connect_client(client)
            .erased()
    }
}

#[cfg(feature = "test-util")]
mod test_util {
    use {
        super::*,
        alloy_contract::{CallBuilder, CallDecoder},
        alloy_primitives::TxHash,
        alloy_provider::Network,
        alloy_rpc_types::TransactionRequest,
        std::time::Duration,
        tokio::time::timeout,
    };

    const DEFAULT_WATCH_TIMEOUT: Duration = Duration::from_secs(2);

    pub trait ProviderExt {
        /// Sends the transaction to the node and waits for confirmations.
        ///
        /// If confirmation takes longer than 25 seconds, the operation will
        /// timeout.
        fn send_and_watch(
            &self,
            tx: TransactionRequest,
        ) -> impl Future<Output = anyhow::Result<TxHash>>;
    }

    impl ProviderExt for AlloyProvider {
        async fn send_and_watch(&self, tx: TransactionRequest) -> anyhow::Result<TxHash> {
            let pending = self.send_transaction(tx).await?;
            let result = timeout(DEFAULT_WATCH_TIMEOUT, pending.watch()).await??;
            Ok(result)
        }
    }

    pub trait CallBuilderExt<N> {
        /// Converts the current call into a [`TransactionRequest`], sends it to
        /// the node and waits for confirmations.
        ///
        /// If confirmation takes longer than 25 seconds, the operation will
        /// timeout.
        fn send_and_watch(&self) -> impl Future<Output = anyhow::Result<TxHash>>;
    }

    impl<P: Provider<N>, D: CallDecoder, N: Network> CallBuilderExt<N> for CallBuilder<P, D, N> {
        async fn send_and_watch(&self) -> anyhow::Result<TxHash> {
            let pending = self.send().await?;
            let result = timeout(DEFAULT_WATCH_TIMEOUT, pending.watch()).await??;
            Ok(result)
        }
    }
}

#[cfg(feature = "test-util")]
pub use test_util::{CallBuilderExt, ProviderExt};
use {alloy_rpc_client::RpcClientInner, alloy_transport::IntoBoxTransport};

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{
            net::SocketAddr,
            sync::{Arc, Mutex},
            time::Duration,
        },
        tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::{TcpListener, TcpStream},
        },
    };

    /// A minimal JSON-RPC endpoint that records the packets it receives and
    /// answers every request with an error. The tests only care about the
    /// shape of what reaches the node, not about the results.
    struct Recorder {
        url: String,
        packets: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    impl Recorder {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr: SocketAddr = listener.local_addr().unwrap();
            let packets = Arc::new(Mutex::new(Vec::new()));

            let recorded = packets.clone();
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    let recorded = recorded.clone();
                    tokio::spawn(async move { serve(socket, recorded).await });
                }
            });

            Self {
                url: format!("http://{addr}"),
                packets,
            }
        }

        /// The recorded packets that carry `eth_call`s, so that incidental
        /// traffic from the provider's fillers cannot affect the assertions.
        fn eth_call_packets(&self) -> Vec<serde_json::Value> {
            self.packets
                .lock()
                .unwrap()
                .iter()
                .filter(|packet| packet.to_string().contains("eth_call"))
                .cloned()
                .collect()
        }
    }

    async fn serve(mut socket: TcpStream, recorded: Arc<Mutex<Vec<serde_json::Value>>>) {
        let body = read_body(&mut socket).await;
        let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let response = serde_json::to_vec(&error_response(&request)).unwrap();
        recorded.lock().unwrap().push(request);

        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
             {}\r\nconnection: close\r\n\r\n",
            response.len()
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        socket.write_all(&response).await.unwrap();
        socket.shutdown().await.unwrap();
    }

    /// Reads one HTTP request and returns its body.
    async fn read_body(socket: &mut TcpStream) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let read = socket.read(&mut chunk).await.unwrap();
            assert!(
                read > 0,
                "connection closed before the request was complete"
            );
            buffer.extend_from_slice(&chunk[..read]);

            let Some(end_of_head) = buffer.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let head = String::from_utf8_lossy(&buffer[..end_of_head]).to_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .expect("request without content length")
                .trim()
                .parse()
                .unwrap();

            let body = end_of_head + 4;
            if buffer.len() >= body + length {
                return buffer[body..body + length].to_vec();
            }
        }
    }

    /// Builds a JSON-RPC error answer of the same shape (single or batch) as
    /// the request.
    fn error_response(request: &serde_json::Value) -> serde_json::Value {
        let error = |request: &serde_json::Value| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": { "code": -32000, "message": "recorder does not answer calls" },
            })
        };
        match request {
            serde_json::Value::Array(requests) => requests.iter().map(error).collect(),
            request => error(request),
        }
    }

    fn config() -> Config {
        Config {
            ethrpc_max_batch_size: 20,
            ethrpc_max_concurrent_requests: 10,
            // Give both aggregates time to land in the same batch.
            ethrpc_batch_delay: Duration::from_millis(100),
        }
    }

    /// `MulticallBuilder` sends its `aggregate3` through `provider.root()`,
    /// which skips provider layers. It does not skip [`BatchCallLayer`],
    /// because that one lives in the transport stack of the [`RpcClient`]
    /// that `root()` holds.
    ///
    /// The proof: two `aggregate3` calls issued concurrently against a
    /// provider built by [`provider()`] arrive at the node as a *single*
    /// JSON-RPC batch of two `eth_call`s. Nothing but our layer coalesces
    /// requests that way, as [`multicall3_is_not_batched_by_alloy_itself`]
    /// shows.
    #[tokio::test]
    async fn multicall3_still_passes_through_our_layers() {
        let node = Recorder::start().await;
        let (provider, _wallet) = provider(&node.url, config(), Some("test"));

        let first = provider.multicall().get_block_number().get_chain_id();
        let second = provider.multicall().get_block_number().get_chain_id();
        let (first, second) = futures::join!(first.aggregate3(), second.aggregate3());

        // The node answers with errors, so both aggregates fail. What matters
        // is the shape of what reached it.
        assert!(first.is_err());
        assert!(second.is_err());

        let packets = node.eth_call_packets();
        assert_eq!(packets.len(), 1, "expected one packet, got {packets:#?}");
        let batch = packets[0]
            .as_array()
            .unwrap_or_else(|| panic!("the aggregates were not batched: {packets:#?}"));
        assert_eq!(batch.len(), 2, "expected both aggregates in the batch");
        assert!(batch.iter().all(|request| {
            request["method"] == "eth_call"
                // Both entries are `aggregate3` calls to `Multicall3`.
                && request["params"][0]["to"]
                    .as_str()
                    .unwrap()
                    .eq_ignore_ascii_case("0xcA11bde05977b3631167028862bE2a173976CA11")
        }));
    }

    /// Negative control for the test above: the same two aggregates sent
    /// through [`unbuffered_provider()`], which is the same stack without
    /// [`BatchCallLayer`], arrive as two separate single requests. So the
    /// batching seen above comes from our layer and not from alloy.
    #[tokio::test]
    async fn multicall3_is_not_batched_by_alloy_itself() {
        let node = Recorder::start().await;
        let (provider, _wallet) = unbuffered_provider(&node.url, Some("test"));

        let first = provider.multicall().get_block_number().get_chain_id();
        let second = provider.multicall().get_block_number().get_chain_id();
        let _ = futures::join!(first.aggregate3(), second.aggregate3());

        let packets = node.eth_call_packets();
        assert_eq!(packets.len(), 2, "expected two packets, got {packets:#?}");
        assert!(
            packets.iter().all(|packet| packet.is_object()),
            "expected single requests, got {packets:#?}"
        );
    }
}
