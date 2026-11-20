use std::sync::atomic::{AtomicBool, Ordering};

/// True once [`install`] ran.
///
/// A process has one panic hook. `std::panic::set_hook` drops the current hook
/// unless the caller took it with `std::panic::take_hook` first, which lets
/// the new hook call it. A hook set after [`install`] without taking it drops
/// the exit, so it has to call [`exit_if_installed`].
static IS_PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

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
    IS_PANIC_HOOK_INSTALLED.store(true, Ordering::Relaxed);
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicHookInfo| {
        previous_hook(info);
        std::process::exit(1);
    };
    std::panic::set_hook(Box::new(new_hook));
}

/// Exits the process if [`install`] ran. A panic hook that replaced the one
/// `install` set calls this to keep the exit.
pub(crate) fn exit_if_installed() {
    if IS_PANIC_HOOK_INSTALLED.load(Ordering::Relaxed) {
        std::process::exit(1);
    }
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

    /// A background panic ends the process in either order of [`install`] and
    /// tracing initialization, which sets a panic hook of its own. The panic
    /// runs in a child process and the test reads its exit code.
    #[tokio::test(flavor = "multi_thread")]
    async fn exits_on_panic_in_either_order_of_install_and_tracing_init() {
        const ORDER: &str = "PANIC_HOOK_TEST_ORDER";
        if let Ok(order) = std::env::var(ORDER) {
            let config = Config::new("off", None, false, None);
            match order.as_str() {
                "install_first" => {
                    install();
                    crate::tracing::init::initialize_reentrant(&config);
                }
                "tracing_first" => {
                    crate::tracing::init::initialize_reentrant(&config);
                    install();
                }
                other => panic!("unknown order {other}"),
            }
            let _ = tokio::spawn(async { panic!("background panic") }).await;
            // Reached only when the panic left the process running.
            std::process::exit(0);
        }

        for order in ["install_first", "tracing_first"] {
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "panic_hook::tests::exits_on_panic_in_either_order_of_install_and_tracing_init",
                ])
                .env(ORDER, order)
                .status()
                .unwrap();
            assert_eq!(child.code(), Some(1), "{order}");
        }
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
