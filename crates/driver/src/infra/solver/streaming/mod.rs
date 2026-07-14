mod best_effort_sink;
mod tee_writer;
mod timed_writer;

use {
    best_effort_sink::BestEffortSink,
    bytes::Bytes,
    futures::StreamExt,
    observe::http_body::Measured,
    std::{
        convert::Infallible,
        io::{BufWriter, Write},
        time::Duration,
    },
    tee_writer::TeeWriter,
    timed_writer::Timed,
    tokio::sync::{mpsc, oneshot},
    tokio_stream::wrappers::ReceiverStream,
};

const CHUNK_SIZE: usize = 64 * 1024;

/// Two is enough for double buffering: the consumer transmits one block while
/// the serializer produces the next; more only raises the memory ceiling.
const CHANNEL_CAPACITY: usize = 2;

/// Timing measurements captured while streaming a request body, delivered once
/// serialization finishes. The caller decides what to do with them (e.g. expose
/// the serialization overhead as a metric for real auctions but not quotes).
pub struct Measurements {
    /// Wall-clock serialization cost, isolated from the time spent blocked
    /// writing to the sinks (solver transfer + gzip).
    pub serialize: Duration,
    /// Total wall-clock of the streaming task: serialization plus the time
    /// spent blocked writing to the sinks (solver transfer + gzip).
    pub total: Duration,
}

/// Serializes `value` to JSON on a blocking thread, streaming it into the
/// returned reqwest body. Backpressure from the body channel caps memory.
pub fn stream_body<T>(value: T) -> (reqwest::Body, oneshot::Receiver<Measurements>)
where
    T: serde::Serialize + Send + 'static,
{
    stream_into(value, std::io::sink())
}

/// Like [`stream_body`], but also gzips the same serialization in one pass. The
/// first receiver yields the compressed bytes once serialization finishes.
pub fn stream_body_and_gzip<T>(
    value: T,
) -> (
    reqwest::Body,
    oneshot::Receiver<Bytes>,
    oneshot::Receiver<Measurements>,
)
where
    T: serde::Serialize + Send + 'static,
{
    let (gzip, compressed) = GzipCapture::new();
    let (body, measurements) = stream_into(value, gzip);
    (body, compressed, measurements)
}

/// Serializes `value` into a tee of the body channel and `secondary` on a
/// blocking thread, then finalizes each sink. Serialization always runs to
/// completion even if the request receiver drops, so the secondary sink (the
/// gzip archive) is captured regardless of the request outcome. The returned
/// receiver reports the timing [`Measurements`] once serialization finishes.
fn stream_into<T, S>(value: T, secondary: S) -> (reqwest::Body, oneshot::Receiver<Measurements>)
where
    T: serde::Serialize + Send + 'static,
    S: Write + Finalize + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Bytes>(CHANNEL_CAPACITY);
    let (measurements_tx, measurements_rx) = oneshot::channel();
    // E2E measurement of the body transfer time (includes serialization)
    let stream = Measured::new(ReceiverStream::new(rx)).map(Ok::<_, Infallible>);
    let body = reqwest::Body::wrap_stream(stream);
    // spawn_blocking loses the current span; carry it so the logs keep context.
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _guard = span.enter();
        // We measure total time + Tee transfer time (includes network + gzip)
        // total time - Tee transfer time = serialization overhead
        let start = std::time::Instant::now();

        // If sending to solvers fails, we should still be able to upload to S3
        let channel = BestEffortSink::new(ChannelWriter(tx));
        // Note that `channel` writes to the solver, while secondary to the gzip
        // (in-memory) as such, writing delays *should* mostly be due to the
        // solver network transfer and not really the gzip; this will have the
        // secondary effect of slowing down *when* the S3 upload starts
        let timed_tee = Timed::new(TeeWriter::new(channel, secondary));
        let mut writer = BufWriter::with_capacity(CHUNK_SIZE, timed_tee);
        if let Err(err) = serde_json::to_writer(&mut writer, &value) {
            tracing::warn!(?err, "serializing streamed request body failed");
            return;
        }
        let timed = match writer.into_inner() {
            Ok(timed) => timed,
            Err(err) => {
                tracing::warn!(err = ?err.error(), "flushing streamed request body failed");
                return;
            }
        };
        // Measure before `finalize` so the gzip finish cost stays out of the metrics.
        let total = start.elapsed();
        let serialize = total.saturating_sub(timed.elapsed());
        let _ = measurements_tx.send(Measurements { serialize, total });
        timed.into_inner().finalize();
    });
    (body, measurements_rx)
}

/// A [`Write`] that sends each block to the body channel, blocking when it's
/// full so serialization can't outpace the request. Dropping it ends the body.
/// A dropped receiver (the solver connection went away) surfaces as an error;
/// wrap it in [`BestEffortSink`] to keep serializing the archive past that
/// point.
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

    /// `write` sends each block downstream immediately, so a write effectively
    /// always flushes and there's nothing buffered left to do here.
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
        let (_body, gzip_rx, _measurements) = stream_body_and_gzip(value.clone());

        let compressed = gzip_rx.await.unwrap();
        let mut decoded = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, serde_json::to_vec(&value).unwrap());
    }

    #[tokio::test]
    async fn gzip_captured_even_if_request_body_dropped() {
        let value = json!({ "a": 1, "b": [1, 2, 3], "c": "hello" });
        let (body, gzip_rx, _measurements) = stream_body_and_gzip(value.clone());
        // The solver connection going away mid-stream must not skip the archive.
        drop(body);

        let compressed = gzip_rx.await.unwrap();
        let mut decoded = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, serde_json::to_vec(&value).unwrap());
    }

    #[tokio::test]
    async fn reports_measurements_once_serialization_finishes() {
        let value = json!({ "a": 1, "b": [1, 2, 3], "c": "hello" });
        // Keep `body` alive so serialization runs to completion.
        let (_body, measurements) = stream_body(value);
        // The receiver resolves, confirming the caller can pick up the timings.
        measurements.await.unwrap();
    }
}
