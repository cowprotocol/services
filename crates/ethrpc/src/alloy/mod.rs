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
