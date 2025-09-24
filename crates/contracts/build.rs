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
const LENS: &str = "232";

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
            .add_network(
                LENS,
                Network {
                    address: addr("0xFb337f8a725A142f65fb9ff4902d41cc901de222"),
                    // <https://explorer.lens.xyz/tx/0xc59b5ffadb40158f9390b1d77f19346dbe9214b27f26346dfa2990ad379a1a32>
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
        // Not available on Lens
    });
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
        // Not available on Lens
    });
    generate_contract_with_config("BalancerQueries", |builder| {
        builder
            .contract_mod_override("balancer_queries")
            .add_network(
                MAINNET,
                Network {
                    address: addr("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
                    // <https://etherscan.io/tx/0x30799534f3a0ab8c7fa492b88b56e9354152ffaddad15415184a3926c0dd9b09>
                    deployment_information: Some(DeploymentInformation::BlockNumber(15188261)),
                },
            )
            .add_network(
                ARBITRUM_ONE,
                Network {
                    address: addr("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
                    // <https://arbiscan.io/tx/0x710d93aab52b6c10197eab20f9d6db1af3931f9890233d8832268291ef2f54b3>
                    deployment_information: Some(DeploymentInformation::BlockNumber(18238624)),
                },
            )
            .add_network(
                OPTIMISM,
                Network {
                    address: addr("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
                    // <https://optimistic.etherscan.io/tx/0xf3b2aaf3e12c7de0987dc99a26242b227b9bc055342dda2e013dab0657d6f9f1>
                    deployment_information: Some(DeploymentInformation::BlockNumber(15288107)),
                },
            )
            .add_network(
                BASE,
                Network {
                    address: addr("0x300Ab2038EAc391f26D9F895dc61F8F66a548833"),
                    // <https://basescan.org/tx/0x425d04ee79511c17d06cd96fe1df9e0727f7e7d46b31f36ecaa044ada6a0d29a>
                    deployment_information: Some(DeploymentInformation::BlockNumber(1205869)),
                },
            )
            .add_network(
                GNOSIS,
                Network {
                    address: addr("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"),
                    // <https://gnosisscan.io/tx/0x5beb3051d393aac24cb236dc850c644f345af65c4927030bd1033403e2f2e503>
                    deployment_information: Some(DeploymentInformation::BlockNumber(24821845)),
                },
            )
            .add_network(
                POLYGON,
                Network {
                    address: addr("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
                    // <https://polygonscan.com/tx/0x0b74f5c230f9b7df8c7a7f0d1ebd5e6c3fab51a67a9bcc8f05c350180041682e>
                    deployment_information: Some(DeploymentInformation::BlockNumber(30988035)),
                },
            )
            .add_network(
                AVALANCHE,
                Network {
                    address: addr("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"),
                    // <https://snowtrace.io/tx/0xf484e1efde47209bad5f72642bcb8d8e2a4092a5036434724ffa2d039e93a1bf?chainid=43114>
                    deployment_information: Some(DeploymentInformation::BlockNumber(26387068)),
                },
            )
    });
    generate_contract("ERC20");
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
            .add_network(
                LENS,
                Network {
                    address: addr("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
                    // <https://explorer.lens.xyz/tx/0x0730c21885153dcc9a25ab7abdc38309ec7c7a8db15b763fbbaf574d1e7ec498>
                    deployment_information: Some(DeploymentInformation::BlockNumber(2612937)),
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
            .add_network(
                LENS,
                Network {
                    address: addr("0x9008D19f58AAbD9eD0D60971565AA8510560ab41"),
                    // <https://explorer.lens.xyz/tx/0x01584b767dda7b115394b93dbcfecadfe589862ae3f7957846a2db82f2f5c703>
                    deployment_information: Some(DeploymentInformation::BlockNumber(2621745)),
                },
            )
    });
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
            .add_network_str(LENS, "0x60Bf78233f48eC42eE3F101b9a05eC7878728006")
    });
    generate_contract("IAavePool");
    generate_contract("IFlashLoanSolverWrapper");
    // EIP-1271 contract - SignatureValidator
    generate_contract("ERC1271SignatureValidator");
    generate_contract_with_config("UniswapV3SwapRouterV2", |builder| {
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        builder
            .add_network_str(MAINNET, "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
            .add_network_str(ARBITRUM_ONE, "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
            .add_network_str(POLYGON, "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
            .add_network_str(OPTIMISM, "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
            .add_network_str(BASE, "0x2626664c2603336E57B271c5C0b26F421741e481")
            .add_network_str(AVALANCHE, "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE")
            .add_network_str(BNB, "0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2")
            .add_network_str(LENS, "0x6ddD32cd941041D8b61df213B9f515A7D288Dc13")
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
            .add_network_str(LENS, "0x1eEA2B790Dc527c5a4cd3d4f3ae8A2DDB65B2af1")
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
            .add_network_str(LENS, "0x6bDc36E20D267Ff0dd6097799f82e78907105e2F")
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
            // not official
            .add_network_str(LENS, "0xc3A5b857Ba82a2586A45a8B59ECc3AA50Bc3D0e3")
        // Not available on Gnosis Chain
    });
    generate_contract_with_config("CowProtocolToken", |builder| {
        builder
            .add_network_str(MAINNET, "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB")
            .add_network_str(GOERLI, "0x91056D4A53E1faa1A84306D4deAEc71085394bC8")
            .add_network_str(GNOSIS, "0x177127622c4A00F3d409B75571e12cB3c8973d3c")
            .add_network_str(SEPOLIA, "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59")
            .add_network_str(ARBITRUM_ONE, "0xcb8b5CD20BdCaea9a010aC1F8d835824F5C87A04")
            .add_network_str(BASE, "0xc694a91e6b071bF030A18BD3053A7fE09B6DaE69")
        // Not available on Lens
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
    generate_contract_with_config("Balances", |builder| {
        builder
            .add_network_str(MAINNET, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(ARBITRUM_ONE, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(BASE, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(AVALANCHE, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(BNB, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(OPTIMISM, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(POLYGON, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(LENS, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(GNOSIS, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
            .add_network_str(SEPOLIA, "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b")
    });
    generate_contract_with_config("Signatures", |builder| {
        builder
            .add_network_str(MAINNET, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(ARBITRUM_ONE, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(BASE, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(AVALANCHE, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(BNB, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(OPTIMISM, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(POLYGON, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(LENS, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(GNOSIS, "0x8262d639c38470F38d2eff15926F7071c28057Af")
            .add_network_str(SEPOLIA, "0x8262d639c38470F38d2eff15926F7071c28057Af")
    });

    // Support contract used for solver fee simulations.
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
        // Not available on Lens
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
