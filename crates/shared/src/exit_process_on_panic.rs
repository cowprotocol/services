/// Install a panic hook that first calls the previous panic hook and then exits the process.
///
/// This prevents the situation where a background task/thread unexpectedly or unnoticed panics
/// which can affect the rest of the process in unpredictable ways like a cache never getting
/// updated.
///
/// The downside of this approach is that it prevents use of expected/intentional panics. We do not
/// use those so this isn't a problem. See https://github.com/cowprotocol/services/issues/514 for
/// alternatives.
pub fn set_panic_hook() {
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicInfo| {
        previous_hook(info);
        std::process::exit(1);
    };
    std::panic::set_hook(Box::new(new_hook));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn manual_thread() {
        crate::tracing::initialize("info", tracing::level_filters::LevelFilter::OFF);

        // Should print panic trace log but not kill the process.
        let handle = std::thread::spawn(|| panic!("you should see this message"));
        assert!(handle.join().is_err());

        set_panic_hook();
        // Should print panic trace log because we call the previous panic handler installed by
        // tracing::initialize, and kill the process.
        let handle = std::thread::spawn(|| panic!("you should see this message"));
        let _ = handle.join();
        unreachable!("you should NOT see this message");
    }

    // Like above but using tokio tasks.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn manual_tokio() {
        crate::tracing::initialize("info", tracing::level_filters::LevelFilter::OFF);

        let handle = tokio::task::spawn(async { panic!("you should see this message") });
        assert!(handle.await.is_err());

        set_panic_hook();
        let handle = tokio::task::spawn(async { panic!("you should see this message") });
        let _ = handle.await;
        unreachable!("you should NOT see this message");
    }
}
