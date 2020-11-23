//! This script is used to vendor Truffle JSON artifacts to be used for code
//! generation with `ethcontract`. This is done instead of fetching contracts
//! at build time to reduce the risk of failure.

use anyhow::Result;
use contracts::paths;
use env_logger::Env;
use ethcontract_generate::Source;
use serde_json::{Map, Value};
use std::fs;

const ARTIFACTS: &[(&str, &[&str])] = &[("@openzeppelin/contracts@2.5.0", &["IERC20"])];

fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("warn,vendor=info"));

    if let Err(err) = run() {
        log::error!("Error vendoring contracts: {:?}", err);
        std::process::exit(-1);
    }
}

fn run() -> Result<()> {
    let artifacts = paths::contract_artifacts_dir();
    fs::create_dir_all(&artifacts)?;

    log::info!("vendoring contract artifacts to '{}'", artifacts.display());
    for (package, contracts) in ARTIFACTS {
        for contract in *contracts {
            log::info!("retrieving {} from {}", contract, package);
            let path = format!("{}/build/contracts/{}.json", package, contract);
            let source = Source::npm(path);
            let artifact_json = source.artifact_json()?;

            log::debug!("pruning artifact JSON");
            let pruned_artifact_json = {
                let mut json = serde_json::from_str::<Value>(&artifact_json)?;
                let mut pruned = Map::new();
                for property in &[
                    "abi",
                    "bytecode",
                    "contractName",
                    "devdoc",
                    "networks",
                    "userdoc",
                ] {
                    if let Some(value) = json.get_mut(property) {
                        pruned.insert(property.to_string(), value.take());
                    }
                }
                serde_json::to_string(&pruned)?
            };

            let path = artifacts.join(format!("{}.json", contract));
            log::debug!("saving artifact to {}", path.display());
            fs::write(path, pruned_artifact_json)?;
        }
    }

    Ok(())
}
