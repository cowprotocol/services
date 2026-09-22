//! Allocation of the ids quotes are stored under.
//!
//! Ids are drawn from the database sequence the `quotes` table uses.
//! Drawing one per solver request would cost one database round trip per
//! request, so the allocator draws them in chunks and hands them out one at a
//! time.

use {
    crate::QuoteIdGenerating,
    anyhow::{Context, Result},
    model::quote::QuoteId,
    std::{collections::VecDeque, num::NonZeroUsize, sync::Arc},
    tokio::sync::Mutex,
};

/// How many ids one refill draws. Sized so that a busy orderbook refills a few
/// times per second at most while a restart wastes no more than this many ids
/// of the sequence, which already has gaps anyway.
const DEFAULT_CHUNK_SIZE: NonZeroUsize = NonZeroUsize::new(128).expect("non-zero literal");

/// Hands out quote ids one at a time, drawing them from the underlying
/// generator in chunks.
///
/// Ids handed out by one allocator increase monotonically, but ids handed out
/// by different processes (or by the trivial-quote path, which allocates
/// straight from the database) interleave with them, and ids still buffered
/// when the process stops are never used.
pub struct QuoteIdAllocator {
    generator: Arc<dyn QuoteIdGenerating>,
    chunk_size: NonZeroUsize,
    buffered: Mutex<VecDeque<QuoteId>>,
}

impl QuoteIdAllocator {
    pub fn new(generator: Arc<dyn QuoteIdGenerating>) -> Self {
        Self::with_chunk_size(generator, DEFAULT_CHUNK_SIZE)
    }

    pub fn with_chunk_size(
        generator: Arc<dyn QuoteIdGenerating>,
        chunk_size: NonZeroUsize,
    ) -> Self {
        Self {
            generator,
            chunk_size,
            buffered: Mutex::new(VecDeque::with_capacity(chunk_size.get())),
        }
    }

    /// The next unused quote id, refilling the buffer from the generator when
    /// it runs empty.
    pub async fn next(&self) -> Result<QuoteId> {
        let mut buffered = self.buffered.lock().await;
        if let Some(id) = buffered.pop_front() {
            return Ok(id);
        }
        let ids = self
            .generator
            .generate(self.chunk_size.get())
            .await
            .context("failed to draw quote ids")?;
        buffered.extend(ids);
        buffered
            .pop_front()
            .context("quote id generator returned no ids")
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::MockQuoteIdGenerating,
        futures::FutureExt,
        std::{
            collections::BTreeSet,
            sync::atomic::{AtomicI64, Ordering},
        },
    };

    fn allocator(generator: MockQuoteIdGenerating, chunk_size: usize) -> QuoteIdAllocator {
        QuoteIdAllocator::with_chunk_size(
            Arc::new(generator),
            NonZeroUsize::new(chunk_size).unwrap(),
        )
    }

    /// A generator that hands out consecutive ids and counts its calls.
    fn counting_generator(calls: Arc<AtomicI64>) -> MockQuoteIdGenerating {
        let next = Arc::new(AtomicI64::new(1));
        let mut generator = MockQuoteIdGenerating::new();
        generator.expect_generate().returning(move |n| {
            calls.fetch_add(1, Ordering::SeqCst);
            let n = i64::try_from(n).unwrap();
            let first = next.fetch_add(n, Ordering::SeqCst);
            async move { Ok((first..first + n).collect()) }.boxed()
        });
        generator
    }

    #[tokio::test]
    async fn hands_out_a_chunk_before_drawing_the_next() {
        let calls = Arc::new(AtomicI64::new(0));
        let allocator = allocator(counting_generator(calls.clone()), 3);

        let mut ids = vec![];
        for _ in 0..4 {
            ids.push(allocator.next().await.unwrap());
        }

        assert_eq!(ids, vec![1, 2, 3, 4]);
        // Three ids came from the first chunk, the fourth needed a second one.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn concurrent_callers_share_one_refill() {
        let calls = Arc::new(AtomicI64::new(0));
        let allocator = Arc::new(allocator(counting_generator(calls.clone()), 16));

        let ids = futures::future::try_join_all((0..10).map(|_| {
            let allocator = allocator.clone();
            async move { allocator.next().await }
        }))
        .await
        .unwrap();

        assert_eq!(
            ids.iter().collect::<BTreeSet<_>>().len(),
            10,
            "ids must be unique"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn propagates_generator_errors() {
        let mut generator = MockQuoteIdGenerating::new();
        generator
            .expect_generate()
            .returning(|_| async { Err(anyhow::anyhow!("db down")) }.boxed());
        let allocator = allocator(generator, 8);

        let err = allocator.next().await.unwrap_err();
        assert!(
            err.to_string().contains("failed to draw quote ids"),
            "{err:#}"
        );
    }
}
