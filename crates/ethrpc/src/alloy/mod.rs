mod buffering;
pub mod conversions;
mod instrumentation;

pub use instrumentation::ProviderLabelingExt;
use {
    crate::AlloyProvider,
    alloy::{
        network::EthereumWallet,
        providers::{Provider, ProviderBuilder, mock},
        rpc::client::ClientBuilder,
        signers::local::PrivateKeySigner,
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

pub fn provider_with_account(url: &str, private_key: &[u8; 32]) -> anyhow::Result<AlloyProvider> {
    let rpc = ClientBuilder::default()
        .layer(LabelingLayer {
            label: "main".into(),
        })
        .layer(InstrumentationLayer)
        .layer(BatchCallLayer::new(Default::default()))
        .http(url.parse().unwrap());

    let signer = PrivateKeySigner::from_slice(private_key)?;
    let wallet = EthereumWallet::new(signer);

    Ok(ProviderBuilder::new()
        .wallet(wallet)
        .connect_client(rpc)
        .erased())
}

pub fn dummy_provider() -> AlloyProvider {
    let asserter = mock::Asserter::new();
    ProviderBuilder::new()
        .connect_mocked_client(asserter)
        .erased()
}
