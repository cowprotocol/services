//! Instrumentation for outgoing HTTP request bodies.

use {
    futures::Stream,
    std::{
        pin::Pin,
        task::{Context, Poll},
        time::Instant,
    },
};

/// Wraps an HTTP request body stream and, once it's fully drained, logs how
/// long the hand-off to the network took — to tell whether a slow round-trip
/// is us sending a large body or the remote being slow to read it.
///
/// `to_transmission_start_ms` is construction to first poll (until the client
/// starts reading); `transmission_ms` is first poll to exhaustion.
pub struct Measured<S> {
    inner: S,
    created_at: Instant,
    first_polled_at: Option<Instant>,
    span: tracing::Span,
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

impl<S: Stream + Unpin> Stream for Measured<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let this = self.get_mut();
        let first_polled_at = *this.first_polled_at.get_or_insert_with(Instant::now);
        let poll = Pin::new(&mut this.inner).poll_next(cx);
        if matches!(poll, Poll::Ready(None)) {
            let _span = this.span.enter();
            tracing::debug!(
                to_transmission_start_ms =
                    first_polled_at.duration_since(this.created_at).as_millis(),
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
