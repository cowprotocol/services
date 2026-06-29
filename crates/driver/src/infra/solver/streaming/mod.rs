//! Streams a serializable value into a reqwest body so the full (multi-MB)
//! payload is never held in memory at once.
//!
//! A [`BufWriter`] coalesces the serializer's many tiny writes into
//! `CHUNK_SIZE` blocks, which a [`TeeWriter`] then fans out to the body channel
//! and an optional secondary sink. Both entry points share one core
//! ([`stream_into`]) and differ only in that secondary: [`stream_body`]
//! discards it, [`stream_body_and_gzip`] captures a gzip copy for S3.

mod tee_writer;

use {
    bytes::Bytes,
    futures::StreamExt,
    observe::http_body::Measured,
    std::{
        convert::Infallible,
        io::{BufWriter, Write},
    },
    tee_writer::TeeWriter,
    tokio::sync::{mpsc, oneshot},
    tokio_stream::wrappers::ReceiverStream,
};

/// Block size the serializer's writes are coalesced into before being chunked
/// onto the body channel and fed to the gzip encoder.
const CHUNK_SIZE: usize = 64 * 1024;

/// Blocks buffered in the body channel before the serializing thread is parked.
/// Two is enough for double buffering: the consumer can transmit one block
/// while the serializer produces the next. A larger value would only raise the
/// memory ceiling (~`CHANNEL_CAPACITY * CHUNK_SIZE`) without improving
/// throughput (the gzip archive copy, when enabled, is retained separately).
const CHANNEL_CAPACITY: usize = 2;

/// Serializes `value` to JSON on a blocking thread, streaming it into the
/// returned reqwest body. Backpressure from the body channel caps memory.
pub fn stream_body<T>(value: T) -> reqwest::Body
where
    T: serde::Serialize + Send + 'static,
{
    stream_into(value, std::io::sink())
}

/// Like [`stream_body`], but also gzips the same serialization for S3 in one
/// pass. The receiver yields the compressed bytes once serialization finishes,
/// or errors (sender dropped) if it aborts — leaving nothing to archive.
pub fn stream_body_and_gzip<T>(value: T) -> (reqwest::Body, oneshot::Receiver<Bytes>)
where
    T: serde::Serialize + Send + 'static,
{
    let (gzip, compressed) = GzipCapture::new();
    let body = stream_into(value, gzip);
    (body, compressed)
}

/// Serializes `value` into a tee of the body channel and `secondary` on a
/// blocking thread, then finalizes each sink. Shared by both entry points.
fn stream_into<T, S>(value: T, secondary: S) -> reqwest::Body
where
    T: serde::Serialize + Send + 'static,
    S: Write + Finalize + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Bytes>(CHANNEL_CAPACITY);
    // The channel carries raw `Bytes`; `wrap_stream` wants a `TryStream`, so the
    // chunks are wrapped into infallible `Ok`s at the boundary. `Measured` logs
    // how long the solver took to read the body once it is fully drained.
    let stream = Measured::new(ReceiverStream::new(rx)).map(Ok::<_, Infallible>);
    let body = reqwest::Body::wrap_stream(stream);
    // `spawn_blocking` doesn't inherit the caller's span, so carry it across so
    // the diagnostics below keep the auction/request context.
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _guard = span.enter();
        let mut writer =
            BufWriter::with_capacity(CHUNK_SIZE, TeeWriter::new(ChannelWriter(tx), secondary));
        if let Err(err) = serde_json::to_writer(&mut writer, &value) {
            tracing::debug!(?err, "aborting streamed request body");
            return;
        }
        // `into_inner` flushes the final block into the tee; on error the body
        // channel is dropped (truncating the request) and `secondary` with it.
        let tee = match writer.into_inner() {
            Ok(tee) => tee,
            Err(err) => {
                tracing::debug!(err = ?err.error(), "flushing streamed request body failed");
                return;
            }
        };
        let (channel, secondary) = tee.into_parts();
        // Close the body so reqwest can finish; finalize the copy separately.
        drop(channel);
        secondary.finalize();
    });
    body
}

/// A [`Write`] that sends each block to the body channel, blocking when it's
/// full so serialization can't outpace the request (backpressure). Dropping it
/// closes the channel, signalling the end of the body.
struct ChannelWriter(mpsc::Sender<Bytes>);

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        // `copy_from_slice` copies each block a second time (after `BufWriter`'s
        // own buffering). If this ever shows up in a profile, a `BytesMut`-backed
        // writer that chunks and `freeze()`s could hand the channel owned bytes
        // without the copy — at the cost of reimplementing the coalescing
        // `BufWriter` gives us for free.
        self.0
            .blocking_send(Bytes::copy_from_slice(data))
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "request body receiver dropped",
                )
            })?;
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A consume-by-value cleanup for the tee's secondary sink, run after the full
/// body has been written (e.g. finishing a gzip stream and handing off the
/// result). [`stream_into`] invokes it only on success; an aborted
/// serialization drops the sink without calling it.
trait Finalize {
    fn finalize(self);
}

/// No-op secondary for [`stream_body`].
impl Finalize for std::io::Sink {
    fn finalize(self) {}
}

/// A [`Write`] sink that gzips into memory and delivers the bytes over a
/// oneshot when finalized. The gzip settings live in the `s3` crate so the
/// archived copy is compressed identically to the eager upload path.
struct GzipCapture {
    writer: s3::GzipWriter,
    tx: oneshot::Sender<Bytes>,
}

impl GzipCapture {
    fn new() -> (Self, oneshot::Receiver<Bytes>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                writer: s3::GzipWriter::new(),
                tx,
            },
            rx,
        )
    }
}

impl Finalize for GzipCapture {
    /// Finishes the gzip stream and sends the bytes. A dropped receiver just
    /// means archival was no longer wanted.
    fn finalize(self) {
        match self.writer.finish() {
            Ok(compressed) => drop(self.tx.send(Bytes::from(compressed))),
            Err(err) => tracing::debug!(?err, "gzip of archived request body failed"),
        }
    }
}

impl Write for GzipCapture {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.writer.write(data)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, flate2::read::GzDecoder, serde_json::json, std::io::Read};

    /// The streamed body must reassemble to exactly the serialized JSON.
    #[tokio::test]
    async fn channel_writer_forwards_all_bytes() {
        let data = vec![7u8; CHUNK_SIZE * 2 + 123];
        let (tx, mut rx) = mpsc::channel::<Bytes>(CHANNEL_CAPACITY);
        let expected = data.clone();
        let writer = tokio::task::spawn_blocking(move || {
            let mut writer = BufWriter::with_capacity(CHUNK_SIZE, ChannelWriter(tx));
            writer.write_all(&data).unwrap();
            writer.flush().unwrap();
        });

        let mut reassembled = Vec::new();
        while let Some(chunk) = rx.recv().await {
            reassembled.extend_from_slice(&chunk);
        }
        writer.await.unwrap();

        assert_eq!(reassembled, expected);
    }

    /// The gzip capture must compress exactly the serialized JSON.
    #[tokio::test]
    async fn gzip_capture_matches_serialized_value() {
        let value = json!({ "a": 1, "b": [1, 2, 3], "c": "hello" });
        // Keep `body` alive: dropping its receiver would abort serialization
        // before the gzip is captured.
        let (_body, gzip_rx) = stream_body_and_gzip(value.clone());

        let compressed = gzip_rx.await.unwrap();
        let mut decoded = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, serde_json::to_vec(&value).unwrap());
    }
}
