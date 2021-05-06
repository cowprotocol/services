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
        "@openzeppelin/contracts@3.3.0/build/contracts/ERC20.json",
        "ERC20.json",
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
        "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Router02.json",
        "IUniswapLikeRouter.json",
    ),
    (
        "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Pair.json",
        "IUniswapLikePair.json",
    ),
    (
        "@uniswap/v2-core@1.0.1/build/UniswapV2Factory.json",
        "UniswapV2Factory.json",
    ),
    (
        "@uniswap/v2-core@1.0.1/build/UniswapV2Pair.json",
        "UniswapV2Pair.json",
    ),
    (
        "@gnosis.pm/gp-v2-contracts@0.0.1-alpha.15/deployments/rinkeby/GPv2Settlement.json",
        "GPv2Settlement.json",
    ),
    // We use `_Implementation` because the use of a proxy contract (https://github.com/wighawag/hardhat-deploy/blob/52be3661d74a6ba873c8bb06510e29a43a4a39c1/solc_0.7/proxy/EIP173Proxy.sol#L18)
    // makes deploying for the e2e test more cumbersome.
    (
        "@gnosis.pm/gp-v2-contracts@0.0.1-alpha.15/deployments/rinkeby/GPv2AllowListAuthentication_Implementation.json",
        "GPv2AllowListAuthentication.json",
    ),
    (
        "canonical-weth@1.4.0/build/contracts/WETH9.json",
        "WETH9.json",
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
