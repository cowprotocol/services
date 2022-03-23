//! This script is used to vendor Truffle JSON artifacts to be used for code
//! generation with `ethcontract`. This is done instead of fetching contracts
//! at build time to reduce the risk of failure.

use anyhow::Result;
use contracts::paths;
use env_logger::Env;
use ethcontract_generate::Source;
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("warn,vendor=info"));

    if let Err(err) = run() {
        log::error!("Error vendoring contracts: {:?}", err);
        std::process::exit(-1);
    }
}

fn run() -> Result<()> {
    let vendor = Vendor::new()?;

    vendor
        .full()
        .github(
            "BalancerV2Authorizer",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/Authorizer.json",
        )?
        .github(
            "BalancerV2Vault",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/Vault.json",
        )?
        .github(
            "BalancerV2WeightedPoolFactory",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/WeightedPoolFactory.json",
        )?
        .github(
            "BalancerV2WeightedPool2TokensFactory",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/WeightedPool2TokensFactory.json",
        )?
        .github(
            "BalancerV2StablePoolFactory",
            "balancer-labs/balancer-v2-monorepo/ad1442113b26ec22081c2047e2ec95355a7f12ba/\
             pkg/deployments/tasks/20210624-stable-pool/abi/StablePoolFactory.json",
        )?
        .npm(
            "ERC20Mintable",
            "@openzeppelin/contracts@2.5.0/build/contracts/ERC20Mintable.json",
        )?
        .npm(
            "GPv2AllowListAuthentication",
            // We use `_Implementation` because the use of a proxy contract makes
            // deploying  for the e2e tests more cumbersome.
            "@gnosis.pm/gp-v2-contracts@1.0.1/\
             deployments/mainnet/GPv2AllowListAuthentication_Implementation.json",
        )?
        .npm(
            "GPv2Settlement",
            "@gnosis.pm/gp-v2-contracts@1.0.1/deployments/mainnet/GPv2Settlement.json",
        )?
        .npm(
            "UniswapV2Factory",
            "@uniswap/v2-core@1.0.1/build/UniswapV2Factory.json",
        )?
        .npm(
            "UniswapV2Router02",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/UniswapV2Router02.json",
        )?
        .npm("WETH9", "canonical-weth@1.4.0/build/contracts/WETH9.json")?;

    vendor
        .abi_only()
        .github(
            "BalancerV2LiquidityBootstrappingPool",
            "balancer-labs/balancer-v2-monorepo/7a643349a5ef4511234b19a33e3f18d30770cb66/\
             pkg/deployments/tasks/20210721-liquidity-bootstrapping-pool/abi/LiquidityBootstrappingPool.json",
        )?
        .github(
            "BalancerV2LiquidityBootstrappingPoolFactory",
            "balancer-labs/balancer-v2-monorepo/7a643349a5ef4511234b19a33e3f18d30770cb66/\
             pkg/deployments/tasks/20210721-liquidity-bootstrapping-pool/abi/LiquidityBootstrappingPoolFactory.json",
        )?
        .github(
            "BalancerV2WeightedPool",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/extra-abis/WeightedPool.json",
        )?
        .github(
            "BalancerV2StablePool",
            "balancer-labs/balancer-subgraph-v2/2b97edd5e65aed06718ce64a69111ccdabccf048/\
             abis/StablePool.json",
        )?
        .npm(
            "ERC20",
            "@openzeppelin/contracts@3.3.0/build/contracts/ERC20.json",
        )?
        .npm(
            "IUniswapLikeFactory",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Factory.json",
        )?
        .npm(
            "IUniswapLikePair",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Pair.json",
        )?
        .npm(
            "IUniswapLikeRouter",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Router02.json",
        )?
        .manual(
            "BalancerV2BasePool",
            "Balancer does not publish ABIs for base contracts",
        )
        .manual(
            "BalancerV2BasePoolFactory",
            "Balancer does not publish ABIs for base contracts",
        )
        .npm(
            "IUniswapV3Factory",
            "@uniswap/v3-core@1.0.0/artifacts/contracts/interfaces/IUniswapV3Factory.sol/IUniswapV3Factory.json",
        )?
        .github(
            "IZeroEx",
            "0xProject/protocol/c1177416f50c2465ee030dacc14ff996eebd4e74/\
             packages/contract-artifacts/artifacts/IZeroEx.json",
        )?
        .github(
            "ISwaprPair",
            "levelkdev/dxswap-core/3511bab996096f9c9c9bc3af0d94222650fd1e40/\
             build/IDXswapPair.json",
        )?
        .npm(
            "CowProtocolToken",
            "@gnosis.pm/cow-token@1.0.3/build/artifacts/src/contracts/CowProtocolToken.sol/CowProtocolToken.json",
        )?
        .npm(
            "CowProtocolVirtualToken",
            "@gnosis.pm/cow-token@1.0.3/build/artifacts/src/contracts/CowProtocolVirtualToken.sol/CowProtocolVirtualToken.json",
        )?;

    Ok(())
}

