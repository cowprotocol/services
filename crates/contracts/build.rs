use {
    ethcontract::{
        common::{contract::Network, DeploymentInformation},
        Address,
    },
    ethcontract_generate::{loaders::TruffleLoader, ContractBuilder},
    std::{env, path::Path},
};

#[path = "src/paths.rs"]
mod paths;

const MAINNET: &str = "1";
const GOERLI: &str = "5";
const GNOSIS: &str = "100";
const SEPOLIA: &str = "11155111";

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existent files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");

    generate_contract_with_config("CoWSwapEthFlow", |builder| {
        builder
            .contract_mod_override("cowswap_eth_flow")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x40a50cf069e992aa4536211b23f286ef88752187"),
                    // <https://etherscan.io/tx/0x0247e3c15f59a52b099f192265f1c1e6227f48a280717b3eefd7a5d9d0c051a1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16169866)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0x40a50cf069e992aa4536211b23f286ef88752187"),
                    // <https://goerli.etherscan.io/tx/0x427f4e96a6de122720428c652258eb07b463869a32239f99a6e9b321d9584f9c>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8123017)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x40a50cf069e992aa4536211b23f286ef88752187"),
                    // <https://gnosisscan.io/tx/0x6280e079f454fbb5de3c52beddd64ca2b5be0a4b3ec74edfd5f47e118347d4fb>
                    deployment_information: Some(DeploymentInformation::BlockNumber(25414331)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    // <https://github.com/cowprotocol/ethflowcontract/blob/v1.1.0-artifacts/networks.prod.json#L11-L14>
                    address: addr("0x0b7795E18767259CC253a2dF471db34c72B49516"),
                    // <https://sepolia.etherscan.io/tx/0x558a7608a770b5c4f68fffa9b02e7908a40f61b557b435ea768a4c62cb79ae25>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4718739)),
                },
            )
    });
    generate_contract_with_config("CoWSwapOnchainOrders", |builder| {
        builder.contract_mod_override("cowswap_onchain_orders")
    });
    generate_contract_with_config("BalancerV2Authorizer", |builder| {
        builder.contract_mod_override("balancer_v2_authorizer")
    });
    generate_contract_with_config("BalancerV2BasePool", |builder| {
        builder.contract_mod_override("balancer_v2_base_pool")
    });
    generate_contract_with_config("BalancerV2BasePoolFactory", |builder| {
        builder.contract_mod_override("balancer_v2_base_pool_factory")
    });
    // Balancer addresses can be obtained from:
    // <https://github.com/balancer/balancer-subgraph-v2/blob/master/networks.yaml>
    generate_contract_with_config("BalancerV2Vault", |builder| {
        builder
            .contract_mod_override("balancer_v2_vault")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://etherscan.io/tx/0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12272146)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://goerli.etherscan.io/tx/0x116a2c379d6e496f7848d5704ed3fe0c6e1caa841dd1cac10f631b7bc71b0ec5>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4648099)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://gnosisscan.io/tx/0x21947751661e1b9197492f22779af1f5175b71dc7057869e5a8593141d40edf1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(24821598)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://sepolia.etherscan.io/tx/0xb22509c6725dd69a975ecb96a0c594901eeee6a279cc66d9d5191022a7039ee6>
                    deployment_information: Some(DeploymentInformation::BlockNumber(3418831)),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_factory")
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
                MAINNET,
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://etherscan.io/tx/0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12272147)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/goerli.html#ungrouped-active-current-contracts>
                GOERLI,
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://goerli.etherscan.io/tx/0x0ce1710e896fb090a2387e94a31e1ac40f3005de30388a63c44381f2c900d732>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4648101)),
                },
            )
        // Not available on Sepolia (only version ≥ 4)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config("BalancerV2WeightedPoolFactoryV3", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_factory_v3")
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
                MAINNET,
                Network {
                    address: addr("0x5Dd94Da3644DDD055fcf6B3E1aa310Bb7801EB8b"),
                    // <https://etherscan.io/tx/0x39f357b78c03954f0bcee2288bf3b223f454816c141ef20399a7bf38057254c4>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16520627)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/goerli.html#ungrouped-active-current-contracts>
                GOERLI,
                Network {
                    address: addr("0x26575A44755E0aaa969FDda1E4291Df22C5624Ea"),
                    // <https://goerli.etherscan.io/tx/0x20850573d9efcb8882046d116bc241f8ff9a5d925fcfa345441facb852366e74>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8456831)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xC128a9954e6c874eA3d62ce62B468bA073093F25"),
                    // <https://gnosisscan.io/tx/0x2ac3d873b6f43de6dd77525c7e5b68a8fc3a1dee40303e1b6a680b0285b26091>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26226256)),
                },
            )
        // Not available on Sepolia (only version ≥ 4)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config("BalancerV2WeightedPoolFactoryV4", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_factory_v4")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x897888115Ada5773E02aA29F775430BFB5F34c51"),
                    // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16878323)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"),
                    // <https://goerli.etherscan.io/tx/0xf573046881049ffeb65210adc5b76f41adbd2202f46593d22767e8bbd6c6198d>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8694778)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x6CaD2ea22BFA7F4C14Aae92E47F510Cd5C509bc7"),
                    // <https://gnosisscan.io/tx/0xcb6768bd92add227d46668357291e1d67c864769d353f9f0041c59ad2a3b21bf>
                    deployment_information: Some(DeploymentInformation::BlockNumber(27055829)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#pool-factories>
                SEPOLIA,
                Network {
                    address: addr("0x7920BFa1b2041911b354747CA7A6cDD2dfC50Cfd"),
                    // <https://sepolia.etherscan.io/tx/0x7dd392b586f1cdecfc635e7dd40ee1444a7836772811e59321fd4873ecfdf3eb>
                    deployment_information: Some(DeploymentInformation::BlockNumber(3424893)),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPool2TokensFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_2_tokens_factory")
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
                MAINNET,
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    // <https://etherscan.io/tx/0xf40c05058422d730b7035c254f8b765722935a5d3003ac37b13a61860adbaf08>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12349891)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/goerli.html#ungrouped-active-current-contracts>
                GOERLI,
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    // <https://goerli.etherscan.io/tx/0x5d5aa13cce6f81c36c69ad5aae6f5cb9cc6f8605a5eb1dc99815b5d74ae0796a>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4716924)),
                },
            )
        // Not available on Sepolia
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config("BalancerV2StablePoolFactoryV2", |builder| {
        builder
            .contract_mod_override("balancer_v2_stable_pool_factory_v2")
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
                MAINNET,
                Network {
                    address: addr("0x8df6efec5547e31b0eb7d1291b511ff8a2bf987c"),
                    // <https://etherscan.io/tx/0xef36451947ebd97b72278face57a53806e90071f4c902259db2db41d0c9a143d>
                    deployment_information: Some(DeploymentInformation::BlockNumber(14934936)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/goerli.html#ungrouped-active-current-contracts>
                GOERLI,
                Network {
                    address: addr("0xD360B8afb3d7463bE823bE1Ec3c33aA173EbE86e"),
                    // <https://goerli.etherscan.io/tx/0x71bdf2cb1d2cf4c1521d82f6821aa0bee2c144252c3ae0dd7d651cb5bbcbc860>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7169381)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xf23b4DB826DbA14c0e857029dfF076b1c0264843"),
                    // <https://gnosisscan.io/tx/0xe062237f0c8583375b10cf514d091781bfcd52d9ababbd324180770a5efbc6b1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(25415344)),
                },
            )
        // Not available on Sepolia
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config("BalancerV2LiquidityBootstrappingPoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_liquidity_bootstrapping_pool_factory")
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
                MAINNET,
                Network {
                    address: addr("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE"),
                    // <https://etherscan.io/tx/0x665ac1c7c5290d70154d9dfc1d91dc2562b143aaa9e8a77aa13e7053e4fe9b7c>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12871780)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0xb48Cc42C45d262534e46d5965a9Ac496F1B7a830"),
                    // <https://goerli.etherscan.io/tx/0x7dcb9e2026789e194e6e78605ac6a65e00392ba5d73e084d468e3dfbb188ea70>
                    deployment_information: Some(DeploymentInformation::BlockNumber(6993037)),
                },
            )
        // Not available on Sepolia
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config(
        "BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory",
        |builder| {
            builder
                .contract_mod_override(
                    "balancer_v2_no_protocol_fee_liquidity_bootstrapping_pool_factory",
                )
                .add_network(
                    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
                    MAINNET,
                    Network {
                        address: addr("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"),
                        // <https://etherscan.io/tx/0x298381e567ff6643d9b32e8e7e9ff0f04a80929dce3e004f6fa1a0104b2b69c3>
                        deployment_information: Some(DeploymentInformation::BlockNumber(13730248)),
                    },
                )
                .add_network(
                    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/goerli.html#ungrouped-active-current-contracts>
                    GOERLI,
                    Network {
                        address: addr("0xB0C726778C3AE4B3454D85557A48e8fa502bDD6A"),
                        // <https://goerli.etherscan.io/tx/0x278e68794c90221334e251974d65bbd7733f5fd7ef2617c978bf7c817828ce8d>
                        deployment_information: Some(DeploymentInformation::BlockNumber(6993471)),
                    },
                )
                .add_network(
                    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/gnosis.html#ungrouped-active-current-contracts>
                    GNOSIS,
                    Network {
                        address: addr("0x85a80afee867aDf27B50BdB7b76DA70f1E853062"),
                        // <https://gnosis.blockscout.com/tx/0xbd56fefdb27e4ff1c0852e405f78311d6bc2befabaf6c87a405ab19de8c1506a>
                        deployment_information: Some(DeploymentInformation::BlockNumber(25415236)),
                    },
                )
                .add_network(
                    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
                    SEPOLIA,
                    Network {
                        address: addr("0x45fFd460cC6642B8D8Fb12373DFd77Ceb0f4932B"),
                        // <https://sepolia.etherscan.io/tx/0xe0e8feb509a8aa8a1eaa0b0c4b34395ff2fd880fb854fbeeccc0af1826e395c9>
                        deployment_information: Some(DeploymentInformation::BlockNumber(3419649)),
                    },
                )
        },
    );
    generate_contract_with_config("BalancerV2ComposableStablePoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_composable_stable_pool_factory")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F"),
                    // <https://etherscan.io/tx/0x3b9e93ae050e59b3ca3657958ca30d1fd13fbc43208f8f0aa01ae992294f9961>
                    deployment_information: Some(DeploymentInformation::BlockNumber(15485885)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0xB848f50141F3D4255b37aC288C25C109104F2158"),
                    // <https://goerli.etherscan.io/tx/0x068e47605db29b7f9e5a8ba8bc7075fe3beab9801b4891b8656d6845f6477721>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7542764)),
                },
            )
        // Not available on Sepolia and Gnosis Chain
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/gnosis.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config("BalancerV2ComposableStablePoolFactoryV3", |builder| {
        builder
            .contract_mod_override("balancer_v2_composable_stable_pool_factory_v3")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0xdba127fBc23fb20F5929C546af220A991b5C6e01"),
                    // <https://etherscan.io/tx/0xd8c9ba758cb318beb0c9525b7621280a22b6dfe02cf725a3ece0718598f260ef>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16580899)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0xbfD9769b061E57e478690299011A028194D66e3C"),
                    // <https://goerli.etherscan.io/tx/0x63fe0afaaf0df4f197ea7681e99a899bed9fb0b9b3508441998dc3bbc75abef1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8456835)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"),
                    // <https://gnosisscan.io/tx/0x2abd7c865f8ab432b340f7de897192c677ffa254908fdec14091e0cd06962963>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26365805)),
                },
            )
        // Not available on Sepolia (only version ≥ 4)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
    });
    generate_contract_with_config("BalancerV2ComposableStablePoolFactoryV4", |builder| {
        builder
            .contract_mod_override("balancer_v2_composable_stable_pool_factory_v4")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0xfADa0f4547AB2de89D1304A668C39B3E09Aa7c76"),
                    // <https://etherscan.io/tx/0x3b61da162f3414c376cfe8b38d57ca6ba3c40b24446029ddab1187f4ae7c2bd7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16878679)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0x1802953277FD955f9a254B80Aa0582f193cF1d77"),
                    // <https://goerli.etherscan.io/tx/0xeb7c53925dfc372103b956df39bdc7b7360485838e451e74ce715cd13a65624a>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8695012)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xD87F44Df0159DC78029AB9CA7D7e57E7249F5ACD"),
                    // <https://gnosisscan.io/tx/0x2739416da7e44add08bdfb5e4e5a29ca981383b97162748887efcc5c1241b2f1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(27056416)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#deprecated-contracts>
                SEPOLIA,
                Network {
                    address: addr("0xA3fd20E29358c056B727657E83DFd139abBC9924"),
                    // <https://sepolia.etherscan.io/tx/0x9313a59ad9a95f2518076cbf4d0dc5f312e0b013a43a7ea4821cae2aa7a50aa2>
                    deployment_information: Some(DeploymentInformation::BlockNumber(3425277)),
                },
            )
    });
    generate_contract_with_config("BalancerV2ComposableStablePoolFactoryV5", |builder| {
        builder
            .contract_mod_override("balancer_v2_composable_stable_pool_factory_v5")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0xDB8d758BCb971e482B2C45f7F8a7740283A1bd3A"),
                    // <https://etherscan.io/tx/0x1fc28221925959c0713d04d9f9159255927ebb94b7fa76e4795db0e365643c07>
                    deployment_information: Some(DeploymentInformation::BlockNumber(17672478)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"),
                    // <https://goerli.etherscan.io/tx/0xbe4b6a7cc3849da725fdb5699432646e051e275b1058b83e97d62d595abd23e7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(9329440)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"),
                    // <https://gnosisscan.io/tx/0xcbf18b5a0d1f1fca9b30d08ab77d8554567c3bffa7efdd3add273073d20bb1e2>
                    deployment_information: Some(DeploymentInformation::BlockNumber(28900564)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
                SEPOLIA,
                Network {
                    address: addr("0xa523f47A933D5020b23629dDf689695AA94612Dc"),
                    // <https://sepolia.etherscan.io/tx/0x2c155dde7c480929991dd2a3344d9fdd20252f235370d46d0887b151dc0416bd>
                    deployment_information: Some(DeploymentInformation::BlockNumber(3872211)),
                },
            )
    });
    generate_contract("BalancerV2WeightedPool");
    generate_contract_with_config("BalancerV2StablePool", |builder| {
        builder.add_method_alias(
            "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),\
             uint256[],uint256,uint256)",
            "on_swap_with_balances",
        )
    });
    generate_contract("BalancerV2LiquidityBootstrappingPool");
    generate_contract("BalancerV2ComposableStablePool");
    generate_contract_with_config("BaoswapRouter", |builder| {
        builder.add_network_str(GNOSIS, "0x6093AeBAC87d62b1A5a4cEec91204e35020E38bE")
    });
    generate_contract("ERC20");
    generate_contract("ERC20Mintable");
    generate_contract_with_config("GPv2AllowListAuthentication", |builder| {
        builder
            .contract_mod_override("gpv2_allow_list_authentication")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://etherscan.io/tx/0xb84bf720364f94c749f1ec1cdf0d4c44c70411b716459aaccfd24fc677013375>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12593263)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://goerli.etherscan.io/tx/0x39dcf30baf887a5db54551a84de8bfdb6cf418bb284b09680d13aed17d5fa0c1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7020442)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://gnosisscan.io/tx/0x1a2d87a05a94bc6680a4faee31bbafbd74e9ddb63dd3941c717b5c609c08b957>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16465099)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://sepolia.etherscan.io/tx/0x73c54c75b3f382304f3adf33e3876c8999fb10df786d4a902733369251033cd1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4717469)),
                },
            )
    });
    generate_contract_with_config("GPv2Settlement", |builder| {
        builder
            .contract_mod_override("gpv2_settlement")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://etherscan.io/tx/0xf49f90aa5a268c40001d1227b76bb4dd8247f18361fcad9fffd4a7a44f1320d3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12593265)),
                },
            )
            .add_network(
                GOERLI,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://goerli.etherscan.io/tx/0x982f089060ff66e19d0683ef1cc6a637297331a9ba95b65d8eb84b9f8dc64b04>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7020473)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://blockscout.com/xdai/mainnet/tx/0x9ddc538f89cd8433f4a19bc4de0de27e7c68a1d04a14b327185e4bba9af87133>
                    deployment_information: Some(DeploymentInformation::BlockNumber(16465100)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://sepolia.etherscan.io/tx/0x6bba22a00ffcff6bca79aced546e18d2a5a4f4e484a4e4dbafab13daf42f718d>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4717488)),
                },
            )
    });
    generate_contract("GnosisSafe");
    generate_contract_with_config("GnosisSafeCompatibilityFallbackHandler", |builder| {
        builder.add_method_alias("isValidSignature(bytes,bytes)", "is_valid_signature_legacy")
    });
    generate_contract("GnosisSafeProxy");
    generate_contract("GnosisSafeProxyFactory");
    generate_contract_with_config("HoneyswapRouter", |builder| {
        builder.add_network_str(GNOSIS, "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77")
    });
    generate_contract_with_config("HooksTrampoline", |builder| {
        // <https://github.com/cowprotocol/hooks-trampoline/blob/993427166ade6c65875b932f853776299290ac4b/networks.json>
        builder
            .add_network_str(MAINNET, "0x01DcB88678aedD0C4cC9552B20F4718550250574")
            .add_network_str(GOERLI, "0x01DcB88678aedD0C4cC9552B20F4718550250574")
            .add_network_str(GNOSIS, "0x01DcB88678aedD0C4cC9552B20F4718550250574")
            .add_network_str(SEPOLIA, "0x01DcB88678aedD0C4cC9552B20F4718550250574")
    });
    generate_contract("IUniswapLikeRouter");
    generate_contract("IUniswapLikePair");
    // EIP-1271 contract - SignatureValidator
    generate_contract("ERC1271SignatureValidator");
    generate_contract_with_config("PancakeRouter", |builder| {
        builder.add_network_str(MAINNET, "0xEfF92A263d31888d860bD50809A8D171709b7b1c")
    });
    generate_contract_with_config("SushiSwapRouter", |builder| {
        // <https://docs.sushi.com/docs/Products/Classic%20AMM/Deployment%20Addresses>
        builder
            .add_network_str(MAINNET, "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")
            .add_network_str(GOERLI, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str(GNOSIS, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
    });
    generate_contract_with_config("SwaprRouter", |builder| {
        // <https://swapr.gitbook.io/swapr/contracts>
        builder
            .add_network_str(MAINNET, "0xb9960d9bca016e9748be75dd52f02188b9d0829f")
            .add_network_str(GNOSIS, "0xE43e60736b1cb4a75ad25240E2f9a62Bff65c0C0")
    });
    generate_contract("ISwaprPair");
    generate_contract_with_config("UniswapV2Factory", |builder| {
        // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/factory>
        builder
            .add_network_str(MAINNET, "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str(GOERLI, "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str(GNOSIS, "0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7")
        // Not available on Sepolia
    });
    generate_contract_with_config("UniswapV2Router02", |builder| {
        // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/router-02>
        builder
            .add_network_str(MAINNET, "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str(GOERLI, "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str(GNOSIS, "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77")
        // Not available on Sepolia
    });
    generate_contract_with_config("UniswapV3SwapRouter", |builder| {
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        builder
            .add_network_str(MAINNET, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            .add_network_str(GOERLI, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            .add_network_str(SEPOLIA, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
        // Not available on Gnosis Chain
    });
    generate_contract("UniswapV3Pool");
    generate_contract_with_config("WETH9", |builder| {
        // Note: the WETH address must be consistent with the one used by the ETH-flow
        // contract
        builder
            .add_network_str(MAINNET, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .add_network_str(GOERLI, "0xB4FBF271143F4FBf7B91A5ded31805e42b2208d6")
            .add_network_str(GNOSIS, "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d")
            .add_network_str(SEPOLIA, "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14")
    });
    generate_contract_with_config("IUniswapV3Factory", |builder| {
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        builder
            .add_network_str(MAINNET, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(GOERLI, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(SEPOLIA, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
        // Not available on Gnosis Chain
    });
    generate_contract_with_config("IZeroEx", |builder| {
        // <https://docs.0xprotocol.org/en/latest/basics/addresses.html?highlight=contracts#addresses>
        // <https://github.com/0xProject/protocol/blob/652d4226229c97895ae9350bbf276370ebb38c5e/packages/contract-addresses/addresses.json>
        builder
            .add_network_str(MAINNET, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(SEPOLIA, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_method_alias(
                "_transformERC20((address,address,address,uint256,uint256,(uint32,bytes)[],bool,\
                 address))",
                "_transform_erc_20",
            )
            .add_method_alias(
                "_fillRfqOrder((address,address,uint128,uint128,address,address,address,bytes32,\
                 uint64,uint256),(uint8,uint8,bytes32,bytes32),uint128,address,bool,address)",
                "_fill_rfq_order",
            )
            .add_method_alias(
                "_fillLimitOrder((address,address,uint128,uint128,uint128,address,address,address,\
                 address,bytes32,uint64,uint256),(uint8,uint8,bytes32,bytes32),uint128,address,\
                 address)",
                "_fill_limit_order",
            )
            .add_method_alias(
                "_fillOtcOrder((address,address,uint128,uint128,address,address,address,uint256),\
                 (uint8,uint8,bytes32,bytes32),uint128,address,bool,address)",
                "_fill_otc_order",
            )
    });
    generate_contract_with_config("CowProtocolToken", |builder| {
        builder
            .add_network_str(MAINNET, "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB")
            .add_network_str(GOERLI, "0x91056D4A53E1faa1A84306D4deAEc71085394bC8")
            .add_network_str(GNOSIS, "0x177127622c4A00F3d409B75571e12cB3c8973d3c")
            .add_network_str(SEPOLIA, "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59")
    });

    // Unofficial Uniswap v2 liquidity on the Sepolia testnet.
    generate_contract_with_config("TestnetUniswapV2Router02", |builder| {
        // <https://github.com/eth-clients/sepolia/issues/47#issuecomment-1681562464>
        builder.add_network_str(SEPOLIA, "0x86dcd3293C53Cf8EFd7303B57beb2a3F671dDE98")
    });

    // Chainalysis oracle for sanctions screening
    generate_contract_with_config("ChainalysisOracle", |builder| {
        builder.add_network_str(MAINNET, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
    });

    // Support contracts used for trade and token simulations.
    generate_contract("Trader");
    generate_contract("Solver");

    // Support contracts used for various order simulations.
    generate_contract("Balances");
    generate_contract("Signatures");
    generate_contract("SimulateCode");

    // Support contract used for solver fee simulations.
    generate_contract("AnyoneAuthenticator");
    generate_contract("Swapper");

    // Support contract used for global block stream.
    generate_contract("FetchBlock");

    // Contract for batching multiple `eth_call`s into a single one.
    generate_contract("Multicall");

    // Test Contract for incrementing arbitrary counters.
    generate_contract("Counter");

    // Test Contract for using up a specified amount of gas.
    generate_contract("GasHog");
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
        .write_to_file(Path::new(&dest).join(format!("{name}.rs")))
        .unwrap();
}

fn addr(s: &str) -> Address {
    s.parse().unwrap()
}
