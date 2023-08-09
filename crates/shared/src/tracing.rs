use {
    std::{panic::PanicInfo, sync::Once},
    time::macros::format_description,
    tracing::level_filters::LevelFilter,
    tracing_subscriber::fmt::{time::UtcTime, writer::MakeWriterExt as _},
};

/// Initializes tracing setup that is shared between the binaries.
/// `env_filter` has similar syntax to env_logger. It is documented at
/// https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html
pub fn initialize(env_filter: &str, stderr_threshold: LevelFilter) {
    set_tracing_subscriber(env_filter, stderr_threshold);
    std::panic::set_hook(Box::new(tracing_panic_hook));
}

/// Like [`initialize`], but can be called multiple times in a row. Later calls
/// are ignored.
///
/// Useful for tests.
pub fn initialize_reentrant(env_filter: &str) {
    // The tracing subscriber below is global object so initializing it again in the
    // same process by a different thread would fail.
    static ONCE: Once = Once::new();
    ONCE.call_once(|| set_tracing_subscriber(env_filter, LevelFilter::ERROR));
}

fn set_tracing_subscriber(env_filter: &str, stderr_threshold: LevelFilter) {
    // This is what kibana uses to separate multi line log messages.
    let subscriber_builder = tracing_subscriber::fmt::fmt()
        .with_timer(UtcTime::new(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        )))
        .with_env_filter(env_filter)
        .with_ansi(atty::is(atty::Stream::Stdout));
    match stderr_threshold.into_level() {
        Some(threshold) => subscriber_builder
            .with_writer(
                std::io::stderr
                    .with_max_level(threshold)
                    .or_else(std::io::stdout),
            )
            .init(),
        None => subscriber_builder.init(),
    }
}

/// Panic hook that prints roughly the same message as the default panic hook
/// but uses tracing:error instead of stderr.
///
/// Useful when we want panic messages to have the proper log format for Kibana.
fn tracing_panic_hook(panic: &PanicInfo) {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    let backtrace = std::backtrace::Backtrace::force_capture();
    tracing::error!("thread '{name}' {panic}\nstack backtrace:\n{backtrace}");
}

tokio::task_local! {
    pub static REQUEST_ID: std::cell::RefCell<String>;
}

pub fn get_task_local_storage() -> Option<String> {
    let mut id = None;
    let _ = crate::tracing::REQUEST_ID.try_with(|cell| {
        id = Some(cell.borrow().clone());
    });
    id
}

/// Panics if called in a task that was not created with `REQUEST_ID.sync()`.
pub fn set_task_local_storage(value: String) {
    crate::tracing::REQUEST_ID.with(|storage| {
        *storage.borrow_mut() = value;
    });
}
