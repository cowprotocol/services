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
    generate_contract_with_config("BalancerV2BasePool", |builder| {
        builder.contract_mod_override("balancer_v2_base_pool")
    });
    generate_contract_with_config("BalancerV2BasePoolFactory", |builder| {
        builder.contract_mod_override("balancer_v2_base_pool_factory")
    });
    generate_contract_with_config("BalancerV2Vault", |builder| {
        builder
            .contract_mod_override("balancer_v2_vault")
            .add_network(
                "1",
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://etherscan.io/tx/0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12272146)),
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
                    // <https://etherscan.io/tx/0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12272147)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://rinkeby.etherscan.io/tx/0xae8c45c1d40756d0eb312723a2993341e379ea6d8bef4adfae2709345939f8eb>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8441703)),
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
                    // <https://etherscan.io/tx/0xf40c05058422d730b7035c254f8b765722935a5d3003ac37b13a61860adbaf08>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12349891)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    // <https://rinkeby.etherscan.io/tx/0xbe28062b575c2743b3b4525c3a175b9acad36695c15dba1c69af5f3fc3ceca37>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8510540)),
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
                    // <https://etherscan.io/tx/0xfd417511f3902a304cca51023e8e771de22ffa7f30b9c8650ec5757328ab89a6>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12703127)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xc66ba2b6595d3613ccab350c886ace23866ede24"),
                    // <https://rinkeby.etherscan.io/tx/0x26ccac4bd7af78607107489fa05868a68291b5e6f217f6829fc3767d8926264a>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8822477)),
                },
            )
    });
    generate_contract_with_config("BalancerV2LiquidityBootstrappingPoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_liquidity_bootstrapping_pool_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE"),
                    // <https://etherscan.io/tx/0x665ac1c7c5290d70154d9dfc1d91dc2562b143aaa9e8a77aa13e7053e4fe9b7c>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12871780)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xdcdbf71A870cc60C6F9B621E28a7D3Ffd6Dd4965"),
                    // <https://rinkeby.etherscan.io/tx/0x4344f7e7404c24f03c1fb1b421294ce4ced8f44092424344a49602937cf9907e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8976588)),
                },
            )
    });
    generate_contract_with_config(
        "BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory",
        |builder| {
            builder
                .contract_mod_override(
                    "balancer_v2_no_protocol_fee_liquidity_bootstrapping_pool_factory",
                )
                .add_network(
                    "1",
                    Network {
                        address: addr("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"),
                        // <https://etherscan.io/tx/0x298381e567ff6643d9b32e8e7e9ff0f04a80929dce3e004f6fa1a0104b2b69c3>
                        deployment_information: Some(DeploymentInformation::BlockNumber(13730248)),
                    },
                )
                .add_network(
                    "4",
                    Network {
                        address: addr("0x41B953164995c11C81DA73D212ED8Af25741b7Ac"),
                        // <https://rinkeby.etherscan.io/tx/0x69211f2b510d5d18b49e226822f4b920979b75ba87f5041034dc53d38a79a7c3>
                        deployment_information: Some(DeploymentInformation::BlockNumber(13730248)),
                    },
                )
        },
    );
    generate_contract("BalancerV2WeightedPool");
    generate_contract_with_config("BalancerV2StablePool", |builder| {
        builder.add_method_alias(
            "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),uint256[],uint256,uint256)",
            "on_swap_with_balances"
        )
    });
    generate_contract("BalancerV2LiquidityBootstrappingPool");
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
                    // <https://etherscan.io/tx/0xf49f90aa5a268c40001d1227b76bb4dd8247f18361fcad9fffd4a7a44f1320d3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12593265)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://rinkeby.etherscan.io/tx/0x609fa2e8f32c73c1f5dc21ff60a26238dacb50d4674d336c90d6950bdda17a21>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8727415)),
                },
            )
            .add_network(
                "100",
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://blockscout.com/xdai/mainnet/tx/0x9ddc538f89cd8433f4a19bc4de0de27e7c68a1d04a14b327185e4bba9af87133>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16465100)),
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
    generate_contract_with_config("SwaprFactory", |builder| {
        builder.add_network_str("100", "0x5D48C95AdfFD4B40c1AAADc4e08fc44117E02179")
    });
    generate_contract_with_config("SwaprRouter", |builder| {
        builder.add_network_str("100", "0xE43e60736b1cb4a75ad25240E2f9a62Bff65c0C0")
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
    generate_contract_with_config("UniswapSwapRouter02", |builder| {
        builder
            .add_network_str("1", "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
            .add_network_str("4", "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
            .add_method_alias(
                "checkOracleSlippage(bytes[],uint128[],uint24,uint32)",
                "check_oracle_slippage_with_amounts",
            )
            .add_method_alias(
                "multicall(bytes32,bytes[])",
                "multicall_with_previous_blockhash",
            )
            .add_method_alias(
                "multicall(uint256,bytes[])",
                "multicall_with_deadline",
            )
            .add_method_alias(
                "sweepToken(address,uint256,address)",
                "sweep_token_with_recipient",
            )
            .add_method_alias(
                "sweepTokenWithFee(address,uint256,address,uint256,address)",
                "sweep_token_with_fee_and_recipient",
            )
            .add_method_alias(
                "unwrapWETH9(uint256,address)",
                "unwrap_weth9_with_recipient",
            )
            .add_method_alias(
                "unwrapWETH9WithFee(uint256,address,uint256,address)",
                "unwrap_weth9_with_fee_and_recipient",
            )
    });
    generate_contract_with_config("WETH9", |builder| {
        builder
            .add_network_str("1", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .add_network_str("4", "0xc778417E063141139Fce010982780140Aa0cD5Ab")
            .add_network_str("100", "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d")
    });
    generate_contract_with_config("IUniswapV3Factory", |builder| {
        builder
            .add_network_str("1", "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str("4", "0x1F98431c8aD98523631AE4a59f267346ea31F984")
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
