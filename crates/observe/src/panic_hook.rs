/// Install a panic hook that first calls the previous panic hook and then exits
/// the process.
///
/// This prevents the situation where a background task/thread unexpectedly or
/// unnoticed panics which can affect the rest of the process in unpredictable
/// ways like a cache never getting updated.
///
/// The downside of this approach is that it prevents use of
/// expected/intentional panics. We do not use those so this isn't a problem. See https://github.com/cowprotocol/services/issues/514 for
/// alternatives.
pub fn install() {
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicHookInfo| {
        previous_hook(info);
        std::process::exit(1);
    };
    std::panic::set_hook(Box::new(new_hook));
}

/// Installs a panic handler that executes [`handler`] plus whatever panic
/// handler was already set up.
/// This can be useful to make absolutely sure to clean up some resources like
/// running processes on a panic.
pub fn prepend_panic_handler(handler: Box<dyn Fn(&std::panic::PanicHookInfo) + Send + Sync>) {
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicHookInfo| {
        handler(info);
        previous_hook(info);
    };
    std::panic::set_hook(Box::new(new_hook));
}

#[cfg(test)]
mod tests {
    use {super::*, crate::config::Config};

    #[test]
    #[ignore]
    fn manual_thread() {
        let obs_config = Config::new("info", None, false, None);
        crate::tracing::initialize(&obs_config);

        // Should print panic trace log but not kill the process.
        let handle = std::thread::spawn(|| panic!("you should see this message"));
        assert!(handle.join().is_err());

        install();
        // Should print panic trace log because we call the previous panic handler
        // installed by tracing::initialize, and kill the process.
        let handle = std::thread::spawn(|| panic!("you should see this message"));
        let _ = handle.join();
        unreachable!("you should NOT see this message");
    }

    // Like above but using tokio tasks.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn manual_tokio() {
        let obs_config = Config::new("info", None, false, None);
        crate::tracing::initialize(&obs_config);

        let handle = tokio::task::spawn(async { panic!("you should see this message") });
        assert!(handle.await.is_err());

        install();
        let handle = tokio::task::spawn(async { panic!("you should see this message") });
        let _ = handle.await;
        unreachable!("you should NOT see this message");
    }
}
