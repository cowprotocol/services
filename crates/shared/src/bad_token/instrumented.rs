use super::{BadTokenDetecting, TokenQuality};
use crate::metrics::get_metric_storage_registry;
use anyhow::Result;
use prometheus::IntCounterVec;
use prometheus_metric_storage::MetricStorage;

pub trait InstrumentedBadTokenDetectorExt {
    fn instrumented(self) -> InstrumentedBadTokenDetector;
}

impl<T: BadTokenDetecting + 'static> InstrumentedBadTokenDetectorExt for T {
    fn instrumented(self) -> InstrumentedBadTokenDetector {
        InstrumentedBadTokenDetector {
            inner: Box::new(self),
        }
    }
}

#[derive(MetricStorage, Clone, Debug)]
#[metric(subsystem = "token_quality")]
struct Metrics {
    /// Tracks how many token detections result in good or bad token quality or an error.
    #[metric(labels("quality"))]
    results: IntCounterVec,
}

pub struct InstrumentedBadTokenDetector {
    inner: Box<dyn BadTokenDetecting>,
}

#[async_trait::async_trait]
impl BadTokenDetecting for InstrumentedBadTokenDetector {
    async fn detect(&self, token: ethcontract::H160) -> Result<TokenQuality> {
        let result = self.inner.detect(token).await;

        let label = match &result {
            Ok(TokenQuality::Good) => "good",
            // prometheus isn't very good for string based data so we simply log the bad
            // tokens/errors and get the information from Kibana when we need it.
            Err(err) => {
                tracing::warn!(
                    "bad token detection for {:?} returned error:\n{:?}",
                    token,
                    err
                );
                "error"
            }
            Ok(quality @ TokenQuality::Bad { .. }) => {
                tracing::warn!("bad token detection for {:?} returned {:?}", token, quality);
                "bad"
            }
        };

        Metrics::instance(get_metric_storage_registry())
            .expect("unexpected error getting metrics instance")
            .results
            .with_label_values(&[label])
            .inc();

        result
    }
}
