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
    /// Wraps an HTTP request body stream and logs, once the body has been fully
    /// drained, how long it took to hand off to the network. Useful to tell
    /// whether a slow round-trip is dominated by us sending a large (multi-MB)
    /// body or by the remote being slow to read it.
    ///
    /// `to_transmission_start_ms` is the gap between construction and the first
    /// poll (how long until the HTTP client started reading the body);
    /// `transmission_ms` spans that first poll until the body is exhausted.
    ///
    /// The numbers are an approximation: a poll only reflects when `hyper`
    /// pulled the chunk into the network stack's buffer, not when the bytes
    /// actually hit the wire. For bodies large enough to require several buffer
    /// flushes that approximation is close enough to be useful.
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
