//! Write a JSON document mapping each event subject to the JSON schema of its
//! full (envelope-wrapped) wire format.
//!
//! By default writes to `schemas/events.json` inside the crate. Pass an
//! alternative path as the first argument to override.

use std::{collections::BTreeMap, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
struct CliArguments {
    /// The path to the output schema file.
    ///
    /// Defaults to `schemas/events.json` inside the crate.
    #[arg(default_value_os_t = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas").join("events.json"))]
    out_path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let CliArguments { out_path } = CliArguments::parse();

    // Sort by subject so the generated document is stable across runs
    let schemas: BTreeMap<String, serde_json::Value> = event_bus_dto::schemas()
        .into_iter()
        .map(|(subject, schema)| Ok((subject.to_owned(), serde_json::to_value(schema)?)))
        .collect::<Result<_, serde_json::Error>>()?;
    let body = serde_json::to_string_pretty(&schemas)?;

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, format!("{body}\n"))?;
    eprintln!("wrote {} ({} events)", out_path.display(), schemas.len());
    Ok(())
}
