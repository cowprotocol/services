use {
    std::{panic::PanicInfo, sync::Once},
    time::macros::format_description,
    tracing::level_filters::LevelFilter,
    tracing_subscriber::{
        fmt::{time::UtcTime, writer::MakeWriterExt as _},
        prelude::*,
        util::SubscriberInitExt,
        EnvFilter,
        Layer,
    },
};

/// Initializes tracing setup that is shared between the binaries.
/// `env_filter` has similar syntax to env_logger. It is documented at
/// https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html
pub fn initialize(env_filter: &str, stderr_threshold: LevelFilter, with_console: bool) {
    set_tracing_subscriber(env_filter, stderr_threshold, with_console);
    std::panic::set_hook(Box::new(tracing_panic_hook));
}

/// Like [`initialize`], but can be called multiple times in a row. Later calls
/// are ignored.
///
/// Useful for tests.
pub fn initialize_reentrant(env_filter: &str, with_console: bool) {
    // The tracing subscriber below is global object so initializing it again in the
    // same process by a different thread would fail.
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        set_tracing_subscriber(env_filter, LevelFilter::ERROR, with_console);
        std::panic::set_hook(Box::new(tracing_panic_hook));
    });
}

fn set_tracing_subscriber(env_filter: &str, stderr_threshold: LevelFilter, with_console: bool) {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(
            std::io::stdout
                .with_min_level(
                    stderr_threshold
                        .into_level()
                        .unwrap_or(tracing::Level::ERROR),
                )
                .or_else(std::io::stderr),
        )
        .with_timer(UtcTime::new(format_description!(
            // This is what kibana uses to separate multi line log messages.
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        )))
        .with_ansi(atty::is(atty::Stream::Stdout))
        .with_filter::<EnvFilter>(env_filter.into());

    let registry = tracing_subscriber::registry().with(fmt_layer);
    if with_console {
        if !cfg!(tokio_unstable) {
            panic!(
                "compile with `RUSTFLAGS=\"--cfg tokio_unstable\"` if you want to enable the \
                 tokio console"
            );
        }
        registry.with(console_subscriber::spawn()).init();
    } else {
        registry.init()
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
