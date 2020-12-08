use ethcontract_generate::Builder;
use std::{env, path::Path};

#[path = "src/paths.rs"]
mod paths;

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existant files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");

    generate_contract("IERC20");
    generate_contract("IUniswapV2Router02");
    generate_contract("GPv2Settlement");
}

fn generate_contract(name: &str) {
    let artifact = paths::contract_artifacts_dir().join(format!("{}.json", name));
    let dest = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", artifact.display());
    let builder = Builder::new(artifact)
        .with_contract_name_override(Some(name))
        .with_visibility_modifier(Some("pub"))
        .add_event_derive("serde::Deserialize")
        .add_event_derive("serde::Serialize");

    builder
        .generate()
        .unwrap()
        .write_to_file(Path::new(&dest).join(format!("{}.rs", name)))
        .unwrap();
}
