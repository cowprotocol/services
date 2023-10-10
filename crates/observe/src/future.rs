use {
    pin_project_lite::pin_project,
    std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    },
};

pub trait Measure: Sized {
    fn measure(self, label: &str) -> Measurable<Self> {
        Measurable {
            inner: self,
            label: label.to_owned(),
            timer: None,
        }
    }
}

impl<T: Sized> Measure for T {}

pin_project! {
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Measurable<T> {
        #[pin]
        inner: T,
        label: String,
        timer: Option<prometheus::HistogramTimer>,
    }
}

impl<T: Future> Future for Measurable<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if this.timer.is_none() {
            *this.timer = Some(
                Metrics::get()
                    .future_execution_times
                    .with_label_values(&[this.label])
                    .start_timer(),
            );
        }
        this.inner.poll(cx)
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Timing of measured futures.
    #[metric(labels("label"))]
    future_execution_times: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(super::metrics::get_storage_registry()).unwrap()
    }
}
