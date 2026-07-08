//! Write a JSON document mapping each event subject to the JSON schema of its
//! full (envelope-wrapped) wire format.
//!
//! By default writes to `schemas/events.json` inside the crate. Pass an
//! alternative path as the first argument to override.

use std::{collections::BTreeMap, path::PathBuf};

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("schemas")
                .join("events.json")
        });

    // Sort by subject so the generated document is stable across runs
    let schemas: BTreeMap<String, serde_json::Value> = event_bus_dto::schemas()
        .into_iter()
        .map(|(subject, schema)| (subject.to_owned(), serde_json::to_value(schema).unwrap()))
        .collect();
    let body = serde_json::to_string_pretty(&schemas).unwrap();

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create output directory");
    }
    std::fs::write(&out_path, format!("{body}\n")).expect("failed to write schema file");
    eprintln!("wrote {} ({} events)", out_path.display(), schemas.len());
}
