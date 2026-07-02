use {super::Finalize, std::io::Write};

/// A [`Write`] adapter that makes its inner writer best-effort: on the first
/// error it logs once and drops the inner, after which writes are accepted as
/// no-ops. Lets a non-critical sink fall out of a tee without aborting the
/// sinks that must finish.
pub(super) struct BestEffortSink<W>(Option<W>);

impl<W> BestEffortSink<W> {
    pub(super) fn new(inner: W) -> Self {
        Self(Some(inner))
    }
}

impl<W: Write> Write for BestEffortSink<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if let Some(inner) = &mut self.0
            && let Err(err) = inner.write_all(data)
        {
            // The sink was declared best-effort, so its failure is non-critical:
            // log it, stop writing to it, and let the remaining sinks carry on.
            tracing::debug!(?err, "best-effort sink failed; dropping it");
            self.0 = None;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(inner) = &mut self.0 {
            let _ = inner.flush();
        }
        Ok(())
    }
}

impl<W: Finalize> Finalize for BestEffortSink<W> {
    fn finalize(self) {
        if let Some(inner) = self.0 {
            inner.finalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A failing inner must not surface as an error to the caller.
    #[test]
    fn swallows_inner_failure() {
        struct Failing;
        impl Write for Failing {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone"))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = BestEffortSink::new(Failing);
        assert_eq!(writer.write(b"hello").unwrap(), 5);
        assert_eq!(writer.write(b"world").unwrap(), 5);
        writer.flush().unwrap();
    }
}
