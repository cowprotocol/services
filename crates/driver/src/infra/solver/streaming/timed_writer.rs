use std::{
    io::Write,
    time::{Duration, Instant},
};

/// A [`Write`] wrapper that accumulates the wall-clock time spent inside the
/// inner writer's `write`/`flush` calls.
///
/// Placed *below* the serialization buffer, it captures everything the
/// serializer waits on downstream — solver backpressure (the body channel
/// blocks when the socket is slow to drain) and the gzip archival copy. That
/// lets the caller subtract this from the task's total time to isolate the
/// serialization cost, which network pacing would otherwise inflate.
pub(super) struct Timed<W> {
    inner: W,
    elapsed: Duration,
}

impl<W> Timed<W> {
    pub(super) fn new(inner: W) -> Self {
        Self {
            inner,
            elapsed: Duration::ZERO,
        }
    }

    /// Total time spent in downstream `write`/`flush` calls so far.
    pub(super) fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub(super) fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for Timed<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let start = Instant::now();
        let res = self.inner.write(data);
        self.elapsed += start.elapsed();
        res
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let start = Instant::now();
        let res = self.inner.flush();
        self.elapsed += start.elapsed();
        res
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::thread::sleep};

    /// A sink that sleeps on every write so we can assert the wrapper adds up
    /// the downstream time.
    struct Slow;

    impl Write for Slow {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            sleep(Duration::from_millis(5));
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn accumulates_downstream_time() {
        let mut writer = Timed::new(Slow);
        writer.write_all(b"ab").unwrap();
        writer.write_all(b"cd").unwrap();
        assert!(writer.elapsed() >= Duration::from_millis(10));
    }
}
