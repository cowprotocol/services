use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    Address,
};
use ethcontract_generate::{loaders::TruffleLoader, ContractBuilder};
use std::{env, path::Path};

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

    generate_contract_with_config("BalancerV2Authorizer", |builder| {
        builder.contract_mod_override("balancer_v2_authorizer")
    });
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
    generate_contract_with_config("BalancerV2StablePoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_stable_pool_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0xc66ba2b6595d3613ccab350c886ace23866ede24"),
                    deployment_information: Some(tx(
                        "0xfd417511f3902a304cca51023e8e771de22ffa7f30b9c8650ec5757328ab89a6",
                    )),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xc66ba2b6595d3613ccab350c886ace23866ede24"),
                    deployment_information: Some(tx(
                        "26ccac4bd7af78607107489fa05868a68291b5e6f217f6829fc3767d8926264a",
                    )),
                },
            )
    });
    generate_contract("BalancerV2WeightedPool");
    generate_contract_with_config("BalancerV2StablePool", |builder| {
        builder.add_method_alias(
            "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),uint256[],uint256,uint256)",
            "on_swap_with_balances"
        )
    });
    generate_contract_with_config("BaoswapFactory", |builder| {
        builder.add_network_str("100", "0x45DE240fbE2077dd3e711299538A09854FAE9c9b")
    });
    generate_contract_with_config("BaoswapRouter", |builder| {
        builder.add_network_str("100", "0x6093AeBAC87d62b1A5a4cEec91204e35020E38bE")
    });
    generate_contract("ERC20");
    generate_contract("ERC20Mintable");
    generate_contract("GPv2AllowListAuthentication");
    generate_contract_with_config("GPv2Settlement", |builder| {
        builder
            .contract_mod_override("gpv2_settlement")
            .add_network(
                "1",
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    deployment_information: Some(tx(
                        "f49f90aa5a268c40001d1227b76bb4dd8247f18361fcad9fffd4a7a44f1320d3",
                    )),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    deployment_information: Some(tx(
                        "609fa2e8f32c73c1f5dc21ff60a26238dacb50d4674d336c90d6950bdda17a21",
                    )),
                },
            )
            .add_network(
                "100",
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    deployment_information: Some(tx(
                        "9ddc538f89cd8433f4a19bc4de0de27e7c68a1d04a14b327185e4bba9af87133",
                    )),
                },
            )
    });
    generate_contract_with_config("HoneyswapFactory", |builder| {
        builder.add_network_str("100", "0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7")
    });
    generate_contract_with_config("HoneyswapRouter", |builder| {
        builder.add_network_str("100", "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77")
    });
    generate_contract("IUniswapLikeRouter");
    generate_contract("IUniswapLikePair");
    generate_contract_with_config("SushiSwapFactory", |builder| {
        builder
            .add_network_str("1", "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")
            .add_network_str("4", "0xc35DADB65012eC5796536bD9864eD8773aBc74C4")
            .add_network_str("100", "0xc35DADB65012eC5796536bD9864eD8773aBc74C4")
    });
    generate_contract_with_config("SushiSwapRouter", |builder| {
        builder
            .add_network_str("1", "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")
            .add_network_str("4", "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str("100", "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
    });
    generate_contract_with_config("UniswapV2Factory", |builder| {
        builder
            .add_network_str("1", "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str("4", "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
    });
    generate_contract_with_config("UniswapV2Router02", |builder| {
        builder
            .add_network_str("1", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str("4", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
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
    let dest = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", path.display());

    config(ContractBuilder::new().visibility_modifier("pub"))
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
