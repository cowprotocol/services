use {
    futures::future::FusedFuture,
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
            state: State::NeverPolled,
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
        state: State,
    }
}

#[derive(Debug)]
enum State {
    NeverPolled,
    #[allow(dead_code)]
    Running(prometheus::HistogramTimer),
    Done,
}

impl<T: Future> Future for Measurable<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if matches!(this.state, State::NeverPolled) {
            *this.state = State::Running(
                Metrics::get()
                    .future_execution_times
                    .with_label_values(&[this.label])
                    .start_timer(),
            );
        }
        let result = this.inner.poll(cx);
        if result.is_ready() {
            // Dropping the timer will record the execution time.
            *this.state = State::Done;
        }
        result
    }
}

impl<T: FusedFuture> FusedFuture for Measurable<T> {
    fn is_terminated(&self) -> bool {
        matches!(self.state, State::Done)
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
