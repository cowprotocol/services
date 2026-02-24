use {
    bytes::Bytes,
    futures::Stream,
    std::{task::Poll, time::Instant},
};

/// Thin wrapper around a payload that already exists fully serialized
/// in-memory. The purpose of converting that into a stream is to allow
/// measuring the data transfer of the request body (for debugging and
/// optimization purposes).
///
/// Note that this measurement is only an approximation of the truth.
/// The reason is that this only measures how long it takes `hyper`
/// to load the last byte from the body into the buffer of the network
/// stack. However, given how big `/solve` requests are in practice
/// `hyper` should have to flush the buffer a couple of times so the
/// measured time should be reasonably accurate.
pub struct ByteStream {
    data: Bytes,
    created_at: Instant,
    first_polled_at: Option<Instant>,
    span: tracing::Span,
}

impl ByteStream {
    pub fn new(data: Bytes) -> Self {
        Self {
            data,
            created_at: Instant::now(),
            first_polled_at: None,
            span: tracing::Span::current(),
        }
    }
}

// Since `hyper` uses `Bytes` under the hood which are reference counted
// the chunks we yield can be as big as we want. To minimize overhead
// that's only there for debugging purposes we always yield all the
// data at once. The measurements will still be accurate because `hyper`
// has to poll the stream once more to confirm that it's actually
// exhausted.
impl Stream for ByteStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        if this.first_polled_at.is_none() {
            this.first_polled_at = Some(Instant::now());
        }

        if this.data.is_empty() {
            let _span = this.span.enter();
            let first_poll = this.first_polled_at.expect("initialized at first poll");
            tracing::debug!(
                to_transmission_start = ?first_poll.duration_since(this.created_at),
                transmission = ?first_poll.elapsed(),
                "finished streaming http request body"
            );
            Poll::Ready(None)
        } else {
            // steals all the data and leaves 0 bytes in self
            let chunk = std::mem::take(&mut this.data);
            Poll::Ready(Some(Ok(chunk)))
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, futures::FutureExt, tokio_stream::StreamExt};

    #[test]
    fn byte_stream_yields_bytes() {
        let original = Bytes::from_iter(0..100);
        let mut stream = ByteStream::new(original.clone());

        let chunk = stream
            .next()
            .now_or_never()
            .expect("stream is always ready");
        // stream always yields all the data on the first poll
        assert_eq!(chunk.unwrap().unwrap(), original);

        let chunk = stream
            .next()
            .now_or_never()
            .expect("stream is always ready");
        assert!(chunk.is_none());
    }
}
