//! Streams a serializable value into a reqwest request body so the full (and
//! potentially multi-MB) serialized payload is never held in memory at once.
//!
//! [`stream_body_and_gzip`] additionally captures a gzip-compressed copy of the
//! same serialization (for S3 archival) without a second pass: it tees the
//! serializer output into the request-body channel and a gzip encoder, so only
//! the (much smaller) compressed result is retained.

use {
    bytes::{BufMut, Bytes, BytesMut},
    flate2::{Compression, write::GzEncoder},
    std::io::Write,
    tokio::sync::{mpsc, oneshot},
    tokio_stream::wrappers::ReceiverStream,
};

const CHUNK_SIZE: usize = 64 * 1024;
const CHANNEL_CAPACITY: usize = 8;

/// Serializes `value` to JSON on a blocking thread and streams it into a
/// reqwest body through a bounded channel, so the full (potentially multi-MB)
/// payload is never materialized at once. Peak body memory is roughly
/// `CHANNEL_CAPACITY * CHUNK_SIZE`: when reqwest hasn't yet drained the
/// channel, `blocking_send` parks the serializing thread, applying
/// backpressure.
pub fn stream_body<T>(value: T) -> reqwest::Body
where
    T: serde::Serialize + Send + 'static,
{
    let (tx, body) = body_channel();
    tokio::task::spawn_blocking(move || {
        let mut writer = ChannelWriter::new(tx);
        if let Err(err) = serde_json::to_writer(&mut writer, &value) {
            tracing::debug!(?err, "aborting streamed request body");
            return;
        }
        if let Err(err) = writer.flush() {
            tracing::debug!(?err, "aborting streamed request body");
        }
    });
    body
}

/// Like [`stream_body`], but also gzip-compresses the same serialization into
/// an in-memory buffer, delivered through the returned receiver once
/// serialization finishes. The gzip sink only ever touches memory (never the
/// network), so it can't backpressure the request body; the request is
/// throttled solely by reqwest draining the channel.
///
/// The receiver resolves with an error (sender dropped) if serialization or the
/// request body aborts, in which case there is nothing worth archiving.
pub fn stream_body_and_gzip<T>(value: T) -> (reqwest::Body, oneshot::Receiver<Bytes>)
where
    T: serde::Serialize + Send + 'static,
{
    let (tx, body) = body_channel();
    let (gzip_tx, gzip_rx) = oneshot::channel();
    tokio::task::spawn_blocking(move || {
        let mut http = ChannelWriter::new(tx);
        let mut gzip = GzEncoder::new(Vec::new(), Compression::new(3));
        {
            let mut tee = TeeWriter {
                primary: &mut http,
                secondary: &mut gzip,
            };
            if let Err(err) = serde_json::to_writer(&mut tee, &value) {
                tracing::debug!(?err, "aborting streamed request body");
                return;
            }
        }
        if let Err(err) = http.flush() {
            tracing::debug!(?err, "aborting streamed request body");
            return;
        }
        match gzip.finish() {
            // A dropped receiver just means archival was no longer wanted.
            Ok(compressed) => drop(gzip_tx.send(Bytes::from(compressed))),
            Err(err) => tracing::debug!(?err, "gzip of archived request body failed"),
        }
    });
    (body, gzip_rx)
}

fn body_channel() -> (mpsc::Sender<std::io::Result<Bytes>>, reqwest::Body) {
    let (tx, rx) = mpsc::channel::<std::io::Result<Bytes>>(CHANNEL_CAPACITY);
    (tx, reqwest::Body::wrap_stream(ReceiverStream::new(rx)))
}

/// A [`Write`] that forwards data to an mpsc channel in `chunk_size` pieces,
/// blocking the current (blocking) thread when the channel is full.
struct ChannelWriter {
    tx: mpsc::Sender<std::io::Result<Bytes>>,
    buf: BytesMut,
    chunk_size: usize,
}

impl ChannelWriter {
    fn new(tx: mpsc::Sender<std::io::Result<Bytes>>) -> Self {
        Self {
            tx,
            buf: BytesMut::with_capacity(CHUNK_SIZE),
            chunk_size: CHUNK_SIZE,
        }
    }

    fn send(&self, chunk: Bytes) -> std::io::Result<()> {
        self.tx.blocking_send(Ok(chunk)).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "request body receiver dropped",
            )
        })
    }
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.put_slice(data);
        while self.buf.len() >= self.chunk_size {
            let chunk = self.buf.split_to(self.chunk_size).freeze();
            self.send(chunk)?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buf.is_empty() {
            let chunk = std::mem::take(&mut self.buf).freeze();
            self.send(chunk)?;
        }
        Ok(())
    }
}

/// A [`Write`] that forwards every write to two underlying writers. A write
/// succeeds only if it succeeds on both.
struct TeeWriter<A, B> {
    primary: A,
    secondary: B,
}

impl<A: Write, B: Write> Write for TeeWriter<A, B> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.primary.write_all(data)?;
        self.secondary.write_all(data)?;
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.primary.flush()?;
        self.secondary.flush()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, flate2::read::GzDecoder, serde_json::json, std::io::Read};

    /// `ChannelWriter` must forward every byte, split into `chunk_size` pieces
    /// with the remainder flushed last, so the streamed body is byte-identical
    /// to a buffered serialization.
    #[tokio::test]
    async fn channel_writer_chunks_and_forwards_all_bytes() {
        const CHUNK: usize = 1024;
        let data = vec![7u8; CHUNK * 2 + 123];

        let (tx, mut rx) = mpsc::channel::<std::io::Result<Bytes>>(8);
        let expected = data.clone();
        let writer = tokio::task::spawn_blocking(move || {
            let mut writer = ChannelWriter {
                tx,
                buf: BytesMut::with_capacity(CHUNK),
                chunk_size: CHUNK,
            };
            writer.write_all(&data).unwrap();
            writer.flush().unwrap();
        });

        let mut chunks = Vec::new();
        while let Some(chunk) = rx.recv().await {
            chunks.push(chunk.unwrap());
        }
        writer.await.unwrap();

        // two full chunks then the remainder
        assert_eq!(
            chunks.iter().map(|c| c.len()).collect::<Vec<_>>(),
            vec![CHUNK, CHUNK, 123]
        );
        let reassembled: Vec<u8> = chunks.into_iter().flatten().collect();
        assert_eq!(reassembled, expected);
    }

    /// The tee must hand identical bytes to both sinks.
    #[test]
    fn tee_writer_forwards_to_both_sinks() {
        let data = b"the quick brown fox jumps over the lazy dog".repeat(100);
        let mut plain = Vec::new();
        let mut gzip = GzEncoder::new(Vec::new(), Compression::new(3));
        {
            let mut tee = TeeWriter {
                primary: &mut plain,
                secondary: &mut gzip,
            };
            tee.write_all(&data).unwrap();
        }
        let compressed = gzip.finish().unwrap();

        assert_eq!(plain, data);
        let mut decoded = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, data);
    }

    /// The gzip capture must compress exactly the serialized JSON.
    #[tokio::test]
    async fn gzip_capture_matches_serialized_value() {
        let value = json!({ "a": 1, "b": [1, 2, 3], "c": "hello" });
        // Keep `body` alive so its receiver isn't dropped (which would break the
        // pipe and abort serialization before the gzip is captured).
        let (_body, gzip_rx) = stream_body_and_gzip(value.clone());

        let compressed = gzip_rx.await.unwrap();
        let mut decoded = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, serde_json::to_vec(&value).unwrap());
    }
}
