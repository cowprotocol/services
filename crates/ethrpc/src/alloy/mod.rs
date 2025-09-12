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
        let client = RpcClient::new(transport, is_local);

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

#[cfg(feature = "test-util")]
pub use test_util::{CallBuilderExt, ProviderExt};
