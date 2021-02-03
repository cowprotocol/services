use ethcontract_generate::{Address, Builder, TransactionHash};
use maplit::hashmap;
use std::{collections::HashMap, env, fs, path::Path, str::FromStr};

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

    generate_contract("IERC20", hashmap! {});
    generate_contract("ERC20Mintable", hashmap! {});
    generate_contract(
        "UniswapV2Router02",
        hashmap! {
            1 => (Address::from_str("7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap(), None),
            4 => (Address::from_str("7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap(), None),
            100 => (Address::from_str("1C232F01118CB8B424793ae03F870aa7D0ac7f77").unwrap(), None),
        },
    );
    generate_contract(
        "UniswapV2Factory",
        hashmap! {
            1 => (Address::from_str("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap(), None),
            4 => (Address::from_str("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap(), None),
            100 => (Address::from_str("A818b4F111Ccac7AA31D0BCc0806d64F2E0737D7").unwrap(), None),
        },
    );
    generate_contract("UniswapV2Pair", hashmap! {});
    generate_contract(
        "GPv2Settlement",
        hashmap! {
            1 => (Address::from_str("4E608b7Da83f8E9213F554BDAA77C72e125529d0").unwrap(), Some("4e20d94594f7ae340dadb18a9b7fa10e80f0a8dae2c72c27442cb015b9b205cf".parse().unwrap())),
            4 => (Address::from_str("4E608b7Da83f8E9213F554BDAA77C72e125529d0").unwrap(), Some("0xd1030e85508042a6762598935751d38c4acd537401e2f2980c8fba5261022691".parse().unwrap())),
            100 => (Address::from_str("4E608b7Da83f8E9213F554BDAA77C72e125529d0").unwrap(), Some("0xa1f149073825dc0ea99ce00656a54b038b739a70309193e43c2e64935828fdaa".parse().unwrap())),
        },
    );
    generate_contract("GPv2AllowListAuthentication", hashmap! {});
}

fn generate_contract(
    name: &str,
    deployment_overrides: HashMap<u32, (Address, Option<TransactionHash>)>,
) {
    let artifact = paths::contract_artifacts_dir().join(format!("{}.json", name));
    let address_file = paths::contract_address_file(name);
    let dest = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", artifact.display());
    let mut builder = Builder::new(artifact)
        .with_contract_name_override(Some(name))
        .with_visibility_modifier(Some("pub"))
        .add_event_derive("serde::Deserialize")
        .add_event_derive("serde::Serialize");

    if let Ok(address) = fs::read_to_string(&address_file) {
        println!("cargo:rerun-if-changed={}", address_file.display());
        builder = builder.add_deployment_str(5777, address.trim());
    }

    for (network_id, (address, transaction_hash)) in deployment_overrides.into_iter() {
        builder = builder.add_deployment(network_id, address, transaction_hash);
    }

    builder
        .generate()
        .unwrap()
        .write_to_file(Path::new(&dest).join(format!("{}.rs", name)))
        .unwrap();
}
