use std::sync::atomic::{AtomicBool, Ordering};

/// True once [`install`] ran. Outlives the hook that `install` sets, which a
/// later `std::panic::set_hook` replaces.
static EXIT_ON_PANIC: AtomicBool = AtomicBool::new(false);

/// Install a panic hook that first calls the previous panic hook and then exits
/// the process. A panic also exits the process if tracing is initialized
/// afterwards, which replaces this hook.
///
/// This prevents the situation where a background task/thread unexpectedly or
/// unnoticed panics which can affect the rest of the process in unpredictable
/// ways like a cache never getting updated.
///
/// The downside of this approach is that it prevents use of
/// expected/intentional panics. We do not use those so this isn't a problem. See https://github.com/cowprotocol/services/issues/514 for
/// alternatives.
pub fn install() {
    EXIT_ON_PANIC.store(true, Ordering::Relaxed);
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicHookInfo| {
        previous_hook(info);
        std::process::exit(1);
    };
    std::panic::set_hook(Box::new(new_hook));
}

/// Whether [`install`] ran, so a panic has to exit the process.
pub(crate) fn exits_process() -> bool {
    EXIT_ON_PANIC.load(Ordering::Relaxed)
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

    /// A background panic ends the process when the hook is installed before
    /// tracing, which sets a panic hook of its own. The panic runs in a child
    /// process and the test reads its exit code.
    #[tokio::test(flavor = "multi_thread")]
    async fn exits_on_panic_when_installed_before_tracing_init() {
        const CHILD: &str = "PANIC_HOOK_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            install();
            crate::tracing::init::initialize_reentrant(&Config::new("off", None, false, None));
            let _ = tokio::spawn(async { panic!("background panic") }).await;
            // Reached only when the panic left the process running.
            std::process::exit(0);
        }

        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "panic_hook::tests::exits_on_panic_when_installed_before_tracing_init",
            ])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert_eq!(child.code(), Some(1));
    }

    #[test]
    #[ignore]
    fn manual_thread() {
        let obs_config = Config::new("info", None, false, None);
        crate::tracing::init::initialize(&obs_config);

        // Should print panic trace log but not kill the process.
        let handle = std::thread::spawn(|| panic!("you should see this message"));
        assert!(handle.join().is_err());

        install();
        // Should print panic trace log because we call the previous panic
        // handler installed by tracing::initialize, and kill the
        // process.
        let handle = std::thread::spawn(|| panic!("you should see this message"));
        let _ = handle.join();
        unreachable!("you should NOT see this message");
    }

    // Like above but using tokio tasks.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn manual_tokio() {
        let obs_config = Config::new("info", None, false, None);
        crate::tracing::init::initialize(&obs_config);

        let handle = tokio::task::spawn(async { panic!("you should see this message") });
        assert!(handle.await.is_err());

        install();
        let handle = tokio::task::spawn(async { panic!("you should see this message") });
        let _ = handle.await;
        unreachable!("you should NOT see this message");
    }
}
