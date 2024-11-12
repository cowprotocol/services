pub use tikv_jemallocator;

#[macro_export]
macro_rules! use_custom_global_allocator {
    () => {
        use shared::alloc::tikv_jemallocator;

        #[global_allocator]
        static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
    };
}
