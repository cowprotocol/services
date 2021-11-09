use std::{
    panic::{self, PanicInfo},
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
use time::macros::format_description;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::{time::UtcTime, writer::MakeWriterExt as _};

/// Initializes tracing setup that is shared between the binaries.
/// `env_filter` has similar syntax to env_logger. It is documented at
/// https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html
pub fn initialize(env_filter: &str, stderr_threshold: LevelFilter) {
    set_tracing_subscriber(env_filter, stderr_threshold);
    set_panic_hook();
}

// Like above but meant to be used in tests.
pub fn initialize_for_tests(env_filter: &str) {
    // The tracing subscriber below is global object so initializing it again in the same process by
    // a different thread would fail.
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    if INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    set_tracing_subscriber(env_filter, LevelFilter::OFF);
}

fn set_tracing_subscriber(env_filter: &str, stderr_threshold: LevelFilter) {
    // This is what kibana uses to separate multi line log messages.
    let subscriber_builder = tracing_subscriber::fmt::fmt()
        .with_timer(UtcTime::new(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        )))
        .with_env_filter(env_filter)
        .with_ansi(atty::is(atty::Stream::Stdout));
    // try_init failing indicates that the
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

// Sets a panic hook so panic information is logged in addition to the default panic printer.
fn set_panic_hook() {
    let default_hook = panic::take_hook();
    let hook = move |info: &PanicInfo| {
        let thread = thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        // It is not possible for our custom hook to print a full backtrace on stable rust. To not
        // lose this information we call the default panic handler which prints the full backtrace.
        // The preceding log makes kibana consider this a multi line log message.
        tracing::error!("thread '{}' {}:", thread_name, info);
        default_hook(info);
    };
    panic::set_hook(Box::new(hook));
}
