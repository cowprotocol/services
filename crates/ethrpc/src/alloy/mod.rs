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
        // The random ID mitigates the possibility of duplicate request IDs
        // between providers when batching; furthemore, since we're
        // using a uniform distribution we need to be aware that we
        // might get a value close enough to u64::MAX to overflow after
        // a couple requests, to solve that we generate a u32 first and
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
        alloy_primitives::{Address, TxHash, address, utils::parse_ether},
        alloy_provider::{
            MULTICALL3_ADDRESS,
            Network,
            ext::{AnvilApi, ImpersonateConfig},
        },
        alloy_rpc_types::TransactionRequest,
        anyhow::{Context, ensure},
        contracts::Multicall3,
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

        /// Puts `Multicall3` where every network keeps it, which only a fresh
        /// local node needs: forked networks inherit the real deployment.
        /// Liquidity sources batch their pool reads through it, so without it
        /// they cannot read anything.
        fn deploy_multicall3(&self) -> impl Future<Output = anyhow::Result<()>>;
    }

    impl ProviderExt for AlloyProvider {
        async fn send_and_watch(&self, tx: TransactionRequest) -> anyhow::Result<TxHash> {
            let pending = self.send_transaction(tx).await?;
            let result = timeout(DEFAULT_WATCH_TIMEOUT, pending.watch()).await??;
            Ok(result)
        }

        async fn deploy_multicall3(&self) -> anyhow::Result<()> {
            // The canonical address is the one the original deployer got from
            // its very first transaction, so replaying that
            // transaction is the only way to land there. It works
            // because a fresh node still has that account at nonce 0.
            const DEPLOYER: Address = address!("0x05f32B3cC3888453ff71B01135B34FF8e41263F2");

            if !self
                .get_code_at(MULTICALL3_ADDRESS)
                .await
                .context("could not fetch Multicall3 code")?
                .is_empty()
            {
                return Ok(());
            }

            // A wallet-less provider makes alloy hand the transaction to the
            // node for signing instead of looking for a key we do
            // not have.
            let deployment = Multicall3::Instance::deploy_builder(self.without_wallet())
                .from(DEPLOYER)
                .into_transaction_request();

            self.anvil_send_impersonated_transaction_with_config(
                deployment,
                ImpersonateConfig {
                    fund_amount: Some(parse_ether("1").expect("valid ETH amount")),
                    stop_impersonate: true,
                },
            )
            .await
            .context("failed to deploy Multicall3")?
            .watch()
            .await
            .context("Multicall3 deployment was not mined")?;

            ensure!(
                !self
                    .get_code_at(MULTICALL3_ADDRESS)
                    .await
                    .context("could not fetch Multicall3 code")?
                    .is_empty(),
                "Multicall3 did not end up at {MULTICALL3_ADDRESS}"
            );

            Ok(())
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
