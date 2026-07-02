use {super::Finalize, std::io::Write};

/// A [`Write`] that fans every write out to two writers (à la UNIX `tee`), so
/// one serialization pass feeds two sinks — a request body and a gzip copy. A
/// write succeeds only if it succeeds on both; wrap a sink to make it
/// best-effort if its failure shouldn't abort the other.
pub(super) struct TeeWriter<A, B> {
    primary: A,
    secondary: B,
}

impl<A, B> TeeWriter<A, B> {
    pub(super) fn new(primary: A, secondary: B) -> Self {
        Self { primary, secondary }
    }
}

impl<A: Finalize, B: Finalize> Finalize for TeeWriter<A, B> {
    fn finalize(self) {
        self.primary.finalize();
        self.secondary.finalize();
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
