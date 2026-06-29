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

const CHUNK_SIZE: usize = 64 * 1024;

/// Two is enough for double buffering: the consumer transmits one block while
/// the serializer produces the next; more only raises the memory ceiling.
const CHANNEL_CAPACITY: usize = 2;

/// Serializes `value` to JSON on a blocking thread, streaming it into the
/// returned reqwest body. Backpressure from the body channel caps memory.
pub fn stream_body<T>(value: T) -> reqwest::Body
where
    T: serde::Serialize + Send + 'static,
{
    stream_into(value, std::io::sink())
}

/// Like [`stream_body`], but also gzips the same serialization in one pass. The
/// receiver yields the compressed bytes once serialization finishes.
pub fn stream_body_and_gzip<T>(value: T) -> (reqwest::Body, oneshot::Receiver<Bytes>)
where
    T: serde::Serialize + Send + 'static,
{
    let (gzip, compressed) = GzipCapture::new();
    let body = stream_into(value, gzip);
    (body, compressed)
}

/// Serializes `value` into a tee of the body channel and `secondary` on a
/// blocking thread, then finalizes each sink.
fn stream_into<T, S>(value: T, secondary: S) -> reqwest::Body
where
    T: serde::Serialize + Send + 'static,
    S: Write + Finalize + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Bytes>(CHANNEL_CAPACITY);
    // `Measured` logs how long the solver took to drain the body once finished.
    let stream = Measured::new(ReceiverStream::new(rx)).map(Ok::<_, Infallible>);
    let body = reqwest::Body::wrap_stream(stream);
    // spawn_blocking loses the current span; carry it so the logs keep context.
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _guard = span.enter();
        let mut writer =
            BufWriter::with_capacity(CHUNK_SIZE, TeeWriter::new(ChannelWriter(tx), secondary));
        if let Err(err) = serde_json::to_writer(&mut writer, &value) {
            tracing::debug!(?err, "aborting streamed request body");
            return;
        }
        let tee = match writer.into_inner() {
            Ok(tee) => tee,
            Err(err) => {
                tracing::debug!(err = ?err.error(), "flushing streamed request body failed");
                return;
            }
        };
        tee.finalize();
    });
    body
}

/// A [`Write`] that sends each block to the body channel, blocking when it's
/// full so serialization can't outpace the request. Dropping it ends the body.
struct ChannelWriter(mpsc::Sender<Bytes>);

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
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

/// Consumes a sink once the body is fully written, running any cleanup (e.g.
/// finishing a gzip stream). Only called on success; an aborted serialization
/// drops the sink instead.
trait Finalize {
    fn finalize(self);
}

impl Finalize for std::io::Sink {
    fn finalize(self) {}
}

impl Finalize for ChannelWriter {
    /// Dropping the sender closes the channel, ending the request body.
    fn finalize(self) {}
}

/// A [`Write`] sink that gzips into memory and delivers the bytes over a
/// oneshot when finalized.
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
    /// A dropped receiver just means the archive was no longer wanted.
    fn finalize(self) {
        match self.writer.finish() {
            Ok(compressed) => {
                let _ = self.tx.send(Bytes::from(compressed));
            }
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
