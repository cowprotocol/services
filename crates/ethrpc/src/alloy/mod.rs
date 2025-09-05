mod buffering;
pub mod conversions;
mod instrumentation;

#[cfg(any(test, feature = "test-util"))]
use alloy::providers::mock;
use {
    crate::AlloyProvider,
    alloy::{
        contract::{CallBuilder, CallDecoder},
        network::{EthereumWallet, Network},
        primitives::FixedBytes,
        providers::{
            Provider,
            ProviderBuilder,
        },
        rpc::{
            client::{ClientBuilder, RpcClient},
            types::TransactionRequest,
        },
    },
    buffering::BatchCallLayer,
    instrumentation::{InstrumentationLayer, LabelingLayer},
};
pub use {conversions::Account, instrumentation::ProviderLabelingExt};

pub fn provider(url: &str) -> AlloyProvider {
    let rpc = ClientBuilder::default()
        .layer(LabelingLayer {
            label: "main".into(),
        })
        .layer(InstrumentationLayer)
        .layer(BatchCallLayer::new(Default::default()))
        .http(url.parse().unwrap());
    ProviderBuilder::new().connect_client(rpc).erased()
}

#[cfg(any(test, feature = "test-util"))]
pub fn dummy_provider() -> AlloyProvider {
    let asserter = mock::Asserter::new();
    ProviderBuilder::new()
        .connect_mocked_client(asserter)
        .erased()
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
            .connect_client(client)
            .erased()
    }
}

pub trait ProviderExt {
    /// Sends a transaction and watches it for completion.
    fn send_and_watch(
        &self,
        tx: TransactionRequest,
    ) -> impl Future<Output = anyhow::Result<FixedBytes<32>>>;
}

impl ProviderExt for AlloyProvider {
    fn send_and_watch(
        &self,
        tx: TransactionRequest,
    ) -> impl Future<Output = anyhow::Result<FixedBytes<32>>> {
        async move { Ok(self.send_transaction(tx).await?.watch().await?) }
    }
}

pub trait CallBuilderExt<N>
where
    N: Network,
{
    fn send_and_watch(&self) -> impl Future<Output = anyhow::Result<FixedBytes<32>>>;
}

impl<P: Provider<N>, D: CallDecoder, N: Network> CallBuilderExt<N> for CallBuilder<P, D, N> {
    fn send_and_watch(&self) -> impl Future<Output = anyhow::Result<FixedBytes<32>>> {
        async { Ok(self.send().await?.watch().await?) }
    }
}
