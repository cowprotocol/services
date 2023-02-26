mod deploy;
#[macro_use]
mod onchain_components;
mod services;

use {
    anyhow::{anyhow, Result},
    std::{future::Future, io::Write, time::Duration},
    tempfile::TempPath,
};
pub use {deploy::*, onchain_components::*, services::*};

/// Initialize tracing and assure that panic hook is set.
pub async fn init() {
    let filters = [
        "e2e=debug",
        "orderbook=debug",
        "solver=debug",
        "autopilot=debug",
        "orderbook::api::request_summary=off",
    ]
    .join(",");

    shared::tracing::initialize_reentrant(&filters);
    // shared::exit_process_on_panic::set_panic_hook();
}

/// Create a temporary file with the given content.
pub fn config_tmp_file<C: AsRef<[u8]>>(content: C) -> TempPath {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(content.as_ref()).unwrap();
    file.into_temp_path()
}

/// Repeatedly evaluate condition until it returns true or the timeout is
/// reached. If condition evaluates to true, Ok(()) is returned. If the timeout
/// is reached Err is returned.
pub async fn wait_for_condition<Fut>(
    timeout: Duration,
    mut condition: impl FnMut() -> Fut,
) -> Result<()>
where
    Fut: Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while !condition().await {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if start.elapsed() > timeout {
            return Err(anyhow!("timeout"));
        }
    }
    Ok(())
}
