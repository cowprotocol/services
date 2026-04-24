mod codegen;
mod networks;
mod vendor;

use {
    codegen::{Contract, Module, Submodule},
    networks::*,
    std::path::Path,
};

/// Declare a network tuple with an optional block number.
///
/// Example, without blocks:
/// ```no_run
/// networks! {
///     MAINNET => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
///     SEPOLIA => "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
/// };
/// ```
///
/// Example, with blocks:
/// ```no_run
/// networks! {
///     MAINNET => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 12593265),
///     SEPOLIA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 4717488),
/// };
/// ```
macro_rules! networks {
    [$(
        $id:expr => $value:tt
    ),* $(,)?] => {
        [$(
            networks!(@entry $id => $value)
        ),*]
    };

    (@entry $id:expr => ($addr:expr, $block:expr)) => {
        ($id, ($addr, Some($block)))
    };

    (@entry $id:expr => $value:expr) => {
        ($id, ($value, None))
    };
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("generate");

    let result = match command {
        "vendor" => run_vendor(),
        "generate" => run_generate(),
        "all" => run_vendor().and_then(|()| run_generate()),
        other => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: contracts-generate [vendor|generate|all]");
            std::process::exit(1);
        }
    };

    if let Err(err) = result {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn run_vendor() -> anyhow::Result<()> {
    let artifacts_dir = workspace_root().join("artifacts");
    vendor::run(&artifacts_dir)
}

fn run_generate() -> anyhow::Result<()> {
    let workspace_root = workspace_root();

    let artifacts_dir = workspace_root.join("artifacts");
    let output_dir = workspace_root.join("generated");

    eprintln!("Generating workspace under {}...", output_dir.display());
    eprintln!("  artifacts: {}", artifacts_dir.display());

    build_module().generate(&artifacts_dir, &output_dir)?;

    eprintln!("\nDone!");
    Ok(())
}

#[rustfmt::skip]
fn build_module() -> Module {
    Module::default()
        // 0x
        .add_contract(Contract::new("IZeroex").with_networks(networks![
            MAINNET => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            SEPOLIA => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            ARBITRUM_ONE => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            BASE => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            AVALANCHE => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            BNB => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            OPTIMISM => "0xdef1abe32c034e558cdd535791643c58a13acc10",
            POLYGON => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
        ]))
        // Misc
        .add_contract(Contract::new("ERC20"))
        .add_contract(Contract::new("ERC20Mintable"))
        .add_contract(Contract::new("IERC4626"))
        // GnosisSafe
        .add_contract(Contract::new("GnosisSafe"))
        .add_contract(Contract::new("GnosisSafeCompatibilityFallbackHandler"))
        .add_contract(Contract::new("GnosisSafeProxy"))
        .add_contract(Contract::new("GnosisSafeProxyFactory"))
        // Balancer V2
        .add_contract(Contract::new("BalancerV2Authorizer"))
        .add_contract(Contract::new("BalancerV2BasePool"))
        .add_contract(Contract::new("BalancerV2BasePoolFactory"))
        .add_contract(Contract::new("BalancerV2WeightedPool"))
        .add_contract(Contract::new("BalancerV2StablePool"))
        .add_contract(Contract::new("BalancerV2ComposableStablePool"))
        .add_contract(Contract::new("BalancerV2LiquidityBootstrappingPool"))
        .add_contract(
            Contract::new("BalancerV2WeightedPoolFactory").with_networks(networks![
                MAINNET => ("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9", 12272147),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2WeightedPoolFactoryV3").with_networks(networks![
                MAINNET => ("0x5Dd94Da3644DDD055fcf6B3E1aa310Bb7801EB8b", 16520627),
                GNOSIS => ("0xC128a9954e6c874eA3d62ce62B468bA073093F25", 26226256),
                AVALANCHE => ("0x94f68b54191F62f781Fe8298A8A5Fa3ed772d227", 26389236),
                OPTIMISM => ("0xA0DAbEBAAd1b243BBb243f933013d560819eB66f", 72832703),
                POLYGON => ("0x82e4cFaef85b1B6299935340c964C942280327f4", 39036828),
                BNB => ("0x6e4cF292C5349c79cCd66349c3Ed56357dD11B46", 25474982),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2WeightedPoolFactoryV4").with_networks(networks![
                MAINNET => ("0x897888115Ada5773E02aA29F775430BFB5F34c51", 16878323),
                GNOSIS => ("0x6CaD2ea22BFA7F4C14Aae92E47F510Cd5C509bc7", 27055829),
                SEPOLIA => ("0x7920BFa1b2041911b354747CA7A6cDD2dfC50Cfd", 3424893),
                ARBITRUM_ONE => ("0xc7E5ED1054A24Ef31D827E6F86caA58B3Bc168d7", 72222060),
                BASE => ("0x4C32a8a8fDa4E24139B51b456B42290f51d6A1c4", 1204869),
                AVALANCHE => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 27739006),
                OPTIMISM => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 82737545),
                POLYGON => ("0xFc8a407Bba312ac761D8BFe04CE1201904842B76", 40611103),
                BNB => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 26665331),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2WeightedPool2TokensFactory").with_networks(networks![
                MAINNET => ("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0", 12349891),
                ARBITRUM_ONE => ("0xCF0a32Bbef8F064969F21f7e02328FB577382018", 222864),
                OPTIMISM => ("0xdAE7e32ADc5d490a43cCba1f0c736033F2b4eFca", 7005512),
                POLYGON => ("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9", 15832998),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2StablePoolFactoryV2").with_networks(networks![
                MAINNET => ("0x8df6efec5547e31b0eb7d1291b511ff8a2bf987c", 14934936),
                GNOSIS => ("0xf23b4DB826DbA14c0e857029dfF076b1c0264843", 25415344),
                ARBITRUM_ONE => ("0xEF44D6786b2b4d544b7850Fe67CE6381626Bf2D6", 14244664),
                OPTIMISM => ("0xeb151668006CD04DAdD098AFd0a82e78F77076c3", 11088891),
                POLYGON => ("0xcA96C4f198d343E251b1a01F3EBA061ef3DA73C1", 29371951),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2LiquidityBootstrappingPoolFactory").with_networks(networks![
                MAINNET => ("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE", 12871780),
                ARBITRUM_ONE => ("0x142B9666a0a3A30477b052962ddA81547E7029ab", 222870),
                POLYGON => ("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE", 17116402),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory")
                .with_networks(networks![
                    MAINNET => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 13730248),
                    GNOSIS => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 25415236),
                    SEPOLIA => ("0x45fFd460cC6642B8D8Fb12373DFd77Ceb0f4932B", 3419649),
                    ARBITRUM_ONE => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 4859669),
                    BASE => ("0x0c6052254551EAe3ECac77B01DFcf1025418828f", 1206531),
                    AVALANCHE => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 26386552),
                    BNB => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 22691243),
                    OPTIMISM => ("0xf302f9F50958c5593770FDf4d4812309fF77414f", 7005915),
                    POLYGON => ("0x41B953164995c11C81DA73D212ED8Af25741b7Ac", 22067480),
                ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactory").with_networks(networks![
                MAINNET => ("0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F", 15485885),
                ARBITRUM_ONE => ("0xaEb406b0E430BF5Ea2Dc0B9Fe62E4E53f74B3a33", 23227044),
                BNB => ("0xf302f9F50958c5593770FDf4d4812309fF77414f", 22691193),
                OPTIMISM => ("0xf145caFB67081895EE80eB7c04A30Cf87f07b745", 22182522),
                POLYGON => ("0x136FD06Fa01eCF624C7F2B3CB15742c1339dC2c4", 32774224),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV3").with_networks(networks![
                MAINNET => ("0xdba127fBc23fb20F5929C546af220A991b5C6e01", 16580899),
                GNOSIS => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 26365805),
                ARBITRUM_ONE => ("0x1c99324EDC771c82A0DCCB780CC7DDA0045E50e7", 58948370),
                BNB => ("0xacAaC3e6D6Df918Bf3c809DFC7d42de0e4a72d4C", 25475700),
                OPTIMISM => ("0xe2E901AB09f37884BA31622dF3Ca7FC19AA443Be", 72832821),
                POLYGON => ("0x7bc6C0E73EDAa66eF3F6E2f27b0EE8661834c6C9", 39037615),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV4").with_networks(networks![
                MAINNET => ("0xfADa0f4547AB2de89D1304A668C39B3E09Aa7c76", 16878679),
                GNOSIS => ("0xD87F44Df0159DC78029AB9CA7D7e57E7249F5ACD", 27056416),
                SEPOLIA => ("0xA3fd20E29358c056B727657E83DFd139abBC9924", 3425277),
                ARBITRUM_ONE => ("0x2498A2B0d6462d2260EAC50aE1C3e03F4829BA95", 72235860),
                AVALANCHE => ("0x3B1eb8EB7b43882b385aB30533D9A2BeF9052a98", 29221425),
                BNB => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 26666380),
                OPTIMISM => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 82748180),
                POLYGON => ("0x6Ab5549bBd766A43aFb687776ad8466F8b42f777", 40613553),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV5").with_networks(networks![
                MAINNET => ("0xDB8d758BCb971e482B2C45f7F8a7740283A1bd3A", 17672478),
                GNOSIS => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 28900564),
                SEPOLIA => ("0xa523f47A933D5020b23629dDf689695AA94612Dc", 3872211),
                ARBITRUM_ONE => ("0xA8920455934Da4D853faac1f94Fe7bEf72943eF1", 110212282),
                BASE => ("0x8df317a729fcaA260306d7de28888932cb579b88", 1204710),
                AVALANCHE => ("0xE42FFA682A26EF8F25891db4882932711D42e467", 32478827),
                BNB => ("0x4fb47126Fa83A8734991E41B942Ac29A3266C968", 29877945),
                OPTIMISM => ("0x043A2daD730d585C44FB79D2614F295D2d625412", 106752707),
                POLYGON => ("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b", 44961548),
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV6").with_networks(networks![
                MAINNET => ("0x5B42eC6D40f7B7965BE5308c70e2603c0281C1E9", 19314764),
                GNOSIS => ("0x47B489bf5836f83ABD928C316F8e39bC0587B020", 32650879),
                SEPOLIA => ("0x05503B3aDE04aCA81c8D6F88eCB73Ba156982D2B", 5369821),
                ARBITRUM_ONE => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 184805448),
                BASE => ("0x956CCab09898C0AF2aCa5e6C228c3aD4E93d9288", 11099703),
                AVALANCHE => ("0xb9F8AB3ED3F3aCBa64Bc6cd2DcA74B7F38fD7B88", 42186350),
                BNB => ("0x6B5dA774890Db7B7b96C6f44e6a4b0F657399E2e", 36485719),
                OPTIMISM => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 116694338),
                POLYGON => ("0xEAedc32a51c510d35ebC11088fD5fF2b47aACF2E", 53996258),
            ]),
        )
        .add_contract(Contract::new("BalancerV2Vault").with_networks(networks![
            MAINNET => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 12272146),
            GNOSIS => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 24821598),
            SEPOLIA => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 3418831),
            ARBITRUM_ONE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 222832),
            BASE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 1196036),
            AVALANCHE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 26386141),
            BNB => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 22691002),
            OPTIMISM => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 7003431),
            POLYGON => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 15832990),
            INK => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 34313901),
        ]))
        .add_contract(
            Contract::new("BalancerV3BatchRouter").with_networks(networks![
                MAINNET => ("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1", 21339510),
                GNOSIS => ("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b", 37377506),
                SEPOLIA => ("0xC85b652685567C1B074e8c0D4389f83a2E458b1C", 7219301),
                ARBITRUM_ONE => ("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E", 297828544),
                BASE => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 25347205),
                AVALANCHE => ("0xc9b36096f5201ea332Db35d6D195774ea0D5988f", 59965747),
                OPTIMISM => ("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E", 133969588),
                PLASMA => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 782312),
            ]),
        )
        // UniV2
        .add_contract(Contract::new("BaoswapRouter").with_networks(networks![
            GNOSIS => "0x6093AeBAC87d62b1A5a4cEec91204e35020E38bE",
        ]))
        .add_contract(Contract::new("HoneyswapRouter").with_networks(networks![
            GNOSIS => "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77",
        ]))
        .add_contract(Contract::new("PancakeRouter").with_networks(networks![
            MAINNET => "0xEfF92A263d31888d860bD50809A8D171709b7b1c",
            ARBITRUM_ONE => "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb",
            BASE => "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb",
            BNB => "0x10ED43C718714eb63d5aA57B78B54704E256024E",
        ]))
        .add_contract(Contract::new("SushiSwapRouter").with_networks(networks![
            MAINNET => "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f",
            GNOSIS => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            ARBITRUM_ONE => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            BASE => "0x6bded42c6da8fbf0d2ba55b2fa120c5e0c8d7891",
            AVALANCHE => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            BNB => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            OPTIMISM => "0x2abf469074dc0b54d793850807e6eb5faf2625b1",
            POLYGON => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
        ]))
        .add_contract(Contract::new("SwaprRouter").with_networks(networks![
            MAINNET => "0xb9960d9bca016e9748be75dd52f02188b9d0829f",
            GNOSIS => "0xE43e60736b1cb4a75ad25240E2f9a62Bff65c0C0",
            ARBITRUM_ONE => "0x530476d5583724A89c8841eB6Da76E7Af4C0F17E",
        ]))
        .add_contract(Contract::new("ISwaprPair"))
        .add_contract(
            Contract::new("TestnetUniswapV2Router02").with_networks(networks![
                SEPOLIA => "0x86dcd3293C53Cf8EFd7303B57beb2a3F671dDE98",
            ]),
        )
        .add_contract(Contract::new("UniswapV2Factory").with_networks(networks![
            MAINNET => "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
            GNOSIS => "0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7",
            ARBITRUM_ONE => "0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9",
            BASE => "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6",
            SEPOLIA => "0xF62c03E08ada871A0bEb309762E260a7a6a880E6",
            AVALANCHE => "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C",
            BNB => "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6",
            OPTIMISM => "0x0c3c1c532F1e39EdF36BE9Fe0bE1410313E074Bf",
            POLYGON => "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C",
        ]))
        .add_contract(Contract::new("UniswapV2Router02").with_networks(networks![
            MAINNET => "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",
            GNOSIS => "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77",
            ARBITRUM_ONE => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            BASE => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            SEPOLIA => "0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3",
            AVALANCHE => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            BNB => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            OPTIMISM => "0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2",
            POLYGON => "0xedf6066a2b290C185783862C7F4776A2C8077AD1",
        ]))
        .add_contract(Contract::new("IUniswapLikeRouter"))
        .add_contract(Contract::new("IUniswapLikePair"))
        .add_contract(Contract::new("UniswapV3Pool"))
        .add_contract(Contract::new("UniswapV3QuoterV2").with_networks(networks![
            MAINNET => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            GNOSIS => "0x7E9cB3499A6cee3baBe5c8a3D328EA7FD36578f4",
            ARBITRUM_ONE => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            BASE => "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a",
            AVALANCHE => "0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F",
            BNB => "0x78D78E420Da98ad378D7799bE8f4AF69033EB077",
            OPTIMISM => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            POLYGON => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            LINEA => "0x42bE4D6527829FeFA1493e1fb9F3676d2425C3C1",
            PLASMA => "0xaa52bB8110fE38D0d2d2AF0B85C3A3eE622CA455",
            INK => "0x96b572D2d880cf2Fa2563651BD23ADE6f5516652",
        ]))
        .add_contract(
            Contract::new("UniswapV3SwapRouterV2").with_networks(networks![
                ARBITRUM_ONE => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                MAINNET => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                GNOSIS => "0xc6D25285D5C5b62b7ca26D6092751A145D50e9Be",
                POLYGON => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                OPTIMISM => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                BASE => "0x2626664c2603336E57B271c5C0b26F421741e481",
                AVALANCHE => "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE",
                BNB => "0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2",
                LINEA => "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a",
                PLASMA => "0x807F4E281B7A3B324825C64ca53c69F0b418dE40",
                INK => "0x177778F19E89dD1012BdBe603F144088A95C4B53",
            ]),
        )
        .add_contract(Contract::new("IUniswapV3Factory").with_networks(networks![
            MAINNET => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            GNOSIS => "0xe32F7dD7e3f098D518ff19A22d5f028e076489B1",
            SEPOLIA => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            ARBITRUM_ONE => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            BASE => "0x33128a8fC17869897dcE68Ed026d694621f6FDfD",
            AVALANCHE => "0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD",
            BNB => "0xdB1d10011AD0Ff90774D0C6Bb92e5C5c8b4461F7",
            OPTIMISM => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            POLYGON => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            LINEA => "0x31FAfd4889FA1269F7a13A66eE0fB458f27D72A9",
            PLASMA => "0xcb2436774C3e191c85056d248EF4260ce5f27A9D",
            INK => "0x640887A9ba3A9C53Ed27D0F7e8246A4F933f3424",
        ]))
        .add_contract(Contract::new("HooksTrampoline").with_networks(networks![
            MAINNET => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            GNOSIS => "0x01DcB88678aedD0C4cC9552B20F4718550250574",
            SEPOLIA => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            ARBITRUM_ONE => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            BASE => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            AVALANCHE => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            BNB => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            OPTIMISM => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            POLYGON => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            LINEA => "0x60bf78233f48ec42ee3f101b9a05ec7878728006",
            PLASMA => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            INK => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
        ]))
        .add_contract(Contract::new("CoWSwapEthFlow").with_networks(networks![
            MAINNET => ("0x40a50cf069e992aa4536211b23f286ef88752187", 16169866),
            GNOSIS => ("0x40a50cf069e992aa4536211b23f286ef88752187", 25414331),
            SEPOLIA => ("0x0b7795E18767259CC253a2dF471db34c72B49516", 4718739),
            ARBITRUM_ONE => ("0x6DFE75B5ddce1ADE279D4fa6BD6AeF3cBb6f49dB", 204747458),
            BASE => ("0x3C3eA1829891BC9bEC3d06A81d5d169e52a415e3", 21490258),
            AVALANCHE => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 60496408),
            BNB => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 48411237),
            OPTIMISM => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 134607215),
            POLYGON => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 71296258),
            LINEA => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 24522097),
            PLASMA => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 3521855),
        ]))
        .add_contract(Contract::new("CoWSwapOnchainOrders"))
        .add_contract(Contract::new("ERC1271SignatureValidator"))
        .add_contract(Contract::new("BalancerQueries").with_networks(networks![
            MAINNET => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 15188261),
            ARBITRUM_ONE => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 18238624),
            OPTIMISM => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 15288107),
            BASE => ("0x300Ab2038EAc391f26D9F895dc61F8F66a548833", 1205869),
            GNOSIS => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 24821845),
            POLYGON => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 30988035),
            AVALANCHE => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 26387068),
        ]))
        .add_contract(
            Contract::new("LiquoriceSettlement").with_networks(networks![
                MAINNET => "0x0448633eb8B0A42EfED924C42069E0DcF08fb552",
                ARBITRUM_ONE => "0x0448633eb8B0A42EfED924C42069E0DcF08fb552",
            ]),
        )
        .add_contract(Contract::new("FlashLoanRouter").with_networks(networks![
            MAINNET => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
            GNOSIS => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
            SEPOLIA => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
            ARBITRUM_ONE => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
            BASE => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
            POLYGON => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
            AVALANCHE => "0x9da8b48441583a2b93e2ef8213aad0ec0b392c69",
        ]))
        .add_contract(Contract::new("CowSettlementForwarder"))
        .add_contract(Contract::new("ICowWrapper"))
        .add_contract(Contract::new("ChainalysisOracle").with_networks(networks![
            MAINNET => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            ARBITRUM_ONE => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            BASE => "0x3A91A31cB3dC49b4db9Ce721F50a9D076c8D739B",
            AVALANCHE => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            BNB => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            OPTIMISM => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            POLYGON => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
        ]))
        .add_contract(Contract::new("Permit2").with_networks(networks![
            MAINNET => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 15986406),
            GNOSIS => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 27338672),
            SEPOLIA => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 2356287),
            ARBITRUM_ONE => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 38692735),
            BASE => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 1425180),
            AVALANCHE => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 28844415),
            BNB => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 25343783),
            OPTIMISM => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 38854427),
            POLYGON => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 35701901),
            PLASMA => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 7808),
            INK => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 0),
        ]))
        .add_contract(
            Contract::new("GPv2AllowListAuthentication").with_networks(networks![
                MAINNET => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 12593263),
                GNOSIS => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 16465099),
                SEPOLIA => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 4717469),
                ARBITRUM_ONE => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 204702129),
                BASE => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 21407137),
                AVALANCHE => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 59891351),
                BNB => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 48173639),
                OPTIMISM => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 134254466),
                POLYGON => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 45854728),
                LINEA => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 24333100),
                PLASMA => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 3439709),
                INK => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 34436840),
            ]),
        )
        .add_contract(Contract::new("GPv2Settlement").with_networks(networks![
            MAINNET => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 12593265),
            GNOSIS => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 16465100),
            SEPOLIA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 4717488),
            ARBITRUM_ONE => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 204704802),
            BASE => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 21407238),
            AVALANCHE => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 59891356),
            BNB => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 48173641),
            OPTIMISM => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 134254624),
            POLYGON => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 45859743),
            LINEA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 24333100),
            PLASMA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 3439711),
            INK => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 34436849),
        ]))
        .add_contract(Contract::new("WETH9").with_networks(networks![
            MAINNET => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            GNOSIS => "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d",
            SEPOLIA => "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
            ARBITRUM_ONE => "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
            BASE => "0x4200000000000000000000000000000000000006",
            AVALANCHE => "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7",
            BNB => "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c",
            OPTIMISM => "0x4200000000000000000000000000000000000006",
            POLYGON => "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
            LINEA => "0xe5d7c2a44ffddf6b295a15c148167daaaf5cf34f",
            PLASMA => "0x6100E367285b01F48D07953803A2d8dCA5D19873",
            INK => "0x4200000000000000000000000000000000000006",
        ]))
        .add_submodule(
            Submodule::new("cow_amm")
                .add_contract(Contract::new("CowAmm"))
                .add_contract(Contract::new("CowAmmConstantProductFactory").with_networks(
                    networks![
                        MAINNET => ("0x40664207e3375FB4b733d4743CE9b159331fd034", 19861952),
                        GNOSIS => ("0xdb1cba3a87f2db53b6e1e6af48e28ed877592ec0", 33874317),
                        SEPOLIA => ("0xb808e8183e3a72d196457d127c7fd4befa0d7fd3", 5874562),
                    ],
                ))
                .add_contract(Contract::new("CowAmmLegacyHelper").with_networks(networks![
                    MAINNET => ("0x3705ceee5eaa561e3157cf92641ce28c45a3999c", 20332745),
                    GNOSIS => ("0xd9ec06b001957498ab1bc716145515d1d0e30ffb", 35026999),
                ]))
                .add_contract(Contract::new("CowAmmUniswapV2PriceOracle"))
                .add_contract(Contract::new("CowAmmFactoryGetter")),
        )
        .add_submodule(
            Submodule::new("test")
                .add_contract(Contract::new("GasHog"))
                .add_contract(Contract::new("Counter"))
                .add_contract(Contract::new("MockERC4626Wrapper"))
                .add_contract(Contract::new("CowProtocolToken").with_networks(networks![
                    MAINNET => "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                    GNOSIS => "0x177127622c4A00F3d409B75571e12cB3c8973d3c",
                    SEPOLIA => "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59",
                    ARBITRUM_ONE => "0xcb8b5CD20BdCaea9a010aC1F8d835824F5C87A04",
                    BASE => "0xc694a91e6b071bF030A18BD3053A7fE09B6DaE69",
                ]))
                .add_contract(Contract::new("NonStandardERC20Balances"))
                .add_contract(Contract::new("RemoteERC20Balances")),
        )
        .add_submodule(
            Submodule::new("support")
                .add_contract(Contract::new("AnyoneAuthenticator"))
                .add_contract(Contract::new("Solver"))
                .add_contract(Contract::new("Spardose"))
                .add_contract(Contract::new("Trader"))
                .add_contract(Contract::new("Swapper"))
                .add_contract(Contract::new("Signatures").with_networks(networks![
                    MAINNET => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    ARBITRUM_ONE => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    BASE => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    AVALANCHE => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    BNB => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    OPTIMISM => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    POLYGON => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    GNOSIS => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    SEPOLIA => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    LINEA => "0xf6E57e72F7dB3D9A51a8B4c149C00475b94A37e4",
                    PLASMA => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    INK => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                ]))
                .add_contract(Contract::new("Balances").with_networks(networks![
                    MAINNET => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    ARBITRUM_ONE => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    BASE => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    AVALANCHE => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    BNB => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    OPTIMISM => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    POLYGON => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    GNOSIS => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    SEPOLIA => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    PLASMA => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    LINEA => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                    INK => "0x88b4B74082BffB2976C306CB3f7E9093AE48B94F",
                ])),
        )
}
