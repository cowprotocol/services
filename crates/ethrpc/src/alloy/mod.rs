mod buffering;
pub mod conversions;
mod instrumentation;

pub use instrumentation::ProviderLabelingExt;
use {
    crate::AlloyProvider,
    alloy::{
        providers::{Provider, ProviderBuilder},
        rpc::client::ClientBuilder,
    },
    buffering::BatchCallLayer,
    instrumentation::{InstrumentationLayer, LabelingLayer},
};
#[cfg(any(test, feature = "test-util"))]
use {
    alloy::{network::EthereumWallet, providers::mock, signers::local::PrivateKeySigner},
    ethcontract::PrivateKey,
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
pub fn provider_with_account(url: &str, private_key: &PrivateKey) -> anyhow::Result<AlloyProvider> {
    let rpc = ClientBuilder::default()
        .layer(LabelingLayer {
            label: "main".into(),
        })
        .layer(InstrumentationLayer)
        .http(url.parse()?);

    let signer = PrivateKeySigner::from_slice(&private_key.secret_bytes())?;
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
