mod buffering;
pub mod conversions;
mod instrumentation;

mod wallet;

use {
    crate::AlloyProvider,
    alloy::{
        network::EthereumWallet,
        providers::{Provider, ProviderBuilder},
        rpc::client::{ClientBuilder, RpcClient},
    },
    buffering::BatchCallLayer,
    instrumentation::{InstrumentationLayer, LabelingLayer},
};
pub use {conversions::Account, instrumentation::ProviderLabelingExt, wallet::MutWallet};

/// Creates an [`RpcClient`] from the given URL with [`LabelingLayer`],
/// [`InstrumentationLayer`] and [`BatchCallLayer`].
fn rpc(url: &str) -> RpcClient {
    ClientBuilder::default()
        .layer(LabelingLayer {
            label: "main".into(),
        })
        .layer(InstrumentationLayer)
        .layer(BatchCallLayer::new(Default::default()))
        .http(url.parse().unwrap())
}

/// Creates a provider with the provided URL and an empty [`MutWallet`].
///
/// Returns a copy of the [`MutWallet`] so the caller can modify it later.
pub fn provider(url: &str) -> (AlloyProvider, MutWallet) {
    let rpc = rpc(url);
    let wallet = MutWallet::default();
    let provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        // will query the node for the nonce every time that it is needed
        // adds overhead but makes working with alloy/ethcontract at the same time much simpler
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
    /// Creates a new provider with the given signer.
    fn with_signer(&self, signer: Account) -> Self;
}

impl ProviderSignerExt for AlloyProvider {
    fn with_signer(&self, signer: Account) -> Self {
        let Account::Signer(signer) = signer else {
            // Otherwise an unlocked account is used, not need to change anything.
            return self.clone();
        };

        let is_local = self.client().is_local();
        let transport = self.client().transport().clone();
        let wallet = EthereumWallet::new(signer);
        let client = RpcClient::with_random_id(transport, is_local);

        ProviderBuilder::new()
            .wallet(wallet)
            .with_simple_nonce_management()
            .connect_client(client)
            .erased()
    }
}

#[cfg(feature = "test-util")]
mod test_util {
    use {
        super::*,
        alloy::{
            contract::{CallBuilder, CallDecoder},
            primitives::TxHash,
            providers::Network,
            rpc::types::TransactionRequest,
        },
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

use alloy::{rpc::client::RpcClientInner, transports::IntoBoxTransport};
#[cfg(feature = "test-util")]
pub use test_util::{CallBuilderExt, ProviderExt};
