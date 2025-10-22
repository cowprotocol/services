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

    generate_contract("ERC20");
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
