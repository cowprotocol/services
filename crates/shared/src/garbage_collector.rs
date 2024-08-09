use std::{
    sync::{Arc, LazyLock, Mutex, Weak},
    time::Duration,
};

/// Singleton garbage collector to ensure that we only have a single task
/// cleaning up all the garbage.
static GC: LazyLock<GarbageCollector> = LazyLock::new(Default::default);

/// Returns a reference to the single global static garbage collector.
pub fn singleton() -> &'static GarbageCollector {
    LazyLock::force(&GC)
}

/// Extremely simple garbage collector that periodically tells tracked items
/// to clean up their garbage.
#[derive(Clone)]
pub struct GarbageCollector(Arc<Mutex<Vec<Weak<dyn GarbageCollecting>>>>);

impl GarbageCollector {
    /// Tells the garbage collector to keep track of the memory of the passed
    /// item.
    pub fn trace_memory(&self, garbage_producer: &Arc<impl GarbageCollecting + 'static>) {
        let reference = Arc::downgrade(&(garbage_producer.clone() as Arc<dyn GarbageCollecting>));
        self.0.lock().unwrap().push(reference);
    }

    /// Creates a new garbage collector that sleeps for `gc_interval` after
    /// every sweep.
    pub fn new(gc_interval: Duration) -> Self {
        let this = Self(Default::default());
        let handle = this.clone();
        tokio::task::spawn(async move {
            loop {
                handle.collect_garbage();
                tokio::time::sleep(gc_interval).await;
            }
        });
        this
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

impl GarbageCollecting for GarbageCollector {
    fn collect_garbage(&self) {
        let mut garbage_producers = self.0.lock().unwrap();
        garbage_producers.retain(|garbage_producer| {
            let Some(garbage_producer) = Weak::upgrade(garbage_producer) else {
                // cache has no strong references left so it can be dropped completely.
                return false;
            };

            garbage_producer.collect_garbage();
            true
        });
    }
}

/// A struct implementing this type knows how to get rid of internal memory that
/// is no longer needed.
pub trait GarbageCollecting: Send + Sync {
    /// Remove internal memory that is no longer needed.
    fn collect_garbage(&self);
}

#[cfg(test)]
mod tests {
    use {
        super::{GarbageCollecting, GarbageCollector},
        std::{
            sync::{Arc, Mutex},
            time::Duration,
        },
    };

    struct Inner {
        in_use: bool,
        used_memory: Vec<()>,
    }

    struct GarbageProducer(Mutex<Inner>);

    impl GarbageProducer {
        fn has_used_memory(&self) -> bool {
            !self.0.lock().unwrap().used_memory.is_empty()
        }

        fn mark_for_cleanup(&self) {
            self.0.lock().unwrap().in_use = false;
        }
    }

    impl Default for GarbageProducer {
        fn default() -> Self {
            Self(Mutex::new(Inner {
                in_use: true,
                used_memory: vec![(); 10],
            }))
        }
    }

    impl GarbageCollecting for GarbageProducer {
        fn collect_garbage(&self) {
            let mut locked = self.0.lock().unwrap();
            if !locked.in_use {
                locked.used_memory.clear();
            }
        }
    }

    #[tokio::test]
    async fn gc_frees_memory() {
        const GC_INTERVAL: Duration = Duration::from_millis(10);
        let producer = Arc::new(GarbageProducer::default());
        let gc = GarbageCollector::new(GC_INTERVAL);
        gc.trace_memory(&producer);

        tokio::time::sleep(GC_INTERVAL * 2).await;
        // GC sweep didn't do anything because memory is still in use
        assert!(producer.has_used_memory());

        producer.mark_for_cleanup();
        tokio::time::sleep(GC_INTERVAL * 2).await;
        // GC called collect_garbage on the produce and now it's empty
        assert!(!producer.has_used_memory());

        drop(producer);
        tokio::time::sleep(GC_INTERVAL * 2).await;
        // Nobody is using the producer anymore so the GC sweep removed it completely
        assert!(gc.0.lock().unwrap().is_empty());
    }
}
