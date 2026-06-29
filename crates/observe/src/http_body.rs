//! Instrumentation for outgoing HTTP request bodies.

use {
    futures::Stream,
    pin_project_lite::pin_project,
    std::{
        pin::Pin,
        task::{Context, Poll},
        time::Instant,
    },
};

pin_project! {
    /// Wraps an HTTP request body stream and, once it is fully drained, logs how
    /// long transmission took: `to_transmission_start_ms` (construction until
    /// the first poll, i.e. until the client started reading) and
    /// `transmission_ms` (first poll until exhausted). Approximate — a poll
    /// reflects when `hyper` buffered the chunk, not when it hit the wire.
    pub struct Measured<S> {
        #[pin]
        inner: S,
        created_at: Instant,
        first_polled_at: Option<Instant>,
        span: tracing::Span,
    }
}

impl<S> Measured<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            created_at: Instant::now(),
            first_polled_at: None,
            span: tracing::Span::current(),
        }
    }
}

impl<S: Stream> Stream for Measured<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let this = self.project();
        let first_polled_at = *this.first_polled_at.get_or_insert_with(Instant::now);
        let poll = this.inner.poll_next(cx);
        if matches!(poll, Poll::Ready(None)) {
            let _span = this.span.enter();
            tracing::debug!(
                to_transmission_start_ms =
                    first_polled_at.duration_since(*this.created_at).as_millis(),
                transmission_ms = first_polled_at.elapsed().as_millis(),
                "finished streaming http request body"
            );
        }
        poll
    }
}

#[cfg(test)]
mod tests {
    use {super::*, bytes::Bytes, futures::StreamExt};

    #[tokio::test]
    async fn passes_items_through_unchanged() {
        let inner =
            futures::stream::iter(vec![Bytes::from_static(b"ab"), Bytes::from_static(b"cd")]);
        let collected: Vec<_> = Measured::new(inner).collect().await;
        assert_eq!(
            collected,
            vec![Bytes::from_static(b"ab"), Bytes::from_static(b"cd")]
        );
    }
}
