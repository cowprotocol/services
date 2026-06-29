use std::io::Write;

/// A [`Write`] that fans every write out to two writers (à la UNIX `tee`), so
/// one serialization pass feeds two sinks — a request body and a gzip copy. A
/// write succeeds only if it succeeds on both.
pub(super) struct TeeWriter<A, B> {
    primary: A,
    secondary: B,
}

impl<A, B> TeeWriter<A, B> {
    pub(super) fn new(primary: A, secondary: B) -> Self {
        Self { primary, secondary }
    }

    /// Splits the tee back into its two sinks so each can be finalized on its
    /// own.
    pub(super) fn into_parts(self) -> (A, B) {
        (self.primary, self.secondary)
    }
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
    use {
        super::*,
        flate2::{Compression, read::GzDecoder, write::GzEncoder},
        std::io::Read,
    };

    /// The tee must hand identical bytes to both sinks.
    #[test]
    fn forwards_to_both_sinks() {
        let data = b"the quick brown fox jumps over the lazy dog".repeat(100);
        let mut plain = Vec::new();
        let mut gzip = GzEncoder::new(Vec::new(), Compression::new(3));
        {
            let mut tee = TeeWriter::new(&mut plain, &mut gzip);
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
}
