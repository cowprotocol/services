#[cfg(not(any(feature="allocator-mimalloc", feature="allocator-tcmalloc", feature="allocator-jemalloc")))]
#[macro_export]
macro_rules! custom_global_allocator {
    () => {};
}

#[cfg(feature="allocator-mimalloc")]
pub use mimalloc;

#[cfg(feature="allocator-mimalloc")]
#[macro_export]
macro_rules! custom_global_allocator {
    () => {
        use alloc::mimalloc;

        #[global_allocator]
        static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
    };
}

#[cfg(feature="allocator-tcmalloc")]
pub use tcmalloc;

#[cfg(feature="allocator-tcmalloc")]
#[macro_export]
macro_rules! custom_global_allocator {
    () => {
        use alloc::tcmalloc;

        #[global_allocator]
        static GLOBAL: tcmalloc::TCMalloc = tcmalloc::TCMalloc;
    };
}

#[cfg(feature="allocator-jemalloc")]
pub use jemallocator;

#[cfg(feature="allocator-jemalloc")]
#[macro_export]
macro_rules! custom_global_allocator {
    () => {
        use alloc::jemallocator;

        #[global_allocator]
        static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;
    };
}
