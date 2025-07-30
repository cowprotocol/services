use {
    ethcontract::{
        Address,
        common::{DeploymentInformation, contract::Network},
    },
    ethcontract_generate::{ContractBuilder, loaders::TruffleLoader},
    std::{env, path::Path},
};

#[path = "src/paths.rs"]
mod paths;

const MAINNET: &str = "1";
const GOERLI: &str = "5";
const GNOSIS: &str = "100";
const SEPOLIA: &str = "11155111";
const ARBITRUM_ONE: &str = "42161";
const BASE: &str = "8453";
const POLYGON: &str = "137";
const AVALANCHE: &str = "43114";
const BNB: &str = "56";
const OPTIMISM: &str = "10";

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existent files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");

    generate_contract_with_config("AaveFlashLoanSolverWrapper", |builder| {
        let mut builder = builder;
        for network in [
            MAINNET,
            GNOSIS,
            SEPOLIA,
            ARBITRUM_ONE,
            BASE,
            POLYGON,
            AVALANCHE,
        ] {
            builder = builder.add_network(
                network,
                Network {
                    address: addr("0x7d9c4dee56933151bc5c909cfe09def0d315cb4a"),
                    deployment_information: None,
                },
            );
        }
        builder
    });
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x6DFE75B5ddce1ADE279D4fa6BD6AeF3cBb6f49dB"),
                    // <https://arbiscan.io/tx/0xa4066ca77bbe1f21776b4c26315ead3b1c054b35814b49e0c35afcbff23e1b8d>
                    deployment_information: Some(DeploymentInformation::BlockNumber(204747458)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x3C3eA1829891BC9bEC3d06A81d5d169e52a415e3"),
                    // <https://basescan.org/tx/0xc3555c4b065867cbf34382438e1bbaf8ee39eaf10fb0c70940c8955962e76e2c>
                    deployment_information: Some(DeploymentInformation::BlockNumber(21490258)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
                    // <https://snowscan.xyz/tx/0x71a2ed9754247210786effa3269bc6eb68b7521b5052ac9f205af7ac364f608f>
                    deployment_information: Some(DeploymentInformation::BlockNumber(60496408)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
                    // <https://bscscan.com/tx/0x959a60a42d36e0efd247b3cf19ed9d6da503d01bce6f87ed31e5e5921111222e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(48411237)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
                    // <https://optimistic.etherscan.io/tx/0x0644f10f7ae5448240fc592ad21abf4dabac473a9d80904af5f7865f2d6509e2>
                    deployment_information: Some(DeploymentInformation::BlockNumber(134607215)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
                    // <https://polygonscan.com/tx/0xc3781c19674d97623d13afc938fca94d53583f4051020512100e84fecd230f91>
                    deployment_information: Some(DeploymentInformation::BlockNumber(71296258)),
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://arbiscan.io/tx/0xe2c3826bd7b15ef8d338038769fe6140a44f1957a36b0f27ab321ab6c68d5a8e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(222832)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://basescan.org/tx/0x0dc2e3d436424f2f038774805116896d31828c0bf3795a6901337bdec4e0dff6>
                    deployment_information: Some(DeploymentInformation::BlockNumber(1196036)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://snowscan.xyz/tx/0xc49af0372feb032e0edbba6988410304566b1fd65546c01ced620ac3c934120f>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26386141)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://bscscan.com/tx/0x1de8caa6c54ff9a25600e26d80865d84c9cc4d33c2b98611240529ee7de5cd74>
                    deployment_information: Some(DeploymentInformation::BlockNumber(22691002)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://optimistic.etherscan.io/tx/0xa03cb990595df9eed6c5db17a09468cab534aed5f5589a06c0bb3d19dd2f7ce9>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7003431)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://polygonscan.com/tx/0x66f275a2ed102a5b679c0894ced62c4ebcb2a65336d086a916eb83bd1fe5c8d2>
                    deployment_information: Some(DeploymentInformation::BlockNumber(15832990)),
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
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x94f68b54191F62f781Fe8298A8A5Fa3ed772d227"),
                    // <https://snowscan.xyz/tx/0xdf2c77743cc9287df2022cd6c5f9209ecfecde07371717ab0427d96042a88640>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26389236)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xA0DAbEBAAd1b243BBb243f933013d560819eB66f"),
                    // <https://optimistic.etherscan.io/tx/0xc5e79fb00b9a8e2c89b136aae0be098e58f8e832ede13e8079213a75c9cd9c08>
                    deployment_information: Some(DeploymentInformation::BlockNumber(72832703)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x82e4cFaef85b1B6299935340c964C942280327f4"),
                    // <https://polygonscan.com/tx/0x2bc079c0e725f43670898b474afedf38462feee72ef8e874a1efcec0736672fc>
                    deployment_information: Some(DeploymentInformation::BlockNumber(39036828)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x6e4cF292C5349c79cCd66349c3Ed56357dD11B46"),
                    // <https://bscscan.com/tx/0x91107b9581e18ec0a4a575d4713bdd7b1fc08656c35522d216307930aa4de7b6>
                    deployment_information: Some(DeploymentInformation::BlockNumber(25474982)),
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xc7E5ED1054A24Ef31D827E6F86caA58B3Bc168d7"),
                    // <https://arbiscan.io/tx/0x167fe7eb776d1be36b21402d8ae120088c393e28ae7ca0bd1defac84e0f2848b>
                    deployment_information: Some(DeploymentInformation::BlockNumber(72222060)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x4C32a8a8fDa4E24139B51b456B42290f51d6A1c4"),
                    // <https://basescan.org/tx/0x0732d3a45a3233a134d6e0e72a00ca7a971d82cdc51f71477892ac517bf0d4c9>
                    deployment_information: Some(DeploymentInformation::BlockNumber(1204869)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"),
                    // <https://snowscan.xyz/tx/0xa3fc8aab3b9baba3905045a53e52a47daafe79d4aa26d4fef5c51f3840aa55fa>
                    deployment_information: Some(DeploymentInformation::BlockNumber(27739006)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"),
                    // <https://optimistic.etherscan.io/tx/0xad915050179db368e43703f3ee1ec55ff5e5e5e0268c15f8839c9f360caf7b0b>
                    deployment_information: Some(DeploymentInformation::BlockNumber(82737545)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0xFc8a407Bba312ac761D8BFe04CE1201904842B76"),
                    // <https://polygonscan.com/tx/0x65e6b13231c2c5656357005a9e419ad6697178ae74eda1ea7522ecdafcf77136>
                    deployment_information: Some(DeploymentInformation::BlockNumber(40611103)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"),
                    // <https://bscscan.com/tx/0xc7fada60761e3240332c4cbd169633f1828b2a15de23f0148db9d121afebbb4b>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26665331)),
                },
            )
        // Not available on Base
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xCF0a32Bbef8F064969F21f7e02328FB577382018"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(222864)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xdAE7e32ADc5d490a43cCba1f0c736033F2b4eFca"),
                    // <https://optimistic.etherscan.io/tx/0xd5754950d47179d822ea976a8b2af82ffa80e992cf0660b02c0c218359cc8987>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7005512)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://polygonscan.com/tx/0xb8ac851249cc95bc0943ef0732d28bbd53b0b36c7dd808372666acd8c5f26e1c>
                    deployment_information: Some(DeploymentInformation::BlockNumber(15832998)),
                },
            )
        // Not available on Sepolia, Base, Avalanche and BNB
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xEF44D6786b2b4d544b7850Fe67CE6381626Bf2D6"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(14244664)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xeb151668006CD04DAdD098AFd0a82e78F77076c3"),
                    // <https://optimistic.etherscan.io/tx/0xcf9f0bd731ded0e513708200df28ac11d17246fb53fc852cddedf590e41c9c03>
                    deployment_information: Some(DeploymentInformation::BlockNumber(11088891)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0xcA96C4f198d343E251b1a01F3EBA061ef3DA73C1"),
                    // <https://polygonscan.com/tx/0xa2c41d014791888a29a9491204446c1b9b2f5dee3f3eb31ad03f290259067b44>
                    deployment_information: Some(DeploymentInformation::BlockNumber(29371951)),
                },
            )
        // Not available on Sepolia, Base, Avalanche and BNB
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x142B9666a0a3A30477b052962ddA81547E7029ab"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(222870)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE"),
                    // <https://polygonscan.com/tx/0xd9b5b9a9e6ea17a87f85574e93577e3646c9c2f9c8f38644f936949e6c853288>
                    deployment_information: Some(DeploymentInformation::BlockNumber(17116402)),
                },
            )
        // Not available on Sepolia, Base, Avalanche, BNB and Optimism
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
                .add_network(
                    ARBITRUM_ONE,
                    Network {
                        address: addr("0x1802953277FD955f9a254B80Aa0582f193cF1d77"),
                        deployment_information: Some(DeploymentInformation::BlockNumber(4859669)),
                    },
                )
                .add_network(
                    BASE,
                    Network {
                        address: addr("0x0c6052254551EAe3ECac77B01DFcf1025418828f"),
                        // <https://basescan.org/tx/0x0529de9dbe772f4b4f48da93ae2c2d2c46e3d3221ced9e0c4063a7a5bc47e874>
                        deployment_information: Some(DeploymentInformation::BlockNumber(1206531)),
                    },
                )
                .add_network(
                    AVALANCHE,
                    Network {
                        address: addr("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"),
                        // <https://snowscan.xyz/tx/0x33a75d83436ae9fcda4b4986713417bf3dc80d9ceb8d2541817846b1ac579d9f>
                        deployment_information: Some(DeploymentInformation::BlockNumber(26386552)),
                    },
                )
                .add_network(
                    BNB,
                    Network {
                        address: addr("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"),
                        // <https://bscscan.com/tx/0x8b964b97e6091bd41c93002c558d49adc26b8b31d2b30f3a33babbbbe8c55f47>
                        deployment_information: Some(DeploymentInformation::BlockNumber(22691243)),
                    },
                )
                .add_network(
                    OPTIMISM,
                    Network {
                        address: addr("0xf302f9F50958c5593770FDf4d4812309fF77414f"),
                        // <https://optimistic.etherscan.io/tx/0x14fb43f051eebdec645abf0125e52348dc875b0887b689f8db026d75f9c78dda>
                        deployment_information: Some(DeploymentInformation::BlockNumber(7005915)),
                    },
                )
                .add_network(
                    POLYGON,
                    Network {
                        address: addr("0x41B953164995c11C81DA73D212ED8Af25741b7Ac"),
                        // <https://polygonscan.com/tx/0x125bc007a86d771f8dc8f5fa1017de6e5a11162a458a72f25814503404bbeb0b>
                        deployment_information: Some(DeploymentInformation::BlockNumber(22067480)),
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xaEb406b0E430BF5Ea2Dc0B9Fe62E4E53f74B3a33"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(23227044)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0xf302f9F50958c5593770FDf4d4812309fF77414f"),
                    // <https://bscscan.com/tx/0x6c6e1c72c91c75714f698049f1c7b66d8f2baced54e0dd2522dfadff27b5ccf1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(22691193)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xf145caFB67081895EE80eB7c04A30Cf87f07b745"),
                    // <https://optimistic.etherscan.io/tx/0xad2f330ad865dc7955376a3d9733486b38c53ba0d4757ad4e1b63b105401c506>
                    deployment_information: Some(DeploymentInformation::BlockNumber(22182522)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x136FD06Fa01eCF624C7F2B3CB15742c1339dC2c4"),
                    // <https://polygonscan.com/tx/0xe5d908be686056f1519663a407167c088924f60d29c799ec74438b9de891989e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(32774224)),
                },
            )
        // Not available on Sepolia, Gnosis Chain, Base and Avalanche
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/gnosis.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x1c99324EDC771c82A0DCCB780CC7DDA0045E50e7"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(58948370)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0xacAaC3e6D6Df918Bf3c809DFC7d42de0e4a72d4C"),
                    // <https://bscscan.com/tx/0xfe0c47c2b124a059d11704c1bd1815dcc554834ae0c2d11c433946226015619f>
                    deployment_information: Some(DeploymentInformation::BlockNumber(25475700)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xe2E901AB09f37884BA31622dF3Ca7FC19AA443Be"),
                    // <https://optimistic.etherscan.io/tx/0x2bb1c3fbf1f370c6e20ecda36b555de1a4426340908055c4274823e31f92210e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(72832821)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x7bc6C0E73EDAa66eF3F6E2f27b0EE8661834c6C9"),
                    // <https://polygonscan.com/tx/0xb189a45eac7ea59c0bb638b5ae6c4c93f9877f31ce826e96b792a9154e7a32a7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(39037615)),
                },
            )
        // Not available on Sepolia (only version ≥ 4) and on Base (only version ≥ 5)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x2498A2B0d6462d2260EAC50aE1C3e03F4829BA95"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(72235860)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x3B1eb8EB7b43882b385aB30533D9A2BeF9052a98"),
                    // <https://snowscan.xyz/tx/0x7b396102e767ec5f2bc06fb2c9d7fb704d0ddc537c04f28cb538c6de7cc4261e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(29221425)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x1802953277FD955f9a254B80Aa0582f193cF1d77"),
                    // <https://bscscan.com/tx/0x2819b490b5e04e27d66476730411df8e572bc33038aa869a370ecfa852de0cbf>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26666380)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x1802953277FD955f9a254B80Aa0582f193cF1d77"),
                    // <https://optimistic.etherscan.io/tx/0x5d6c515442188eb4af83524618333c0fbdab0df809af01c4e7a9e380f1841199>
                    deployment_information: Some(DeploymentInformation::BlockNumber(82748180)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x6Ab5549bBd766A43aFb687776ad8466F8b42f777"),
                    // <https://polygonscan.com/tx/0x2cea6a0683e67ebdb7d4a1cf1ad303126c5f228f05f8c9e2ccafdb1f5a024376>
                    deployment_information: Some(DeploymentInformation::BlockNumber(40613553)),
                },
            )
        // Not available on Base
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xA8920455934Da4D853faac1f94Fe7bEf72943eF1"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(110212282)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x8df317a729fcaA260306d7de28888932cb579b88"),
                    // <https://basescan.org/tx/0x1d291ba796b0397d73581b17695cf0e53e61551e419c43d11d81198b00c2bfd3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(1204710)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0xE42FFA682A26EF8F25891db4882932711D42e467"),
                    // <https://snowscan.xyz/tx/0x000659feb0831fc511f5c2ad12f3b2d466152b753c805fcb06e848701fd1b4b7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(32478827)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x4fb47126Fa83A8734991E41B942Ac29A3266C968"),
                    // <https://bscscan.com/tx/0x5bdfed936f82800e80543d5212cb287dceebb52c29133838acbe7e148bf1a447>
                    deployment_information: Some(DeploymentInformation::BlockNumber(29877945)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x043A2daD730d585C44FB79D2614F295D2d625412"),
                    // <https://optimistic.etherscan.io/tx/0xa141b35dbbb18154e2452b1ae6ab7d82a6555724a878b5fccff40e18c8ae3484>
                    deployment_information: Some(DeploymentInformation::BlockNumber(106752707)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b"),
                    // <https://polygonscan.com/tx/0xa3d9a1cf00eaca469d6f9ec2fb836bbbfdfbc3b0eeadc07619bb9e695bfdecb8>
                    deployment_information: Some(DeploymentInformation::BlockNumber(44961548)),
                },
            )
    });
    generate_contract_with_config("BalancerV2ComposableStablePoolFactoryV6", |builder| {
        builder
            .contract_mod_override("balancer_v2_composable_stable_pool_factory_v6")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x5B42eC6D40f7B7965BE5308c70e2603c0281C1E9"),
                    // <https://etherscan.io/tx/0x4149cadfe5d3431205d9819fca44ed7a4c2b101adc51adc75cc4586dee237be8>
                    deployment_information: Some(DeploymentInformation::BlockNumber(19314764)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x47B489bf5836f83ABD928C316F8e39bC0587B020"),
                    // <https://gnosisscan.io/tx/0xc3fc1fb96712a0659b7e9e5f406f63bdf5cbd5df9e04f0372c28f75785036791>
                    deployment_information: Some(DeploymentInformation::BlockNumber(32650879)),
                },
            )
            .add_network(
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
                SEPOLIA,
                Network {
                    address: addr("0x05503B3aDE04aCA81c8D6F88eCB73Ba156982D2B"),
                    // <https://sepolia.etherscan.io/tx/0x53aa3587002469b758e2bb87135d9599fd06e7be944fe61c7f82045c45328566>
                    deployment_information: Some(DeploymentInformation::BlockNumber(5369821)),
                },
            )
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"),
                    // <https://arbiscan.io/tx/0xfa1e7642e135fb32dc14c990b851e5e7a0ac7a463e3a60c5003ae4142396f45e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(184805448)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x956CCab09898C0AF2aCa5e6C229c3aD4E93d9288"),
                    // <https://basescan.org/tx/0x5d3342faf0368b939daa93247536afa26cc72c83de52ba7711ae1b8646688467>
                    deployment_information: Some(DeploymentInformation::BlockNumber(11099703)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0xb9F8AB3ED3F3aCBa64Bc6cd2DcA74B7F38fD7B88"),
                    // <https://snowscan.xyz/tx/0x246248ad396826dbfbdc5360cb9cbbdb3a672efa08cc745d1670900888c58c7b>
                    deployment_information: Some(DeploymentInformation::BlockNumber(42186350)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x6B5dA774890Db7B7b96C6f44e6a4b0F657399E2e"),
                    // <https://bscscan.com/tx/0x6784ab50138c7488bc14d4d9beb6a9e1ddc209a45f0a96b4ee98a7db84167dea>
                    deployment_information: Some(DeploymentInformation::BlockNumber(36485719)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"),
                    // <https://optimistic.etherscan.io/tx/0xa38b696479f35a9751ca8b1f0ddeb160188b3146113975b8c2b657c2fe7d7fd2>
                    deployment_information: Some(DeploymentInformation::BlockNumber(116694338)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0xEAedc32a51c510d35ebC11088fD5fF2b47aACF2E"),
                    // <https://polygonscan.com/tx/0x7b9678ad538b1cd3f3a03e63455e7d49a1bc716ea42310fbf99df4bf93ecfdfa>
                    deployment_information: Some(DeploymentInformation::BlockNumber(53996258)),
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
    generate_contract_with_config("BalancerV3BatchRouter", |builder| {
        builder
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1"),
                    // <https://etherscan.io/tx/0x41cb8619fb92dd532eb09b0e81fd4ce1c6006a10924893f02909e36a317777f3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(21339510)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b"),
                    // <https://gnosisscan.io/tx/0xeafddbace9f445266f851ef1d92928e3d01a4622a1a6780b41ac52d5872f12ab>
                    deployment_information: Some(DeploymentInformation::BlockNumber(37377506)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    address: addr("0xC85b652685567C1B074e8c0D4389f83a2E458b1C"),
                    // <https://sepolia.etherscan.io/tx/0x95ed8e1aaaa7bdc5881f3c8fc5a4914a66639bee52987c3a1ea88545083b0681>
                    deployment_information: Some(DeploymentInformation::BlockNumber(7219301)),
                },
            )
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E"),
                    // <https://arbiscan.io/tx/0xa7968c6bc0775208ffece789c6e5d09b0eea5f2c3ed2806e9bd94fb0b978ff0f>
                    deployment_information: Some(DeploymentInformation::BlockNumber(297828544)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x85a80afee867aDf27B50BdB7b76DA70f1E853062"),
                    // <https://basescan.org/tx/0x47b81146714630ce50445bfa28872a36973acedf785317ca423498810ec8e76c>
                    deployment_information: Some(DeploymentInformation::BlockNumber(25347205)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0xc9b36096f5201ea332Db35d6D195774ea0D5988f"),
                    // <https://snowscan.xyz/tx/0x3bfaba7135ee2d67d98f20ee1aa4c8b7e81e47be64223376f3086bab429ac806>
                    deployment_information: Some(DeploymentInformation::BlockNumber(59965747)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E"),
                    // <https://optimistic.etherscan.io/tx/0xf370aab0d652f3e0f7c34e1a53e1afd98e86c487138300b0939d4e54b0088b67>
                    deployment_information: Some(DeploymentInformation::BlockNumber(133969588)),
                },
            )
    });
    generate_contract_with_config("BaoswapRouter", |builder| {
        builder.add_network_str(GNOSIS, "0x6093AeBAC87d62b1A5a4cEec91204e35020E38bE")
    });
    generate_contract("ERC20");
    generate_contract("ERC20Mintable");
    generate_contract("ERC3156FlashLoanSolverWrapper");
    generate_contract_with_config("FlashLoanRouter", |builder| {
        let mut builder = builder;
        for network in [
            MAINNET,
            GNOSIS,
            SEPOLIA,
            ARBITRUM_ONE,
            BASE,
            POLYGON,
            AVALANCHE,
        ] {
            builder = builder.add_network(
                network,
                Network {
                    address: addr("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
                    deployment_information: None,
                },
            );
        }
        builder
    });
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://arbiscan.io/tx/0xe994adff141a2e72bd9dab3eb7b3480637013bdfb1aa42c62d9d6c90de091237>
                    deployment_information: Some(DeploymentInformation::BlockNumber(204702129)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://basescan.org/tx/0x5497004d2a37c9eafd0bd1e5861a67d3a209c5b845724166e3dbca9527ee05ec>
                    deployment_information: Some(DeploymentInformation::BlockNumber(21407137)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://snowscan.xyz/tx/0xa58fc76846917779d7bcbb7d34f4a2a44aab2b702ef983594e34e6972a0c626b>
                    deployment_information: Some(DeploymentInformation::BlockNumber(59891351)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://bscscan.com/tx/0x8da639c62eb4a810573c178ed245184944d66c834122e3f88994ebf679b50e34>
                    deployment_information: Some(DeploymentInformation::BlockNumber(48173639)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://optimistic.etherscan.io/tx/0x5b6403b485e369ce524d04234807df782e6639e55a7c1d859f0a67925d9a5f49>
                    deployment_information: Some(DeploymentInformation::BlockNumber(134254466)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://polygonscan.com/tx/0x686e4bbcfd6ebae91f0fcc667407c831953629877ec622457916729de3d461c3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(45854728)),
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
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(204704802)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://basescan.org/tx/0x00a3c4e2dc4241465208beeba27e90a9ce3159ad4f41581c4c3a1ef02d6e37cb>
                    deployment_information: Some(DeploymentInformation::BlockNumber(21407238)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://snowscan.xyz/tx/0x374b84f0ea6bc554abc3ffdc3fbce4374fefc76f2bd25e324ce95a62cafcc142>
                    deployment_information: Some(DeploymentInformation::BlockNumber(59891356)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://bscscan.com/tx/0x9e0c16a655ceadcb95ba2de3bf59d2b3a3d10cce7bdf52aa5520164b58ffd969>
                    deployment_information: Some(DeploymentInformation::BlockNumber(48173641)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://optimistic.etherscan.io/tx/0xd1bbd68ee6b0eecf6f883e148284fc4fb4c960299b75004dfddd5135246cd5eb>
                    deployment_information: Some(DeploymentInformation::BlockNumber(134254624)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://polygonscan.com/tx/0x0e24d3a2a8530eaad5ae62e54e64d57665a77ce3970227d20c1b77da315cbbf6>
                    deployment_information: Some(DeploymentInformation::BlockNumber(45859743)),
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
            .add_network_str(MAINNET, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(GOERLI, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(GNOSIS, "0x01DcB88678aedD0C4cC9552B20F4718550250574")
            .add_network_str(SEPOLIA, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(ARBITRUM_ONE, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(BASE, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(AVALANCHE, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(BNB, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(OPTIMISM, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
            .add_network_str(POLYGON, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
    });
    generate_contract("IAavePool");
    generate_contract("IFlashLoanSolverWrapper");
    generate_contract("IUniswapLikeRouter");
    generate_contract("IUniswapLikePair");
    // EIP-1271 contract - SignatureValidator
    generate_contract("ERC1271SignatureValidator");
    generate_contract_with_config("PancakeRouter", |builder| {
        builder
            .add_network_str(MAINNET, "0xEfF92A263d31888d860bD50809A8D171709b7b1c")
            .add_network_str(ARBITRUM_ONE, "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb")
            .add_network_str(BASE, "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb")
            .add_network_str(BNB, "0x10ED43C718714eb63d5aA57B78B54704E256024E")
    });
    generate_contract_with_config("SushiSwapRouter", |builder| {
        // <https://docs.sushi.com/contracts/cpamm>
        builder
            .add_network_str(MAINNET, "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")
            .add_network_str(GOERLI, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str(GNOSIS, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str(ARBITRUM_ONE, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str(BASE, "0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891")
            .add_network_str(AVALANCHE, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str(BNB, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
            .add_network_str(OPTIMISM, "0x2ABf469074dc0b54d793850807E6eb5Faf2625b1")
            .add_network_str(POLYGON, "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506")
    });
    generate_contract_with_config("SwaprRouter", |builder| {
        // <https://swapr.gitbook.io/swapr/contracts>
        builder
            .add_network_str(MAINNET, "0xb9960d9bca016e9748be75dd52f02188b9d0829f")
            .add_network_str(GNOSIS, "0xE43e60736b1cb4a75ad25240E2f9a62Bff65c0C0")
            .add_network_str(ARBITRUM_ONE, "0x530476d5583724A89c8841eB6Da76E7Af4C0F17E")
        // Not available on Base
    });
    generate_contract("ISwaprPair");
    generate_contract_with_config("UniswapV2Factory", |builder| {
        // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/factory>
        builder
            .add_network_str(MAINNET, "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str(GOERLI, "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
            .add_network_str(GNOSIS, "0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7")
            .add_network_str(ARBITRUM_ONE, "0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9")
            .add_network_str(BASE, "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6")
            .add_network_str(SEPOLIA, "0xF62c03E08ada871A0bEb309762E260a7a6a880E6")
            .add_network_str(AVALANCHE, "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C")
            .add_network_str(BNB, "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6")
            .add_network_str(OPTIMISM, "0x0c3c1c532F1e39EdF36BE9Fe0bE1410313E074Bf")
            .add_network_str(POLYGON, "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C")
    });
    generate_contract_with_config("UniswapV2Router02", |builder| {
        // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/router-02>
        builder
            .add_network_str(MAINNET, "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str(GOERLI, "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str(GNOSIS, "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77")
            .add_network_str(ARBITRUM_ONE, "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24")
            .add_network_str(BASE, "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24")
            .add_network_str(SEPOLIA, "0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3")
            .add_network_str(AVALANCHE, "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24")
            .add_network_str(BNB, "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24")
            .add_network_str(OPTIMISM, "0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2")
            .add_network_str(POLYGON, "0xedf6066a2b290C185783862C7F4776A2C8077AD1")
    });
    generate_contract_with_config("UniswapV3SwapRouter", |builder| {
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        builder
            .add_network_str(MAINNET, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            .add_network_str(GOERLI, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            .add_network_str(SEPOLIA, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            .add_network_str(ARBITRUM_ONE, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            // For Base, Avalanche and BNB it is only available SwapRouter02
            // <https://docs.uniswap.org/contracts/v3/reference/deployments/base-deployments>
            .add_network_str(BASE, "0x2626664c2603336E57B271c5C0b26F421741e481")
            .add_network_str(AVALANCHE, "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE")
            .add_network_str(BNB, "0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2")
            .add_network_str(OPTIMISM, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
            .add_network_str(POLYGON, "0xE592427A0AEce92De3Edee1F18E0157C05861564")
        // Not available on Gnosis Chain
    });
    generate_contract("UniswapV3Pool");
    generate_contract_with_config("UniswapV3QuoterV2", |builder| {
        // <https://docs.uniswap.org/contracts/v3/reference/deployments/>
        builder
            .add_network_str(MAINNET, "0x61fFE014bA17989E743c5F6cB21bF9697530B21e")
            .add_network_str(ARBITRUM_ONE, "0x61fFE014bA17989E743c5F6cB21bF9697530B21e")
            .add_network_str(BASE, "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a")
            .add_network_str(AVALANCHE, "0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F")
            .add_network_str(BNB, "0x78D78E420Da98ad378D7799bE8f4AF69033EB077")
            .add_network_str(OPTIMISM, "0x61fFE014bA17989E743c5F6cB21bF9697530B21e")
            .add_network_str(POLYGON, "0x61fFE014bA17989E743c5F6cB21bF9697530B21e")
        // Not listed on Gnosis and Sepolia chains
    });
    generate_contract_with_config("WETH9", |builder| {
        // Note: the WETH address must be consistent with the one used by the ETH-flow
        // contract
        builder
            .add_network_str(MAINNET, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .add_network_str(GOERLI, "0xB4FBF271143F4FBf7B91A5ded31805e42b2208d6")
            .add_network_str(GNOSIS, "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d")
            .add_network_str(SEPOLIA, "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14")
            .add_network_str(ARBITRUM_ONE, "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1")
            .add_network_str(BASE, "0x4200000000000000000000000000000000000006")
            .add_network_str(AVALANCHE, "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7")
            .add_network_str(BNB, "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c")
            .add_network_str(OPTIMISM, "0x4200000000000000000000000000000000000006")
            .add_network_str(POLYGON, "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270")
    });
    generate_contract_with_config("IUniswapV3Factory", |builder| {
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        builder
            .add_network_str(MAINNET, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(GOERLI, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(SEPOLIA, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(ARBITRUM_ONE, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(BASE, "0x33128a8fC17869897dcE68Ed026d694621f6FDfD")
            .add_network_str(AVALANCHE, "0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD")
            .add_network_str(BNB, "0xdB1d10011AD0Ff90774D0C6Bb92e5C5c8b4461F7")
            .add_network_str(OPTIMISM, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
            .add_network_str(POLYGON, "0x1F98431c8aD98523631AE4a59f267346ea31F984")
        // Not available on Gnosis Chain
    });
    generate_contract_with_config("IZeroEx", |builder| {
        // <https://docs.0xprotocol.org/en/latest/basics/addresses.html?highlight=contracts#addresses>
        // <https://github.com/0xProject/protocol/blob/652d4226229c97895ae9350bbf276370ebb38c5e/packages/contract-addresses/addresses.json>
        builder
            .add_network_str(MAINNET, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(SEPOLIA, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(ARBITRUM_ONE, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(BASE, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(AVALANCHE, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(BNB, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
            .add_network_str(OPTIMISM, "0xdef1abe32c034e558cdd535791643c58a13acc10")
            .add_network_str(POLYGON, "0xdef1c0ded9bec7f1a1670819833240f027b25eff")
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
            .add_network_str(ARBITRUM_ONE, "0xcb8b5CD20BdCaea9a010aC1F8d835824F5C87A04")
            .add_network_str(BASE, "0xc694a91e6b071bF030A18BD3053A7fE09B6DaE69")
    });

    // Unofficial Uniswap v2 liquidity on the Sepolia testnet.
    generate_contract_with_config("TestnetUniswapV2Router02", |builder| {
        // <https://github.com/eth-clients/sepolia/issues/47#issuecomment-1681562464>
        builder.add_network_str(SEPOLIA, "0x86dcd3293C53Cf8EFd7303B57beb2a3F671dDE98")
    });

    // Chainalysis oracle for sanctions screening
    generate_contract_with_config("ChainalysisOracle", |builder| {
        builder
            .add_network_str(MAINNET, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
            .add_network_str(ARBITRUM_ONE, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
            .add_network_str(BASE, "0x3A91A31cB3dC49b4db9Ce721F50a9D076c8D739B")
            .add_network_str(AVALANCHE, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
            .add_network_str(BNB, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
            .add_network_str(OPTIMISM, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
            .add_network_str(POLYGON, "0x40C57923924B5c5c5455c48D93317139ADDaC8fb")
    });

    generate_contract("CowAmm");
    generate_contract_with_config("CowAmmConstantProductFactory", |builder| {
        builder
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x40664207e3375FB4b733d4743CE9b159331fd034"),
                    // <https://etherscan.io/tx/0xf37fc438ddacb00c28305bd7dea3b79091cd5be3405a2b445717d9faf946fa50>
                    deployment_information: Some(DeploymentInformation::BlockNumber(19861952)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xdb1cba3a87f2db53b6e1e6af48e28ed877592ec0"),
                    // <https://gnosisscan.io/tx/0x4121efab4ad58ae7ad73b50448cccae0de92905e181648e5e08de3d6d9c66083>
                    deployment_information: Some(DeploymentInformation::BlockNumber(33874317)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    address: addr("0xb808e8183e3a72d196457d127c7fd4befa0d7fd3"),
                    // <https://sepolia.etherscan.io/tx/0x5e6af00c670eb421b96e78fd2e3b9df573b19e6e0ea77d8003e47cdde384b048>
                    deployment_information: Some(DeploymentInformation::BlockNumber(5874562)),
                },
            )
    });
    generate_contract_with_config("CowAmmLegacyHelper", |builder| {
        builder
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x3705ceee5eaa561e3157cf92641ce28c45a3999c"),
                    // <https://etherscan.io/tx/0x07f0ce50fb9cd30e69799a63ae9100869a3c653d62ea3ba49d2e5e1282f42b63>
                    deployment_information: Some(DeploymentInformation::BlockNumber(20332745)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0xd9ec06b001957498ab1bc716145515d1d0e30ffb"),
                    // <https://gnosisscan.io/tx/0x09e56c7173ab1e1c5d02bc2832799422ebca6d9a40e5bae77f6ca908696bfebf>
                    deployment_information: Some(DeploymentInformation::BlockNumber(35026999)),
                },
            )
    });
    generate_contract("CowAmmUniswapV2PriceOracle");

    // Support contracts used for trade and token simulations.
    generate_contract("Solver");
    generate_contract("Spardose");
    generate_contract("Trader");

    // Support contracts used for various order simulations.
    generate_contract("Balances");
    generate_contract("Signatures");
    generate_contract("SimulateCode");

    // Support contract used for solver fee simulations.
    generate_contract("AnyoneAuthenticator");
    generate_contract("Swapper");

    // Contract for batching multiple `eth_call`s into a single one.
    generate_contract("Multicall");

    // Test Contract for incrementing arbitrary counters.
    generate_contract("Counter");

    // Test Contract for using up a specified amount of gas.
    generate_contract("GasHog");

    // Contract for Uniswap's Permit2 contract.
    generate_contract_with_config("Permit2", |builder| {
        builder
            .add_network(
                MAINNET,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://etherscan.io/tx/0xf2f1fe96c16ee674bb7fcee166be52465a418927d124f5f1d231b36eae65d377>
                    deployment_information: Some(DeploymentInformation::BlockNumber(15986406)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://gnosisscan.io/tx/0x3ba511410edc92cafe94bd100e25adb37981499d17947a3d64c8523fbfd31864>
                    deployment_information: Some(DeploymentInformation::BlockNumber(27338672)),
                },
            )
            .add_network(
                SEPOLIA,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://sepolia.etherscan.io/tx/0x363df5deeead44d8fd38425f3986e3e81946a6c59d8b68fe33926cc700713173>
                    deployment_information: Some(DeploymentInformation::BlockNumber(2356287)),
                },
            )
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://arbiscan.io/tx/0xe244dafca8211ed6fb123efaa5075b7d5813749718412ca435c872afd0e2ea82>
                    deployment_information: Some(DeploymentInformation::BlockNumber(38692735)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://basescan.org/tx/0x26fbdea9a47ba8e21676bc6b6a72a19dded1a0c270e96d5236886ca9c5000d3f>
                    deployment_information: Some(DeploymentInformation::BlockNumber(1425180)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://snowscan.xyz/tx/0x38fd76c2165d920c7e006defd67eeeb0069bf93e41741eec3bbb83d196610a56>
                    deployment_information: Some(DeploymentInformation::BlockNumber(28844415)),
                },
            )
            .add_network(
                BNB,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://bscscan.com/tx/0xb038ec7b72db4207e0c0d5433e1cabc41b4e4f9b9cac577173b3188fc508a6c3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(25343783)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://optimistic.etherscan.io/tx/0xf0a51e0d0579ef8cc7965f5797bd7665ddac14d4d2141423676b8862f7668352>
                    deployment_information: Some(DeploymentInformation::BlockNumber(38854427)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0x000000000022D473030F116dDEE9F6B43aC78BA3"),
                    // <https://polygonscan.com/tx/0xe2a4d996de0d6a23108f701b37acba6c47ee34448bb51fec5c23f542a6f3ccc8>
                    deployment_information: Some(DeploymentInformation::BlockNumber(35701901)),
                },
            )
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
        .write_to_file(Path::new(&dest).join(format!("{name}.rs")))
        .unwrap();
}

fn addr(s: &str) -> Address {
    s.parse().unwrap()
}
