use std::{
    panic::{self, PanicInfo},
    thread,
};
use tracing_subscriber::fmt::time::ChronoUtc;

/// Initializes tracing setup that is shared between the binaries.
/// `env_filter` has similar syntax to env_logger. It is documented at
/// https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html
pub fn initialize(env_filter: &str) {
    // This is what kibana uses to separate mutli line log messages.
    let time_format_string = "%Y-%m-%dT%H:%M:%S%.3fZ";
    tracing_subscriber::fmt::fmt()
        .with_timer(ChronoUtc::with_format(String::from(time_format_string)))
        .with_env_filter(env_filter)
        .init();
    set_panic_hook();
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
