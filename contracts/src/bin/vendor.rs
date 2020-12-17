//! This script is used to vendor Truffle JSON artifacts to be used for code
//! generation with `ethcontract`. This is done instead of fetching contracts
//! at build time to reduce the risk of failure.

use anyhow::Result;
use contracts::paths;
use env_logger::Env;
use ethcontract_generate::Source;
use serde_json::{Map, Value};
use std::fs;

// npm path and local file name
const NPM_CONTRACTS: &[(&str, &str)] = &[
    (
        "@openzeppelin/contracts@3.3.0/build/contracts/IERC20.json",
        "IERC20.json",
    ),
    (
        "@openzeppelin/contracts@2.5.0/build/contracts/ERC20Mintable.json",
        "ERC20Mintable.json",
    ),
    (
        "@uniswap/v2-periphery@1.1.0-beta.0/build/UniswapV2Router02.json",
        "UniswapV2Router02.json",
    ),
    (
        "@uniswap/v2-core@1.0.1/build/UniswapV2Factory.json",
        "UniswapV2Factory.json",
    ),
    (
        "@gnosis.pm/gp-v2-contracts@0.0.1-alpha.10/deployments/rinkeby/GPv2Settlement.json",
        "GPv2Settlement.json",
    ),
    (
        "@gnosis.pm/gp-v2-contracts@0.0.1-alpha.10/deployments/rinkeby/GPv2AllowListAuthentication.json",
        "GPv2AllowListAuthentication.json",
    ),
];

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
    for (npm_path, local_path) in NPM_CONTRACTS {
        log::info!("retrieving {}", npm_path);
        let source = Source::npm(npm_path.to_string());
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

        let path = artifacts.join(local_path);
        log::debug!("saving artifact to {}", path.display());
        fs::write(path, pruned_artifact_json)?;
    }

    Ok(())
}
