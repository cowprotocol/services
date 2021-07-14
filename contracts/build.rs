use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    Address,
};
use ethcontract_generate::{loaders::TruffleLoader, ContractBuilder};
use std::{env, fs, path::Path};

#[path = "src/paths.rs"]
mod paths;

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existent files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");

    generate_contract_with_config("BalancerV2Vault", |builder| {
        builder
            .contract_mod_override("balancer_v2_vault")
            .add_network(
                "1",
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    deployment_information: Some(tx(
                        "0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7",
                    )),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://rinkeby.etherscan.io/tx/0x5fe65a242760f7f32b582dc402a081791d57ea561474617fcd0e763c995cfec7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8441702)),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    deployment_information: Some(tx(
                        "0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792",
                    )),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    deployment_information: Some(tx(
                        "0xae8c45c1d40756d0eb312723a2993341e379ea6d8bef4adfae2709345939f8eb",
                    )),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPool2TokensFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_2_tokens_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    deployment_information: Some(tx(
                        "0xf40c05058422d730b7035c254f8b765722935a5d3003ac37b13a61860adbaf08",
                    )),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    deployment_information: Some(tx(
                        "be28062b575c2743b3b4525c3a175b9acad36695c15dba1c69af5f3fc3ceca37",
                    )),
                },
            )
    });
    generate_contract("BalancerV2WeightedPool");
    generate_contract("ERC20");
    generate_contract("ERC20Mintable");
    generate_contract("GPv2AllowListAuthentication");
    generate_contract_with_config("GPv2Settlement", |builder| {
        builder
            .contract_mod_override("gpv2_settlement")
            .add_network(
                "1",
                Network {
                    address: addr("0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf"),
                    deployment_information: Some(tx(
                        "34b7f9a340e663df934fcc662b3ec5fcd7cd0c93d3c46f8ce612e94fff803909",
                    )),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf"),
                    deployment_information: Some(tx(
                        "52badda922fd91052e6682d125daa59dea3ce5c57add5a9d362bec2d6ccfd2b1",
                    )),
                },
            )
            .add_network(
                "100",
                Network {
                    address: addr("0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf"),
                    deployment_information: Some(tx(
                        "95bbefbca7162435eeb71bac6960aae4d7112abce87a51ad3952d7b7af0279e3",
                    )),
                },
            )
    });
    generate_contract("IUniswapLikeRouter");
    generate_contract("IUniswapLikePair");
    generate_contract_with_config("SushiswapV2Router02", |builder| {
        builder
            .add_network_str("1", "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")
            .add_network_str("4", "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str("100", "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
    });
    generate_contract_with_config("SushiswapV2Factory", |builder| {
        builder
            .add_network_str("1", "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")
            .add_network_str("4", "0xc35DADB65012eC5796536bD9864eD8773aBc74C4")
            .add_network_str("100", "0xc35DADB65012eC5796536bD9864eD8773aBc74C4")
    });
    generate_contract_with_config("UniswapV2Router02", |builder| {
        builder
            .add_network_str("1", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str("4", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str("100", "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77")
    });
    generate_contract_with_config("UniswapV2Factory", |builder| {
        builder
            .add_network_str("1", "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str("4", "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str("100", "0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7")
    });
    generate_contract_with_config("WETH9", |builder| {
        builder
            .add_network_str("1", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .add_network_str("4", "0xc778417E063141139Fce010982780140Aa0cD5Ab")
            .add_network_str("100", "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d")
    });
}

fn generate_contract(name: &str) {
    generate_contract_with_config(name, |builder| builder)
}

fn generate_contract_with_config(
    name: &str,
    config: impl FnOnce(ContractBuilder) -> ContractBuilder,
) {
    let path = paths::contract_artifacts_dir()
        .join(name)
        .with_extension("json");
    let contract = TruffleLoader::new()
        .name(name)
        .load_contract_from_file(&path)
        .unwrap();
    let address_file = paths::contract_address_file(name);
    let dest = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", path.display());
    let mut builder = ContractBuilder::new().visibility_modifier("pub");

    if let Ok(address) = fs::read_to_string(&address_file) {
        println!("cargo:rerun-if-changed={}", address_file.display());
        builder = builder.add_network_str("5777", address.trim());
    }

    config(builder)
        .generate(&contract)
        .unwrap()
        .write_to_file(Path::new(&dest).join(format!("{}.rs", name)))
        .unwrap();
}

fn addr(s: &str) -> Address {
    s.parse().unwrap()
}

fn tx(s: &str) -> DeploymentInformation {
    DeploymentInformation::TransactionHash(s.parse().unwrap())
}