struct Vendor {
    artifacts: PathBuf,
}

impl Vendor {
    fn new() -> Result<Self> {
        let artifacts = paths::contract_artifacts_dir();
        log::info!("vendoring contract artifacts to '{}'", artifacts.display());
        fs::create_dir_all(&artifacts)?;
        Ok(Self { artifacts })
    }

    /// Creates a context for vendoring "full" contract data, including bytecode
    /// used for deploying the contract for end-to-end test.
    fn full(&self) -> VendorContext {
        VendorContext {
            artifacts: &self.artifacts,
            properties: &[
                ("abi", "abi,compilerOutput.abi"),
                ("devdoc", "devdoc,compilerOutput.devdoc"),
                ("userdoc", "userdoc"),
                ("bytecode", "bytecode"),
            ],
        }
    }

    /// Creates a context for vendoring only the contract ABI for generating
    /// bindings. This is preferred over [`Vendor::full`] for contracts that do
    /// not need to be deployed for tests, or get created by alternative means
    /// (e.g. `UniswapV2Pair` contracts don't require bytecode as they get
    /// created by `UniswapV2Factory` instances on-chain).
    fn abi_only(&self) -> VendorContext {
        VendorContext {
            artifacts: &self.artifacts,
            properties: &[
                ("abi", "abi,compilerOutput.abi"),
                ("devdoc", "devdoc,compilerOutput.devdoc"),
                ("userdoc", "userdoc"),
            ],
        }
    }
}

struct VendorContext<'a> {
    artifacts: &'a Path,
    properties: &'a [(&'a str, &'a str)],
}

impl VendorContext<'_> {
    fn npm(&self, name: &str, path: &str) -> Result<&Self> {
        self.vendor_source(name, Source::npm(path))
    }

    fn github(&self, name: &str, path: &str) -> Result<&Self> {
        self.vendor_source(
            name,
            Source::http(&format!("https://raw.githubusercontent.com/{}", path))?,
        )
    }

    fn manual(&self, name: &str, reason: &str) -> &Self {
        // We just keep these here to document that they are manually generated
        // and not pulled from some source.
        log::info!("skipping {}: {}", name, reason);
        self
    }

    fn retrieve_value_from_path<'a>(source: &'a Value, path: &'a str) -> Value {
        let mut current_value: &Value = source;
        for property in path.split('.') {
            current_value = &current_value[property];
        }
        current_value.clone()
    }

    fn vendor_source(&self, name: &str, source: Source) -> Result<&Self> {
        log::info!("retrieving {:?}", source);
        let artifact_json = source.artifact_json()?;

        log::debug!("pruning artifact JSON");
        let pruned_artifact_json = {
            let json = serde_json::from_str::<Value>(&artifact_json)?;
            let mut pruned = Map::new();
            for (property, paths) in self.properties {
                if let Some(value) = paths
                    .split(',')
                    .map(|path| Self::retrieve_value_from_path(&json, path))
                    .find(|value| !value.is_null())
                {
                    pruned.insert(property.to_string(), value);
                }
            }
            serde_json::to_string(&pruned)?
        };

        let path = self.artifacts.join(name).with_extension("json");
        log::debug!("saving artifact to {}", path.display());
        fs::write(path, pruned_artifact_json)?;

        Ok(self)
    }
}
