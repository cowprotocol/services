mod buffering;
pub mod conversions;
mod instrumentation;

pub use instrumentation::ProviderLabelingExt;
use {
    crate::AlloyProvider,
    alloy::{
        providers::{Provider, ProviderBuilder, mock},
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
    tracing::info!("newlog url={:?}", url);
    println!("newlog url={:?}", url);
    ProviderBuilder::new().connect_client(rpc).erased()
}

pub fn dummy_provider() -> AlloyProvider {
    let asserter = mock::Asserter::new();
    ProviderBuilder::new()
        .connect_mocked_client(asserter)
        .erased()
}
