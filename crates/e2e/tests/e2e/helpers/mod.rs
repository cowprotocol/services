mod deploy;
#[macro_use]
mod onchain_components;

use {std::io::Write, tempfile::TempPath};

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
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
}

pub fn config_tmp_file<C: AsRef<[u8]>>(config: C) -> TempPath {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(config.as_ref()).unwrap();
    file.into_temp_path()
}

pub use {deploy::*, onchain_components::*};
