mod buffering;
pub mod conversions;
mod instrumentation;

use {
    crate::AlloyProvider,
    alloy::{
        contract::{CallBuilder, CallDecoder},
        network::{EthereumWallet, Network},
        primitives::FixedBytes,
        providers::{Provider, ProviderBuilder},
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
    ProviderBuilder::new()
        // will query the node for the nonce every time that it is needed
        // adds overhead but makes working with alloy/ethcontract at the same time much simpler
        .with_simple_nonce_management()
        .connect_client(rpc)
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
            .with_simple_nonce_management()
            .connect_client(client)
            .erased()
    }
}

pub trait ProviderExt {
    /// Sends a transaction and watches it for completion.
    fn send_and_watch(
        &self,
        tx: TransactionRequest,
    ) -> impl Future<Output = anyhow::Result<TxHash>>;
}

impl ProviderExt for AlloyProvider {
    async fn send_and_watch(&self, tx: TransactionRequest) -> anyhow::Result<FTxHash> {
        Ok(self.send_transaction(tx).await?.watch().await?)
    }
}

pub trait CallBuilderExt<N>
where
    N: Network,
{
    fn send_and_watch(&self) -> impl Future<Output = anyhow::Result<TxHash>>;
}

impl<P: Provider<N>, D: CallDecoder, N: Network> CallBuilderExt<N> for CallBuilder<P, D, N> {
    async fn send_and_watch(&self) -> anyhow::Result<TxHash> {
        Ok(self.send().await?.watch().await?)
    }
}
