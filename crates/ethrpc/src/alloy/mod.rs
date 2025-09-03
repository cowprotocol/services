mod buffering;
pub mod conversions;
mod instrumentation;

#[cfg(any(test, feature = "test-util"))]
use alloy::{network::EthereumWallet, providers::mock};
pub use instrumentation::ProviderLabelingExt;
use {
    crate::AlloyProvider,
    alloy::{
        network::TxSigner,
        primitives::Signature,
        providers::{Provider, ProviderBuilder},
        rpc::client::ClientBuilder,
    },
    buffering::BatchCallLayer,
    instrumentation::{InstrumentationLayer, LabelingLayer},
};

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
pub fn provider_with_signer(
    url: &str,
    signer: Box<dyn TxSigner<Signature> + Send + Sync + 'static>,
) -> anyhow::Result<AlloyProvider> {
    let rpc = ClientBuilder::default()
        .layer(LabelingLayer {
            label: "main".into(),
        })
        .layer(InstrumentationLayer)
        .http(url.parse()?);
    let wallet = EthereumWallet::new(signer);

    Ok(ProviderBuilder::new()
        .wallet(wallet)
        .connect_client(rpc)
        .erased())
}

#[cfg(any(test, feature = "test-util"))]
pub fn dummy_provider() -> AlloyProvider {
    let asserter = mock::Asserter::new();
    ProviderBuilder::new()
        .connect_mocked_client(asserter)
        .erased()
}
