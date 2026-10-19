use {
    prometheus_client::{
        encoding::{EncodeLabelSet, EncodeLabelValue, LabelValueEncoder},
        metrics::{counter::Counter, family::Family},
    },
    std::sync::OnceLock,
};

#[derive(Default)]
struct Metrics {
    refunding_loops: Family<LoopLabels, Counter>,
}

impl Metrics {
    fn get() -> &'static Self {
        static METRICS: OnceLock<Metrics> = OnceLock::new();
        METRICS.get_or_init(|| {
            let metrics = Self::default();
            observe::client_metrics::register(|registry| {
                registry.sub_registry_with_prefix("main").register(
                    "refunding_loops",
                    "Tracks the result of every refunding loops",
                    metrics.refunding_loops.clone(),
                );
            });
            metrics
        })
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct LoopLabels {
    result: LoopResult,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum LoopResult {
    Success,
    Error,
}

impl EncodeLabelValue for LoopResult {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> std::fmt::Result {
        let value = match self {
            Self::Success => "success",
            Self::Error => "error",
        };
        EncodeLabelValue::encode(&value, encoder)
    }
}

pub(crate) fn track_refunding_loop_result(result: LoopResult) {
    Metrics::get()
        .refunding_loops
        .get_or_create(&LoopLabels { result })
        .inc();
}
