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

        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB
        if this.data.is_empty() {
            let first_poll = this.first_polled_at.expect("initialized at first poll");
            let _span = this.span.enter();
            tracing::debug!(
                to_transmission_start = ?first_poll.duration_since(this.created_at),
                transmission = ?first_poll.elapsed(),
                "finished streaming http request body"
            );
            Poll::Ready(None)
        } else {
            let end_index = std::cmp::min(CHUNK_SIZE, this.data.len());
            // splits off the bytes to transmit and only leaves the remaining bytes in
            // `self.data`
            let chunk = this.data.split_to(end_index);
            Poll::Ready(Some(Ok(chunk)))
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, futures::FutureExt, tokio_stream::StreamExt};

    #[test]
    fn byte_stream_yields_bytes() {
        const SIZE: usize = 10 * 1024 * 1024 + 100; // 10.1 MB
        let original = Bytes::from_iter((0..SIZE).map(|byte| byte as u8));
        let mut stream = ByteStream::new(original.clone());
        let mut streamed_data = vec![];
        let mut times_polled = 0;
        loop {
            let poll = stream
                .next()
                .now_or_never()
                .expect("stream is always ready");
            times_polled += 1;
            if let Some(Ok(chunk)) = poll {
                streamed_data.extend_from_slice(&chunk);
            } else {
                break;
            }
        }

        assert_eq!(original.as_ref(), &streamed_data);
        // 10 1MB chunks, 1 100KB chunk, 1 poll to confirm the stream is done
        assert_eq!(times_polled, 12);
    }
}
