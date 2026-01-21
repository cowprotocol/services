use {
    alloy_sol_macro_expander::expand::expand,
    alloy_sol_macro_input::{SolInput, SolInputKind},
    anyhow::{Context, Result},
    networks::*,
    proc_macro2::{Span, TokenStream},
    quote::{ToTokens, format_ident},
    std::{
        collections::HashMap,
        fmt::Write,
        path::{Path, PathBuf},
    },
};

mod networks {
    pub const MAINNET: u64 = 1;
    pub const GNOSIS: u64 = 100;
    pub const SEPOLIA: u64 = 11155111;
    pub const ARBITRUM_ONE: u64 = 42161;
    pub const BASE: u64 = 8453;
    pub const POLYGON: u64 = 137;
    pub const AVALANCHE: u64 = 43114;
    pub const BNB: u64 = 56;
    pub const OPTIMISM: u64 = 10;
    pub const LENS: u64 = 232;
    pub const LINEA: u64 = 59144;
    pub const PLASMA: u64 = 9745;
    pub const INK: u64 = 57073;
}

/// Declare a network tuple with an optional block number.
///
/// Example, without blocks:
/// ```no_run
/// # #[macro_use] extern crate contracts_generate;
/// # use contracts_generate::networks::{MAINNET, SEPOLIA};
/// # fn main() {
/// # let _: [(_, (_, Option<u64>)); _] =
/// networks! {
///     MAINNET => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
///     SEPOLIA => "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
/// };
/// # }
/// ```
///
/// Example, with blocks:
/// ```no_run
/// # #[macro_use] extern crate contracts_generate;
/// # use contracts_generate::networks::{MAINNET, SEPOLIA};
/// # fn main() {
/// # let _: [(_, (_, Option<u64>)); _] =
/// networks! {
///     MAINNET => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 12593265),
///     SEPOLIA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 4717488),
/// };
/// # }
/// ```
#[macro_export]
macro_rules! networks {
    // Entry point: accepts a list of entries and delegates to internal rules
    [$(
        $id:expr => $value:tt
    ),* $(,)?] => {
        [$(
            networks!(@entry $id => $value)
        ),*]
    };

    // Internal rule: handle entry with address and block (parenthesized)
    (@entry $id:expr => ($addr:expr, $block:expr)) => {
        ($id, ($addr, Some($block)))
    };

    // Internal rule: handle entry with just address
    (@entry $id:expr => $value:expr) => {
        ($id, ($value, None))
    };
}

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existent files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=artifacts/");

    // Path to the directory containing the vendored contract artifacts.
    let vendored_bindings = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("alloy");

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
            // Not available on Lens
        ]))
        // Misc
        .add_contract(Contract::new("ERC20"))
        .add_contract(Contract::new("ERC20Mintable"))
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
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
        .add_contract(
            Contract::new("BalancerV2WeightedPoolFactory").with_networks(networks![
                // <https://etherscan.io/tx/0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792>
                MAINNET => ("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9", 12272147),
                // Not available on Sepolia (only version ≥ 4)
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // Not available on Lens, Linea, Plasma and Ink
            ]),
        )
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
        .add_contract(
            Contract::new("BalancerV2WeightedPoolFactoryV3").with_networks(networks![
                // <https://etherscan.io/tx/0x39f357b78c03954f0bcee2288bf3b223f454816c141ef20399a7bf38057254c4>
                MAINNET => ("0x5Dd94Da3644DDD055fcf6B3E1aa310Bb7801EB8b", 16520627),
                // <https://gnosisscan.io/tx/0x2ac3d873b6f43de6dd77525c7e5b68a8fc3a1dee40303e1b6a680b0285b26091>
                GNOSIS => ("0xC128a9954e6c874eA3d62ce62B468bA073093F25", 26226256),
                // <https://snowscan.xyz/tx/0xdf2c77743cc9287df2022cd6c5f9209ecfecde07371717ab0427d96042a88640>
                AVALANCHE => ("0x94f68b54191F62f781Fe8298A8A5Fa3ed772d227", 26389236),
                // <https://optimistic.etherscan.io/tx/0xc5e79fb00b9a8e2c89b136aae0be098e58f8e832ede13e8079213a75c9cd9c08>
                OPTIMISM => ("0xA0DAbEBAAd1b243BBb243f933013d560819eB66f", 72832703),
                // <https://polygonscan.com/tx/0x2bc079c0e725f43670898b474afedf38462feee72ef8e874a1efcec0736672fc>
                POLYGON => ("0x82e4cFaef85b1B6299935340c964C942280327f4", 39036828),
                // <https://bscscan.com/tx/0x91107b9581e18ec0a4a575d4713bdd7b1fc08656c35522d216307930aa4de7b6>
                BNB => ("0x6e4cF292C5349c79cCd66349c3Ed56357dD11B46", 25474982),
                // Not available on Sepolia (only version ≥ 4)
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // Not available on Lens, Linea, Plasma and Ink
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2WeightedPoolFactoryV4").with_networks(networks![
                // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
                MAINNET => ("0x897888115Ada5773E02aA29F775430BFB5F34c51", 16878323),
                // <https://gnosisscan.io/tx/0xcb6768bd92add227d46668357291e1d67c864769d353f9f0041c59ad2a3b21bf>
                GNOSIS => ("0x6CaD2ea22BFA7F4C14Aae92E47F510Cd5C509bc7", 27055829),
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#pool-factories>
                // <https://sepolia.etherscan.io/tx/0x7dd392b586f1cdecfc635e7dd40ee1444a7836772811e59321fd4873ecfdf3eb>
                SEPOLIA => ("0x7920BFa1b2041911b354747CA7A6cDD2dfC50Cfd", 3424893),
                // <https://arbiscan.io/tx/0x167fe7eb776d1be36b21402d8ae120088c393e28ae7ca0bd1defac84e0f2848b>
                ARBITRUM_ONE => ("0xc7E5ED1054A24Ef31D827E6F86caA58B3Bc168d7", 72222060),
                // <https://basescan.org/tx/0x0732d3a45a3233a134d6e0e72a00ca7a971d82cdc51f71477892ac517bf0d4c9>
                BASE => ("0x4C32a8a8fDa4E24139B51b456B42290f51d6A1c4", 1204869),
                // <https://snowscan.xyz/tx/0xa3fc8aab3b9baba3905045a53e52a47daafe79d4aa26d4fef5c51f3840aa55fa>
                AVALANCHE => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 27739006),
                // <https://optimistic.etherscan.io/tx/0xad915050179db368e43703f3ee1ec55ff5e5e5e0268c15f8839c9f360caf7b0b>
                OPTIMISM => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 82737545),
                // <https://polygonscan.com/tx/0x65e6b13231c2c5656357005a9e419ad6697178ae74eda1ea7522ecdafcf77136>
                POLYGON => ("0xFc8a407Bba312ac761D8BFe04CE1201904842B76", 40611103),
                // <https://bscscan.com/tx/0xc7fada60761e3240332c4cbd169633f1828b2a15de23f0148db9d121afebbb4b>
                BNB => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 26665331),
                // Not available on Base, Lens, Linea, Plasma and Ink
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
            ]),
        )
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
        .add_contract(
            Contract::new("BalancerV2WeightedPool2TokensFactory").with_networks(networks![
                // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
                MAINNET => ("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0", 12349891),
                ARBITRUM_ONE => ("0xCF0a32Bbef8F064969F21f7e02328FB577382018", 222864),
                // <https://optimistic.etherscan.io/tx/0xd5754950d47179d822ea976a8b2af82ffa80e992cf0660b02c0c218359cc8987>
                OPTIMISM => ("0xdAE7e32ADc5d490a43cCba1f0c736033F2b4eFca", 7005512),
                // <https://polygonscan.com/tx/0xb8ac851249cc95bc0943ef0732d28bbd53b0b36c7dd808372666acd8c5f26e1c>
                POLYGON => ("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9", 15832998),
                // Not available on Sepolia, Base, Avalanche, BNB, Lens, Linea, Plasma and Ink
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
            ]),
        )
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
        .add_contract(
            Contract::new("BalancerV2StablePoolFactoryV2").with_networks(networks![
                // <https://etherscan.io/tx/0xef36451947ebd97b72278face57a53806e90071f4c902259db2db41d0c9a143d>
                MAINNET => ("0x8df6efec5547e31b0eb7d1291b511ff8a2bf987c", 14934936),
                // <https://gnosisscan.io/tx/0xe062237f0c8583375b10cf514d091781bfcd52d9ababbd324180770a5efbc6b1>
                GNOSIS => ("0xf23b4DB826DbA14c0e857029dfF076b1c0264843", 25415344),
                ARBITRUM_ONE => ("0xEF44D6786b2b4d544b7850Fe67CE6381626Bf2D6", 14244664),
                // <https://optimistic.etherscan.io/tx/0xcf9f0bd731ded0e513708200df28ac11d17246fb53fc852cddedf590e41c9c03>
                OPTIMISM => ("0xeb151668006CD04DAdD098AFd0a82e78F77076c3", 11088891),
                // <https://polygonscan.com/tx/0xa2c41d014791888a29a9491204446c1b9b2f5dee3f3eb31ad03f290259067b44>
                POLYGON => ("0xcA96C4f198d343E251b1a01F3EBA061ef3DA73C1", 29371951),
                // Not available on Sepolia, Base, Avalanche, BNB, Lens, Linea, Plasma and Ink
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
            ]),
        )
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
        .add_contract(
            Contract::new("BalancerV2LiquidityBootstrappingPoolFactory").with_networks(networks![
                // <https://etherscan.io/tx/0x665ac1c7c5290d70154d9dfc1d91dc2562b143aaa9e8a77aa13e7053e4fe9b7c>
                MAINNET => ("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE", 12871780),
                ARBITRUM_ONE => ("0x142B9666a0a3A30477b052962ddA81547E7029ab", 222870),
                // <https://polygonscan.com/tx/0xd9b5b9a9e6ea17a87f85574e93577e3646c9c2f9c8f38644f936949e6c853288>
                POLYGON => ("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE", 17116402),
                // Not available on Sepolia, Base, Avalanche, BNB, Optimism, Lens, Linea, Plasma and Ink
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
            ]),
        )
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
        .add_contract(
            Contract::new("BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory")
                .with_networks(networks![
                    // <https://etherscan.io/tx/0x298381e567ff6643d9b32e8e7e9ff0f04a80929dce3e004f6fa1a0104b2b69c3>
                    MAINNET => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 13730248),
                    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/gnosis.html#ungrouped-active-current-contracts>
                    // <https://gnosis.blockscout.com/tx/0xbd56fefdb27e4ff1c0852e405f78311d6bc2befabaf6c87a405ab19de8c1506a>
                    GNOSIS => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 25415236),
                    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
                    // <https://sepolia.etherscan.io/tx/0xe0e8feb509a8aa8a1eaa0b0c4b34395ff2fd880fb854fbeeccc0af1826e395c9>
                    SEPOLIA => ("0x45fFd460cC6642B8D8Fb12373DFd77Ceb0f4932B", 25415236),
                    ARBITRUM_ONE => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 4859669),
                    // <https://basescan.org/tx/0x0529de9dbe772f4b4f48da93ae2c2d2c46e3d3221ced9e0c4063a7a5bc47e874>
                    BASE => ("0x0c6052254551EAe3ECac77B01DFcf1025418828f", 1206531),
                    // <https://snowscan.xyz/tx/0x33a75d83436ae9fcda4b4986713417bf3dc80d9ceb8d2541817846b1ac579d9f>
                    AVALANCHE => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 26386552),
                    // <https://bscscan.com/tx/0x8b964b97e6091bd41c93002c558d49adc26b8b31d2b30f3a33babbbbe8c55f47>
                    BNB => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 22691243),
                    // <https://optimistic.etherscan.io/tx/0x14fb43f051eebdec645abf0125e52348dc875b0887b689f8db026d75f9c78dda>
                    OPTIMISM => ("0xf302f9F50958c5593770FDf4d4812309fF77414f", 7005915),
                    // <https://polygonscan.com/tx/0x125bc007a86d771f8dc8f5fa1017de6e5a11162a458a72f25814503404bbeb0b>
                    POLYGON => ("0x41B953164995c11C81DA73D212ED8Af25741b7Ac", 22067480),
                    // Not available on Lens, Linea, Plasma and Ink
                ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactory").with_networks(networks![
                // <https://etherscan.io/tx/0x3b9e93ae050e59b3ca3657958ca30d1fd13fbc43208f8f0aa01ae992294f9961>
                MAINNET => ("0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F", 15485885),
                ARBITRUM_ONE => ("0xaEb406b0E430BF5Ea2Dc0B9Fe62E4E53f74B3a33", 23227044),
                // <https://bscscan.com/tx/0x6c6e1c72c91c75714f698049f1c7b66d8f2baced54e0dd2522dfadff27b5ccf1>
                BNB => ("0xf302f9F50958c5593770FDf4d4812309fF77414f", 22691193),
                // <https://optimistic.etherscan.io/tx/0xad2f330ad865dc7955376a3d9733486b38c53ba0d4757ad4e1b63b105401c506>
                OPTIMISM => ("0xf145caFB67081895EE80eB7c04A30Cf87f07b745", 22182522),
                // <https://polygonscan.com/tx/0xe5d908be686056f1519663a407167c088924f60d29c799ec74438b9de891989e>
                POLYGON => ("0x136FD06Fa01eCF624C7F2B3CB15742c1339dC2c4", 32774224),
                // Not available on Sepolia, Gnosis Chain, Base, Avalanche, Lens, Linea, Plasma and Ink
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV3").with_networks(networks![
                // <https://etherscan.io/tx/0xd8c9ba758cb318beb0c9525b7621280a22b6dfe02cf725a3ece0718598f260ef>
                MAINNET => ("0xdba127fBc23fb20F5929C546af220A991b5C6e01", 16580899),
                // <https://gnosisscan.io/tx/0x2abd7c865f8ab432b340f7de897192c677ffa254908fdec14091e0cd06962963>
                GNOSIS => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 26365805),
                ARBITRUM_ONE => ("0x1c99324EDC771c82A0DCCB780CC7DDA0045E50e7", 58948370),
                // <https://bscscan.com/tx/0xfe0c47c2b124a059d11704c1bd1815dcc554834ae0c2d11c433946226015619f>
                BNB => ("0xacAaC3e6D6Df918Bf3c809DFC7d42de0e4a72d4C", 25475700),
                // <https://optimistic.etherscan.io/tx/0x2bb1c3fbf1370c6e20ecda36b555de1a4426340908055c4274823e31f92210e>
                OPTIMISM => ("0xe2E901AB09f37884BA31622dF3Ca7FC19AA443Be", 72832821),
                // <https://polygonscan.com/tx/0xb189a45eac7ea59c0bb638b5ae6c4c93f9877f31ce826e96b792a9154e7a32a7>
                POLYGON => ("0x7bc6C0E73EDAa66eF3F6E2f27b0EE8661834c6C9", 39037615),
                // Not available on Sepolia (only version ≥ 4) and on Base (only version ≥ 5)
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
                // Not available on Lens, Linea, Plasma and Ink
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV4").with_networks(networks![
                // <https://etherscan.io/tx/0x3b61da162f3414c376cfe8b38d57ca6ba3c40b24446029ddab1187f4ae7c2bd7>
                MAINNET => ("0xfADa0f4547AB2de89D1304A668C39B3E09Aa7c76", 16878679),
                // <https://gnosisscan.io/tx/0x2739416da7e44add08bdfb5e4e5a29ca981383b97162748887efcc5c1241b2f1>
                GNOSIS => ("0xD87F44Df0159DC78029AB9CA7D7e57E7249F5ACD", 27056416),
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#deprecated-contracts>
                // <https://sepolia.etherscan.io/tx/0x9313a59ad9a95f2518076cbf4d0dc5f312e0b013a43a7ea4821cae2aa7a50aa2>
                SEPOLIA => ("0xA3fd20E29358c056B727657E83DFd139abBC9924", 3425277),
                ARBITRUM_ONE => ("0x2498A2B0d6462d2260EAC50aE1C3e03F4829BA95", 72235860),
                // <https://snowscan.xyz/tx/0x7b396102e767ec5f2bc06fb2c9d7fb704d0ddc537c04f28cb538c6de7cc4261e>
                AVALANCHE => ("0x3B1eb8EB7b43882b385aB30533D9A2BeF9052a98", 29221425),
                // <https://bscscan.com/tx/0x2819b490b5e04e27d66476730411df8e572bc33038aa869a370ecfa852de0cbf>
                BNB => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 26666380),
                // <https://optimistic.etherscan.io/tx/0x5d6c515442188eb4af83524618333c0fbdab0df809af01c4e7a9e380f1841199>
                OPTIMISM => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 82748180),
                // <https://polygonscan.com/tx/0x2cea6a0683e67ebdb7d4a1cf1ad303126c5f228f05f8c9e2ccafdb1f5a024376>
                POLYGON => ("0x6Ab5549bBd766A43aFb687776ad8466F8b42f777", 40613553),
                // Not available on Base, Lens, Linea, Plasma and Ink
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV5").with_networks(networks![
                // <https://etherscan.io/tx/0x1fc28221925959c0713d04d9f9159255927ebb94b7fa76e4795db0e365643c07>
                MAINNET => ("0xDB8d758BCb971e482B2C45f7F8a7740283A1bd3A", 17672478),
                // <https://gnosisscan.io/tx/0xcbf18b5a0d1f1fca9b30d08ab77d8554567c3bffa7efdd3add273073d20bb1e2>
                GNOSIS => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 28900564),
                // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
                // <https://sepolia.etherscan.io/tx/0x2c155dde7c480929991dd2a3344d9fdd20252f235370d46d0887b151dc0416bd>
                SEPOLIA => ("0xa523f47A933D5020b23629dDf689695AA94612Dc", 3872211),
                ARBITRUM_ONE => ("0xA8920455934Da4D853faac1f94Fe7bEf72943eF1", 110212282),
                // <https://basescan.org/tx/0x1d291ba796b0397d73581b17695cf0e53e61551e419c43d11d81198b00c2bfd3>
                BASE => ("0x8df317a729fcaA260306d7de28888932cb579b88", 1204710),
                // <https://snowscan.xyz/tx/0x000659feb0831fc511f5c2ad12f3b2d466152b753c805fcb06e848701fd1b4b7>
                AVALANCHE => ("0xE42FFA682A26EF8F25891db4882932711D42e467", 32478827),
                // <https://bscscan.com/tx/0x5bdfed936f82800e80543d5212cb287dceebb52c29133838acbe7e148bf1a447>
                BNB => ("0x4fb47126Fa83A8734991E41B942Ac29A3266C968", 29877945),
                // <https://optimistic.etherscan.io/tx/0xa141b35dbbb18154e2452b1ae6ab7d82a6555724a878b5fccff40e18c8ae3484>
                OPTIMISM => ("0x043A2daD730d585C44FB79D2614F295D2d625412", 106752707),
                // <https://polygonscan.com/tx/0xa3d9a1cf00eaca469d6f9ec2fb836bbbfdfbc3b0eeadc07619bb9e695bfdecb8>
                POLYGON => ("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b", 44961548),
                // Not available on Lens, Linea, Plasma and Ink
            ]),
        )
        .add_contract(
            Contract::new("BalancerV2ComposableStablePoolFactoryV6").with_networks(networks![
                // <https://etherscan.io/tx/0x4149cadfe5d3431205d9819fca44ed7a4c2b101adc51adc75cc4586dee237be8>
                MAINNET => ("0x5B42eC6D40f7B7965BE5308c70e2603c0281C1E9", 19314764),
                // <https://gnosisscan.io/tx/0xc3fc1fb96712a0659b7e9e5f406f63bdf5cbd5df9e04f0372c28f75785036791>
                GNOSIS => ("0x47B489bf5836f83ABD928C316F8e39bC0587B020", 32650879),
                // <https://sepolia.etherscan.io/tx/0x53aa3587002469b758e2bb87135d9599fd06e7be944fe61c7f82045c45328566>
                SEPOLIA => ("0x05503B3aDE04aCA81c8D6F88eCB73Ba156982D2B", 5369821),
                // <https://arbiscan.io/tx/0xfa1e7642e135fb32dc14c990b851e5e7a0ac7a463e3a60c5003ae4142396f45e>
                ARBITRUM_ONE => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 184805448),
                // <https://basescan.org/tx/0x5d3342faf0368b939daa93247536afa26cc72c83de52ba7711ae1b8646688467>
                BASE => ("0x956CCab09898C0AF2aCa5e6C229c3aD4E93d9288", 11099703),
                // <https://snowscan.xyz/tx/0x246248ad396826dbfbdc5360cb9cbbdb3a672efa08cc745d1670900888c58c7b>
                AVALANCHE => ("0xb9F8AB3ED3F3aCBa64Bc6cd2DcA74B7F38fD7B88", 42186350),
                // <https://bscscan.com/tx/0x6784ab50138c7488bc14d4d9beb6a9e1ddc209a45f0a96b4ee98a7db84167dea>
                BNB => ("0x6B5dA774890Db7B7b96C6f44e6a4b0F657399E2e", 36485719),
                // <https://optimistic.etherscan.io/tx/0xa38b696479f35a9751ca8b1f0ddeb160188b3146113975b8c2b657c2fe7d7fd2>
                OPTIMISM => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 116694338),
                // <https://polygonscan.com/tx/0x7b9678ad538b1cd3f3a03e63455e7d49a1bc716ea42310fbf99df4bf93ecfdfa>
                POLYGON => ("0xEAedc32a51c510d35ebC11088fD5fF2b47aACF2E", 53996258),
                // Not available on Lens, Linea, Plasma and Ink
            ]),
        )
        // Balancer addresses can be obtained from:
        // <https://github.com/balancer/balancer-subgraph-v2/blob/master/networks.yaml>
        .add_contract(Contract::new("BalancerV2Vault").with_networks(networks![
            // <https://etherscan.io/tx/0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7>
            MAINNET => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 12272146),
            // <https://gnosisscan.io/tx/0x21947751661e1b9197492f22779af1f5175b71dc7057869e5a8593141d40edf1>
            GNOSIS => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 24821598),
            // <https://sepolia.etherscan.io/tx/0xb22509c6725dd69a975ecb96a0c594901eeee6a279cc66d9d5191022a7039ee6>
            SEPOLIA => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 3418831),
            // <https://arbiscan.io/tx/0xe2c3826bd7b15ef8d338038769fe6140a44f1957a36b0f27ab321ab6c68d5a8e>
            ARBITRUM_ONE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 222832),
            // <https://basescan.org/tx/0x0dc2e3d436424f2f038774805116896d31828c0bf3795a6901337bdec4e0dff6>
            BASE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 1196036),
            // <https://snowscan.xyz/tx/0xc49af0372feb032e0edbba6988410304566b1fd65546c01ced620ac3c934120f>
            AVALANCHE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 26386141),
            // <https://bscscan.com/tx/0x1de8caa6c54ff9a25600e26d80865d84c9cc4d33c2b98611240529ee7de5cd74>
            BNB => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 22691002),
            // <https://optimistic.etherscan.io/tx/0xa03cb990595df9eed6c5db17a09468cab534aed5f5589a06c0bb3d19dd2f7ce9>
            OPTIMISM => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 7003431),
            // <https://polygonscan.com/tx/0x66f275a2ed102a5b679c0894ced62c4ebcb2a65336d086a916eb83bd1fe5c8d2>
            POLYGON => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 15832990),
            // <https://explorer.inkonchain.com/address/0xBA12222222228d8Ba445958a75a0704d566BF2C8>
            INK => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 34313901),
            // Not available on Lens, Linea and Plasma
        ]))
        .add_contract(
            Contract::new("BalancerV3BatchRouter").with_networks(networks![
                // <https://etherscan.io/tx/0x41cb8619fb92dd532eb09b0e81fd4ce1c6006a10924893f02909e36a317777f3>
                MAINNET => ("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1", 21339510),
                // <https://gnosisscan.io/tx/0xeafddbace9f445266f851ef1d92928e3d01a4622a1a6780b41ac52d5872f12ab>
                GNOSIS => ("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b", 37377506),
                // <https://sepolia.etherscan.io/tx/0x95ed8e1aaaa7bdc5881f3c8fc5a4914a66639bee52987c3a1ea88545083b0681>
                SEPOLIA => ("0xC85b652685567C1B074e8c0D4389f83a2E458b1C", 7219301),
                // <https://arbiscan.io/tx/0xa7968c6bc0775208ffece789c6e5d09b0eea5f2c3ed2806e9bd94fb0b978ff0f>
                ARBITRUM_ONE => ("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E", 297828544),
                // <https://basescan.org/tx/0x47b81146714630ce50445bfa28872a36973acedf785317ca423498810ec8e76c>
                BASE => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 25347205),
                // <https://snowscan.xyz/tx/0x3bfaba7135ee2d67d98f20ee1aa4c8b7e81e47be64223376f3086bab429ac806>
                AVALANCHE => ("0xc9b36096f5201ea332Db35d6D195774ea0D5988f", 59965747),
                // <https://optimistic.etherscan.io/tx/0xf370aab0d652f3e0f7c34e1a53e1afd98e86c487138300b0939d4e54b0088b67>
                OPTIMISM => ("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E", 133969588),
                // <https://plasmascan.to/tx/0x7368609020622059d1061b4e3cc16aa448315956cace1e089f6876143cf2f90c?chainid=9745>
                PLASMA => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 782312),
                // Not available on Lens, Polygon, BNB, Linea and Ink
            ]),
        )
        // UniV2
        .add_contract(Contract::new("BaoswapRouter").with_networks(networks![
            // https://gnosisscan.io/tx/0xdcbfa037f2c6c7456022df0632ec8d6a75d0f9a195238eec679d5d26895eb7b1
            GNOSIS => "0x6093AeBAC87d62b1A5a4cEec91204e35020E38bE",
        ]))
        .add_contract(Contract::new("HoneyswapRouter").with_networks(networks![
            GNOSIS => "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77",
        ]))
        .add_contract(Contract::new("PancakeRouter").with_networks(networks![
            // <https://etherscan.io/tx/0x6e441248a9835ca10a3c29a19f2e1ed61d2e35f3ecb3a5b9e4ee170d62a22d16>
            MAINNET => "0xEfF92A263d31888d860bD50809A8D171709b7b1c",
            // <https://arbiscan.io/tx/0x4a2da73cbfcaafb0347e4525307a095e38bf7532435cb0327d1f5ee2ee15a011>
            ARBITRUM_ONE => "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb",
            // <https://basescan.org/tx/0xda322aef5776698ac6da56be1ffaa0f9994a983cdeb9f2aeaba47437809ae6ef>
            BASE => "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb",
            // <https://bscscan.com/tx/0x1bfbff8411ed44e609d911476b0d35a28284545b690902806ea0a7ff0453e931>
            BNB => "0x10ED43C718714eb63d5aA57B78B54704E256024E",
        ]))
        // <https://docs.sushi.com/contracts/cpamm>
        .add_contract(Contract::new("SushiSwapRouter").with_networks(networks![
            // <https://etherscan.io/tx/0x4ff39eceee7ba9a63736eae38be69b10347975ff5fa4d9b85743a51e1c384094>
            MAINNET => "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f",
            // <https://gnosisscan.io/tx/0x8b45ccbc2afd0132ef8b636064e0e745ff93b53942a56e320bb930666dd0fb18>
            GNOSIS => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            // <https://arbiscan.io/tx/0x40b22402bcac46330149ac9848f8bddd02b0a1e79d4a71934655a634051be1a1>
            ARBITRUM_ONE => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            // <https://basescan.org/tx/0xbb673c483292e03d202e95a023048b8bda459bf12402e7688f7e10be8b4dc67d>
            BASE => "0x6bded42c6da8fbf0d2ba55b2fa120c5e0c8d7891",
            // <https://snowtrace.io/tx/0x8185bcd3cc8544f8767e5270c4d7eb1e9b170fc0532fc4f0d7e7a1018e1f13ba>
            AVALANCHE => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            // <https://bscscan.com/tx/0xf22f827ae797390f6e478b0a11aa6e92d6da527f47130ef70d313ff0e0b2a83f>
            BNB => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            // <https://optimistic.etherscan.io/tx/0x88be6cc83f5bfccb8196db351866bac5c99ab8f7b451ea9975319ba05c3bf8f7>
            OPTIMISM => "0x2abf469074dc0b54d793850807e6eb5faf2625b1",
            // <https://polygonscan.com/tx/0x3dcf8fc780ae6fbe40b1ae57927a8fb405f54cbe89d0021a781a100d2086e5ba>
            POLYGON => "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506",
            // Not available on Lens
        ]))
        // <https://swapr.gitbook.io/swapr/contracts>
        .add_contract(Contract::new("SwaprRouter").with_networks(networks![
            // <https://etherscan.io/tx/0x3f4ccc676637906db24caf043c89dafce959321c02266c6a4ab706fcec79a5f7>
            MAINNET => "0xb9960d9bca016e9748be75dd52f02188b9d0829f",
            // <https://gnosisscan.io/tx/0x0406e774caced468b8f84d7c7ed9b6e9c324601af38f44e385aecf7a7d01feb4>
            GNOSIS => "0xE43e60736b1cb4a75ad25240E2f9a62Bff65c0C0",
            // <https://arbiscan.io/tx/0x09771774fc138775472910f6bb0f2e03ff74e1e32a658e9c3e4d8f59f6431ba8>
            ARBITRUM_ONE => "0x530476d5583724A89c8841eB6Da76E7Af4C0F17E",
            // Not available on Base and Lens
        ]))
        .add_contract(Contract::new("ISwaprPair"))
        .add_contract(
            Contract::new("TestnetUniswapV2Router02").with_networks(networks![
                // <https://sepolia.etherscan.io/tx/0x2bf9a91a42d53e161897d9c581f798df9db6fb00587803dde7e7b8859118d821>
                SEPOLIA => "0x86dcd3293C53Cf8EFd7303B57beb2a3F671dDE98",
            ]),
        )
        // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/factory>
        .add_contract(Contract::new("UniswapV2Factory").with_networks(networks![
            // <https://etherscan.io/tx/0xc31d7e7e85cab1d38ce1b8ac17e821ccd47dbde00f9d57f2bd8613bff9428396>
            MAINNET => "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
            // <https://gnosisscan.io/tx/0x446de52c460bed3f49a4342eab247bb4b2fe2993962c284fb9bc14a983c7a3d4>
            GNOSIS => "0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7",
            // <https://arbiscan.io/tx/0x83b597d54496c0b64d66a3b9a65c312e406262511c908f702ef06755d13ab2f3>
            ARBITRUM_ONE => "0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9",
            // <https://basescan.org/tx/0x3c94031f81d9afe3beeb8fbcf4dcf1bd5b5688b86081d94e3d0231514dc00d31>
            BASE => "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6",
            // <https://sepolia.etherscan.io/tx/0x0a5e26b22f6b470857957a1d5a92ad4a7d3c5e7cf254ddd80edfe23df70eae71>
            SEPOLIA => "0xF62c03E08ada871A0bEb309762E260a7a6a880E6",
            // <https://snowtrace.io/tx/0xd06a069b11fc0c998b404c5736957cc16c71cf1f7dbf8a7d4244c84036ea6edd>
            AVALANCHE => "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C",
            // <https://bscscan.com/tx/0x7305a4bddc54eee158f245a09526969697ac1a9f56d090b124ebfc85ff71a5cf>
            BNB => "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6",
            // <https://optimistic.etherscan.io/tx/0xf7227dcbbfa4ea2bb2634f2a1f364a64b028f9e9e393974fea8d435cd097c72e>
            OPTIMISM => "0x0c3c1c532F1e39EdF36BE9Fe0bE1410313E074Bf",
            // <https://polygonscan.com/tx/0x712ac56155a301fca4b7a761e232233f41a104865a74b1a59293835da355292a>
            POLYGON => "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C",
            // Not available on Lens and Ink (no Uniswap V2)
        ]))
        // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/router-02>
        .add_contract(Contract::new("UniswapV2Router02").with_networks(networks![
            // <https://etherscan.io/tx/0x4fc1580e7f66c58b7c26881cce0aab9c3509afe6e507527f30566fbf8039bcd0>
            MAINNET => "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",
            // <https://gnosisscan.io/tx/0xfcc495cdb313b48bbb0cd0a25cb2e8fd512eb8fb0b15f75947a9d5668e47a918>
            GNOSIS => "0x1C232F01118CB8B424793ae03F870aa7D0ac7f77",
            // <https://arbiscan.io/tx/0x630cd9d56a85e1bac7795d254fef861304a6838e28869badef19f19defb48ba6>
            ARBITRUM_ONE => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            // <https://basescan.org/tx/0x039224ce16ebe5574f51da761acbdfbd21099d6230c39fcd8ff566bbfd6a50a9>
            BASE => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            // <https://sepolia.etherscan.io/tx/0x92674b51681d2e99e71e03bd387bc0f0e79f2412302b49ed5626d1fa2311bab9>
            SEPOLIA => "0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3",
            // <https://snowtrace.io/tx/0x7372f1eedf9d32fb4185d486911f44542723dae766eea04bc3f14724bae9552e>
            AVALANCHE => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            // <https://bscscan.com/tx/0x9e940f846abea7dcc1f0bd5c261f405c104628c855346f8cac966f52905ee0fa>
            BNB => "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24",
            // <https://optimistic.etherscan.io/tx/0x2dcb9a76100e5be49e89085b87bd447b1966a9d823d5985e1a8197834c60e6bd>
            OPTIMISM => "0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2",
            // <https://polygonscan.com/tx/0x66186e0cacd2f6b3ad2eae586bd331daafd0572eb80bf71be694181858198025>
            POLYGON => "0xedf6066a2b290C185783862C7F4776A2C8077AD1",
            // Not available on Lens and Ink (no Uniswap V2)
        ]))
        .add_contract(Contract::new("IUniswapLikeRouter"))
        .add_contract(Contract::new("IUniswapLikePair"))
        .add_contract(Contract::new("UniswapV3Pool"))
        // <https://docs.uniswap.org/contracts/v3/reference/deployments/>
        .add_contract(Contract::new("UniswapV3QuoterV2").with_networks(networks![
            MAINNET => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            ARBITRUM_ONE => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            BASE => "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a",
            AVALANCHE => "0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F",
            BNB => "0x78D78E420Da98ad378D7799bE8f4AF69033EB077",
            OPTIMISM => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            POLYGON => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
            LENS => "0x1eEA2B790Dc527c5a4cd3d4f3ae8A2DDB65B2af1",
            LINEA => "0x42bE4D6527829FeFA1493e1fb9F3676d2425C3C1",
            // not official (deployed by oku trade)
            // <https://docs.oku.trade/home/extra-information/deployed-contracts#plasma>
            PLASMA => "0xaa52bB8110fE38D0d2d2AF0B85C3A3eE622CA455",
            // <https://explorer.inkonchain.com/address/0x96b572D2d880cf2Fa2563651BD23ADE6f5516652>
            INK => "0x96b572D2d880cf2Fa2563651BD23ADE6f5516652",
            // Not listed on Gnosis and Sepolia chains
        ]))
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        .add_contract(
            Contract::new("UniswapV3SwapRouterV2").with_networks(networks![
                ARBITRUM_ONE => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                MAINNET => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                POLYGON => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                OPTIMISM => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
                BASE => "0x2626664c2603336E57B271c5C0b26F421741e481",
                AVALANCHE => "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE",
                BNB => "0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2",
                LENS => "0x6ddD32cd941041D8b61df213B9f515A7D288Dc13",
                LINEA => "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a",
                // not official (deployed by oku trade)
                // <https://docs.oku.trade/home/extra-information/deployed-contracts#plasma>
                PLASMA => "0x807F4E281B7A3B324825C64ca53c69F0b418dE40",
                // <https://explorer.inkonchain.com/address/0x177778F19E89dD1012BdBe603F144088A95C4B53>
                INK => "0x177778F19E89dD1012BdBe603F144088A95C4B53",
                // Not available on Gnosis Chain
            ]),
        )
        // <https://github.com/Uniswap/v3-periphery/blob/697c2474757ea89fec12a4e6db16a574fe259610/deploys.md>
        .add_contract(Contract::new("IUniswapV3Factory").with_networks(networks![
            MAINNET => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            SEPOLIA => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            ARBITRUM_ONE => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            BASE => "0x33128a8fC17869897dcE68Ed026d694621f6FDfD",
            AVALANCHE => "0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD",
            BNB => "0xdB1d10011AD0Ff90774D0C6Bb92e5C5c8b4461F7",
            OPTIMISM => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            POLYGON => "0x1F98431c8aD98523631AE4a59f267346ea31F984",
            // not official
            LENS => "0xc3A5b857Ba82a2586A45a8B59ECc3AA50Bc3D0e3",
            LINEA => "0x31FAfd4889FA1269F7a13A66eE0fB458f27D72A9",
            // not official (deployed by oku trade)
            // <https://docs.oku.trade/home/extra-information/deployed-contracts#plasma>
            PLASMA => "0xcb2436774C3e191c85056d248EF4260ce5f27A9D",
            // <https://explorer.inkonchain.com/address/0x640887A9ba3A9C53Ed27D0F7e8246A4F933f3424>
            INK => "0x640887A9ba3A9C53Ed27D0F7e8246A4F933f3424",
            // Not available on Gnosis Chain
        ]))
        // <https://github.com/cowprotocol/hooks-trampoline/blob/993427166ade6c65875b932f853776299290ac4b/networks.json>
        .add_contract(Contract::new("HooksTrampoline").with_networks(networks![
            MAINNET => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            // Gnosis is using the old instance of the hook trampoline since it's hardcoded in gnosis pay rebalance integration.
            GNOSIS => "0x01DcB88678aedD0C4cC9552B20F4718550250574",
            SEPOLIA => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            ARBITRUM_ONE => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            BASE => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            AVALANCHE => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            BNB => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            OPTIMISM => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            POLYGON => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            LENS => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            LINEA => "0x60bf78233f48ec42ee3f101b9a05ec7878728006",
            PLASMA => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
            INK => "0x60Bf78233f48eC42eE3F101b9a05eC7878728006",
        ]))
        .add_contract(Contract::new("CoWSwapEthFlow").with_networks(networks![
            // <https://etherscan.io/tx/0x0247e3c15f59a52b099f192265f1c1e6227f48a280717b3eefd7a5d9d0c051a1>
            MAINNET => ("0x40a50cf069e992aa4536211b23f286ef88752187", 16169866),
            // <https://gnosisscan.io/tx/0x6280e079f454fbb5de3c52beddd64ca2b5be0a4b3ec74edfd5f47e118347d4fb>
            GNOSIS => ("0x40a50cf069e992aa4536211b23f286ef88752187", 25414331),
            // <https://github.com/cowprotocol/ethflowcontract/blob/v1.1.0-artifacts/networks.prod.json#L11-L14>
            // <https://sepolia.etherscan.io/tx/0x558a7608a770b5c4f68fffa9b02e7908a40f61b557b435ea768a4c62cb79ae25>
            SEPOLIA => ("0x0b7795E18767259CC253a2dF471db34c72B49516", 4718739),
            // <https://arbiscan.io/tx/0xa4066ca77bbe1f21776b4c26315ead3b1c054b35814b49e0c35afcbff23e1b8d>
            ARBITRUM_ONE => ("0x6DFE75B5ddce1ADE279D4fa6BD6AeF3cBb6f49dB", 204747458),
            // <https://basescan.org/tx/0xc3555c4b065867cbf34382438e1bbaf8ee39eaf10fb0c70940c8955962e76e2c>
            BASE => ("0x3C3eA1829891BC9bEC3d06A81d5d169e52a415e3", 21490258),
            // <https://snowscan.xyz/tx/0x71a2ed9754247210786effa3269bc6eb68b7521b5052ac9f205af7ac364f608f>
            AVALANCHE => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 60496408),
            // <https://bscscan.com/tx/0x959a60a42d36e0efd247b3cf19ed9d6da503d01bce6f87ed31e5e5921111222e>
            BNB => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 48411237),
            // <https://optimistic.etherscan.io/tx/0x0644f10f7ae5448240fc592ad21abf4dabac473a9d80904af5f7865f2d6509e2>
            OPTIMISM => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 134607215),
            // <https://polygonscan.com/tx/0xc3781c19674d97623d13afc938fca94d53583f4051020512100e84fecd230f91>
            POLYGON => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 71296258),
            // <https://explorer.lens.xyz/tx/0xc59b5ffadb40158f9390b1d77f19346dbe9214b27f26346dfa2990ad379a1a32>
            LENS => ("0xFb337f8a725A142f65fb9ff4902d41cc901de222", 3007173),
            // <https://lineascan.build/tx/0x0e20a4e0bbce2e28b89b7dcfc4dd4dfb48f5b0b8473b3b5bdeb1bf9f09943485>
            LINEA => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 24522097),
            // <https://plasmascan.to/tx/0xda72b111ac2a7d182bf3c884373882add6f4c78f6d4bdae7efcae143be716b38>
            PLASMA => ("0x04501b9b1d52e67f6862d157e00d13419d2d6e95", 3521855),
        ]))
        .add_contract(Contract::new("CoWSwapOnchainOrders"))
        .add_contract(Contract::new("ERC1271SignatureValidator"))
        // Used in the gnosis/solvers repo for the balancer solver
        .add_contract(Contract::new("BalancerQueries").with_networks(networks![
            // <https://etherscan.io/tx/0x30799534f3a0ab8c7fa492b88b56e9354152ffaddad15415184a3926c0dd9b09>
            MAINNET => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 15188261),
            // <https://arbiscan.io/tx/0x710d93aab52b6c10197eab20f9d6db1af3931f9890233d8832268291ef2f54b3>
            ARBITRUM_ONE => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 18238624),
            // <https://optimistic.etherscan.io/tx/0xf3b2aaf3e12c7de0987dc99a26242b227b9bc055342dda2e013dab0657d6f9f1>
            OPTIMISM => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 15288107),
            // <https://basescan.org/tx/0x425d04ee79511c17d06cd96fe1df9e0727f7e7d46b31f36ecaa044ada6a0d29a>
            BASE => ("0x300Ab2038EAc391f26D9F895dc61F8F66a548833", 1205869),
            // <https://gnosisscan.io/tx/0x5beb3051d393aac24cb236dc850c644f345af65c4927030bd1033403e2f2e503>
            GNOSIS => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 24821845),
            // <https://polygonscan.com/tx/0x0b74f5c230f9b7df8c7a7f0d1ebd5e6c3fab51a67a9bcc8f05c350180041682e>
            POLYGON => ("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5", 30988035),
            // <https://snowtrace.io/tx/0xf484e1efde47209bad5f72642bcb8d8e2a4092a5036434724ffa2d039e93a1bf?chainid=43114>
            AVALANCHE => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 26387068),
            // Not available on Lens, Linea, Plasma and Ink
        ]))
        // <https://liquorice.gitbook.io/liquorice-docs/links/smart-contracts>
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
        // Only used in <github.com/gnosis/solvers>
        .add_contract(Contract::new("Permit2").with_networks(networks![
            // <https://etherscan.io/tx/0xf2f1fe96c16ee674bb7fcee166be52465a418927d124f5f1d231b36eae65d377>
            MAINNET => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 15986406),
            // <https://gnosisscan.io/tx/0x3ba511410edc92cafe94bd100e25adb37981499d17947a3d64c8523fbfd31864>
            GNOSIS => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 27338672),
            // <https://sepolia.etherscan.io/tx/0x363df5deeead44d8fd38425f3986e3e81946a6c59d8b68fe33926cc700713173>
            SEPOLIA => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 2356287),
            // <https://arbiscan.io/tx/0xe244dafca8211ed6fb123efaa5075b7d5813749718412ca435c872afd0e2ea82>
            ARBITRUM_ONE => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 38692735),
            // <https://basescan.org/tx/0x26fbdea9a47ba8e21676bc6b6a72a19dded1a0c270e96d5236886ca9c5000d3f>
            BASE => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 1425180),
            // <https://snowscan.xyz/tx/0x38fd76c2165d920c7e006defd67eeeb0069bf93e41741eec3bbb83d196610a56>
            AVALANCHE => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 28844415),
            // <https://bscscan.com/tx/0xb038ec7b72db4207e0c0d5433e1cabc41b4e4f9b9cac577173b3188fc508a6c3>
            BNB => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 25343783),
            // <https://optimistic.etherscan.io/tx/0xf0a51e0d0579ef8cc7965f5797bd7665ddac14d4d2141423676b8862f7668352>
            OPTIMISM => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 38854427),
            // <https://polygonscan.com/tx/0xe2a4d996de0d6a23108f701b37acba6c47ee34448bb51fec5c23f542a6f3ccc8>
            POLYGON => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 35701901),
            // <https://plasmascan.to/tx/0x6d19057fb8b25b17fe9107be7726007d503dae50d1b0965b422fc3b512bf5dbe?chainid=9745>
            PLASMA => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 7808),
            // <https://explorer.inkonchain.com/address/0x000000000022D473030F116dDEE9F6B43aC78BA3> Pre Install
            INK => ("0x000000000022D473030F116dDEE9F6B43aC78BA3", 0)
        ]))
        .add_contract(
            Contract::new("GPv2AllowListAuthentication").with_networks(networks![
                // <https://etherscan.io/tx/0xb84bf720364f94c749f1ec1cdf0d4c44c70411b716459aaccfd24fc677013375>
                MAINNET => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 12593263),
                // <https://gnosisscan.io/tx/0x1a2d87a05a94bc6680a4faee31bbafbd74e9ddb63dd3941c717b5c609c08b957>
                GNOSIS => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 16465099),
                // <https://sepolia.etherscan.io/tx/0x73c54c75b3f382304f3adf33e3876c8999fb10df786d4a902733369251033cd1>
                SEPOLIA => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 4717469),
                // <https://arbiscan.io/tx/0xe994adff141a2e72bd9dab3eb7b3480637013bdfb1aa42c62d9d6c90de091237>
                ARBITRUM_ONE => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 204702129),
                // <https://basescan.org/tx/0x5497004d2a37c9eafd0bd1e5861a67d3a209c5b845724166e3dbca9527ee05ec>
                BASE => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 21407137),
                // <https://snowscan.xyz/tx/0xa58fc76846917779d7bcbb7d34f4a2a44aab2b702ef983594e34e6972a0c626b>
                AVALANCHE => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 59891351),
                // <https://bscscan.com/tx/0x8da639c62eb4a810573c178ed245184944d66c834122e3f88994ebf679b50e34>
                BNB => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 48173639),
                // <https://optimistic.etherscan.io/tx/0x5b6403b485e369ce524d04234807df782e6639e55a7c1d859f0a67925d9a5f49>
                OPTIMISM => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 134254466),
                // <https://polygonscan.com/tx/0x686e4bbcfd6ebae91f0fcc667407c831953629877ec622457916729de3d461c3>
                POLYGON => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 45854728),
                // <https://explorer.lens.xyz/tx/0x0730c21885153dcc9a25ab7abdc38309ec7c7a8db15b763fbbaf574d1e7ec498>
                LENS => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 2612937),
                // <https://lineascan.build/tx/0x6e5d2c4381320efdd21ccde1534560ded1b9ab07638776833faa22820c378155>
                LINEA => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 24333100),
                // <https://plasmascan.to/tx/0xc2ac50ad302e402c4db1e956bd357af7d84e3684ad65e4fdee58abea092ac88c>
                PLASMA => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 3439709),
                // <https://explorer.inkonchain.com/address/0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE>
                INK => ("0x2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE", 34436840),
            ]),
        )
        .add_contract(Contract::new("GPv2Settlement").with_networks(networks![
            // <https://etherscan.io/tx/0xf49f90aa5a268c40001d1227b76bb4dd8247f18361fcad9fffd4a7a44f1320d3>
            MAINNET => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 12593265),
            // <https://blockscout.com/xdai/mainnet/tx/0x9ddc538f89cd8433f4a19bc4de0de27e7c68a1d04a14b327185e4bba9af87133>
            GNOSIS => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 16465100),
            // <https://sepolia.etherscan.io/tx/0x6bba22a00ffcff6bca79aced546e18d2a5a4f4e484a4e4dbafab13daf42f718d>
            SEPOLIA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 4717488),
            // <https://arbiscan.io/tx/0x240486f35ebf42ea69b2b3f1569d587c18c87f98c0ec997bef7d18182ca4c38c>
            ARBITRUM_ONE => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 204704802),
            // <https://basescan.org/tx/0x00a3c4e2dc4241465208beeba27e90a9ce3159ad4f41581c4c3a1ef02d6e37cb>
            BASE => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 21407238),
            // <https://snowscan.xyz/tx/0x374b84f0ea6bc554abc3ffdc3fbce4374fefc76f2bd25e324ce95a62cafcc142>
            AVALANCHE => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 59891356),
            // <https://bscscan.com/tx/0x9e0c16a655ceadcb95ba2de3bf59d2b3a3d10cce7bdf52aa5520164b58ffd969>
            BNB => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 48173641),
            // <https://optimistic.etherscan.io/tx/0xd1bbd68ee6b0eecf6f883e148284fc4fb4c960299b75004dfddd5135246cd5eb>
            OPTIMISM => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 134254624),
            // <https://polygonscan.com/tx/0x0e24d3a2a8530eaad5ae62e54e64d57665a77ce3970227d20c1b77da315cbbf6>
            POLYGON => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 45859743),
            // <https://explorer.lens.xyz/tx/0x01584b767dda7b115394b93dbcfecadfe589862ae3f7957846a2db82f2f5c703>
            LENS => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 2621745),
            // <https://lineascan.build/tx/0x6e5d2c4381320efdd21ccde1534560ded1b9ab07638776833faa22820c378155>
            LINEA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 24333100),
            // <https://plasmascan.to/tx/0xf16bd6f307dce94ab252d8dd8266ab30091879fb3d631cbcf3d0ffddf9e6ad05?chainid=9745>
            PLASMA => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 3439711),
            // <https://explorer.inkonchain.com/address/0x9008D19f58AAbD9eD0D60971565AA8510560ab41>
            INK => ("0x9008D19f58AAbD9eD0D60971565AA8510560ab41", 34436849),
        ]))
        // Note: the WETH address must be consistent with the one used by the ETH-flow
        // contract
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
            LENS => "0x6bDc36E20D267Ff0dd6097799f82e78907105e2F",
            LINEA => "0xe5d7c2a44ffddf6b295a15c148167daaaf5cf34f",
            PLASMA => "0x6100E367285b01F48D07953803A2d8dCA5D19873",
            INK => "0x4200000000000000000000000000000000000006",
        ]))
        .add_submodule(
            Submodule::new("cow_amm")
                .add_contract(Contract::new("CowAmm"))
                .add_contract(Contract::new("CowAmmConstantProductFactory").with_networks(
                    networks![
                        // <https://etherscan.io/tx/0xf37fc438ddacb00c28305bd7dea3b79091cd5be3405a2b445717d9faf946fa50>
                        MAINNET => ("0x40664207e3375FB4b733d4743CE9b159331fd034", 19861952),
                        // <https://gnosisscan.io/tx/0x4121efab4ad58ae7ad73b50448cccae0de92905e181648e5e08de3d6d9c66083>
                        GNOSIS => ("0xdb1cba3a87f2db53b6e1e6af48e28ed877592ec0", 33874317),
                        // <https://sepolia.etherscan.io/tx/0x5e6af00c670eb421b96e78fd2e3b9df573b19e6e0ea77d8003e47cdde384b048>
                        SEPOLIA => ("0xb808e8183e3a72d196457d127c7fd4befa0d7fd3", 5874562),
                    ],
                ))
                .add_contract(Contract::new("CowAmmLegacyHelper").with_networks(networks![
                    // <https://etherscan.io/tx/0x07f0ce50fb9cd30e69799a63ae9100869a3c653d62ea3ba49d2e5e1282f42b63>
                    MAINNET => ("0x3705ceee5eaa561e3157cf92641ce28c45a3999c", 20332745),
                    // <https://gnosisscan.io/tx/0x09e56c7173ab1e1c5d02bc2832799422ebca6d9a40e5bae77f6ca908696bfebf>
                    GNOSIS => ("0xd9ec06b001957498ab1bc716145515d1d0e30ffb", 35026999),
                ]))
                .add_contract(Contract::new("CowAmmUniswapV2PriceOracle"))
                .add_contract(Contract::new("CowAmmFactoryGetter")),
        )
        .add_submodule(
            Submodule::new("test") // Test Contract for using up a specified amount of gas.
                .add_contract(Contract::new("GasHog"))
                // Test Contract for incrementing arbitrary counters.
                .add_contract(Contract::new("Counter"))
                // Token with support for `permit` (for pre-interaction tests)
                .add_contract(Contract::new("CowProtocolToken").with_networks(networks![
                    MAINNET => "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                    GNOSIS => "0x177127622c4A00F3d409B75571e12cB3c8973d3c",
                    SEPOLIA => "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59",
                    ARBITRUM_ONE => "0xcb8b5CD20BdCaea9a010aC1F8d835824F5C87A04",
                    BASE => "0xc694a91e6b071bF030A18BD3053A7fE09B6DaE69",
                ]))
                // Test Contract with non-standard ERC20 balance storage
                .add_contract(Contract::new("NonStandardERC20Balances"))
                // Test Contract which selectively pulls balance storage from another target
                .add_contract(Contract::new("RemoteERC20Balances")),
        )
        .add_submodule(
            Submodule::new("support") // support module
                // Support contracts used for trade and token simulations.
                .add_contract(Contract::new("AnyoneAuthenticator"))
                .add_contract(Contract::new("Solver"))
                .add_contract(Contract::new("Spardose"))
                .add_contract(Contract::new("Trader"))
                // Support contract used for solver fee simulations in the gnosis/solvers repo.
                .add_contract(Contract::new("Swapper"))
                .add_contract(Contract::new("Signatures").with_networks(networks![
                    MAINNET => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    ARBITRUM_ONE => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    BASE => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    AVALANCHE => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    BNB => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    OPTIMISM => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    POLYGON => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    LENS => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    GNOSIS => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    SEPOLIA => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    // built with evm=London, because deployment reverts on Linea otherwise
                    LINEA => "0xf6E57e72F7dB3D9A51a8B4c149C00475b94A37e4",
                    PLASMA => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                    INK => "0x8262d639c38470F38d2eff15926F7071c28057Af",
                ]))
                // Support contracts used for various order simulations.
                .add_contract(Contract::new("Balances").with_networks(networks![
                    MAINNET => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    ARBITRUM_ONE => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    BASE => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    AVALANCHE => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    BNB => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    OPTIMISM => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    POLYGON => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    LENS => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    GNOSIS => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    SEPOLIA => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    PLASMA => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                    // built with evm=London, because deployment reverts on Linea otherwise
                    LINEA => "0x361350f708f7c0c63c8a505226592c3e5d1faa29",
                    INK => "0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b",
                ])),
        )
        .write_formatted(Path::new("artifacts"), true, vendored_bindings)
        .unwrap();
}

// Codegen implementation starts here

const MOD_HEADER: &str = r#"#![allow(unused_imports, unused_attributes, clippy::all, rustdoc::all, non_snake_case)]
    //! This module contains the sol! generated bindings for solidity contracts.
    //! This is autogenerated code.
    //! Do not manually edit these files.
    //! These files may be overwritten by the codegen system at any time.
    "#;

#[derive(Default)]
pub struct Module {
    pub contracts: Vec<Contract>,
    submodules: Vec<Submodule>,
}

impl Module {
    pub fn add_contract(mut self, contract: Contract) -> Self {
        self.contracts.push(contract);
        self
    }

    pub fn add_submodule(mut self, module: Submodule) -> Self {
        self.submodules.push(module);
        self
    }

    pub fn write_formatted<P1, P2>(
        self,
        bindings_folder: P1,
        all_derives: bool,
        output_folder: P2,
    ) -> anyhow::Result<()>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        std::fs::create_dir_all(&output_folder)?;

        let mut mod_file = String::from(MOD_HEADER);
        for submodule in self.submodules {
            write_mod_name(&mut mod_file, &submodule.name)?;
            submodule.write_formatted(bindings_folder.as_ref(), all_derives, &output_folder)?;
        }

        for contract in self.contracts {
            let name = contract.name.clone();
            contract.write_formatted(bindings_folder.as_ref(), all_derives, &output_folder)?;
            write_mod_name(&mut mod_file, &name).context(name)?;
        }

        let file: syn::File = syn::parse_file(&mod_file)?;
        let formatted = prettyplease::unparse(&file);
        std::fs::write(output_folder.as_ref().join("mod.rs"), formatted)?;

        Ok(())
    }
}

pub struct Submodule {
    pub name: String,
    pub contracts: Vec<Contract>,
    submodules: Vec<Submodule>,
}

impl Submodule {
    pub fn new<S: ToString>(name: S) -> Self {
        Self {
            name: name.to_string(),
            contracts: vec![],
            submodules: vec![],
        }
    }

    pub fn add_contract(mut self, contract: Contract) -> Self {
        self.contracts.push(contract);
        self
    }

    pub fn add_submodule(mut self, module: Submodule) -> Self {
        self.submodules.push(module);
        self
    }

    pub fn write_formatted<P1, P2>(
        self,
        bindings_folder: P1,
        all_derives: bool,
        output_folder: P2,
    ) -> anyhow::Result<()>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        let output_folder = output_folder.as_ref().join(self.name);
        std::fs::create_dir_all(&output_folder)?;

        let mut mod_file = String::from(MOD_HEADER);
        for submodule in self.submodules {
            write_mod_name(&mut mod_file, &submodule.name)?;
            submodule.write_formatted(bindings_folder.as_ref(), all_derives, &output_folder)?;
        }

        for contract in self.contracts {
            let name = contract.name.clone();
            contract.write_formatted(bindings_folder.as_ref(), all_derives, &output_folder)?;
            write_mod_name(&mut mod_file, &name)?;
        }

        let file: syn::File = syn::parse_file(&mod_file)?;
        let formatted = prettyplease::unparse(&file);
        std::fs::write(output_folder.join("mod.rs"), formatted)?;

        Ok(())
    }
}

pub struct Contract {
    pub name: String,
    networks: HashMap<u64, (String, Option<u64>)>,
}

impl Contract {
    pub fn new<S: AsRef<str>>(name: S) -> Self {
        Self {
            name: name.as_ref().to_string(),
            networks: HashMap::new(),
        }
    }

    pub fn with_networks<S, I>(mut self, networks: I) -> Self
    where
        S: AsRef<str>,
        I: IntoIterator<Item = (u64, (S, Option<u64>))>,
    {
        for (id, (address, block_number)) in networks.into_iter() {
            self.networks
                .insert(id, (address.as_ref().to_string(), block_number));
        }
        self
    }

    fn bindings_path(&self, bindings_folder: &Path) -> PathBuf {
        bindings_folder.join(&self.name).with_extension("json")
    }

    pub fn generate<P: AsRef<Path>>(
        self,
        bindings_folder: P,
        all_derives: bool,
    ) -> anyhow::Result<proc_macro2::TokenStream> {
        let bindings_path = self.bindings_path(bindings_folder.as_ref());
        let mut macrogen = SolMacroGen::new(bindings_path, self.name.clone());
        generate_binding(&mut macrogen, all_derives)?;
        let mut expansion = macrogen
            .expansion
            .expect("if the expansion failed, it should have errored earlier");

        let module_name_ident = format_ident!("{}", self.name);
        let instance_name_ident = format_ident!("{}Instance", self.name);
        let instance = quote::quote! {
            pub type Instance = #module_name_ident :: #instance_name_ident<::alloy::providers::DynProvider>;
        };
        expansion.extend(instance);

        let no_networks = self.networks.is_empty();
        let networks = self.networks.into_iter().map(NetworkArm::from);
        let deployment_info = if no_networks {
            proc_macro2::TokenStream::new()
        } else {
            quote::quote! {
                use {
                    std::{
                        sync::LazyLock,
                        collections::HashMap
                    },
                    anyhow::{Result, Context},
                    alloy::{
                        providers::{Provider, DynProvider},
                        primitives::{address, Address},
                    },
                };

                pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
                    match chain_id {
                        #( #networks ,)*
                        _ => None
                    }
                }

                /// Returns the contract's deployment address (if one exists) for the given chain.
                pub const fn deployment_address(chain_id: &u64) -> Option<::alloy::primitives::Address> {
                    match deployment_info(*chain_id) {
                        Some((address, _)) => Some(address),
                        None => None,
                    }
                }

                /// Returns the contract's deployment block (if one exists) for the given chain.
                pub const fn deployment_block(chain_id: &u64) -> Option<u64> {
                    match deployment_info(*chain_id) {
                        Some((_, block)) => block,
                        None => None,
                    }
                }

                impl Instance {
                    pub fn deployed(provider: &DynProvider) -> impl Future<Output = Result<Self>> + Send {
                        async move {
                            let chain_id = provider
                                .get_chain_id()
                                .await
                                .context("could not fetch current chain id")?;

                            let (address, _deployed_block) = deployment_info(chain_id)
                                .with_context(|| format!("no deployment info for chain {chain_id:?}"))?;

                            Ok(Instance::new(
                                address,
                                provider.clone(),
                            ))
                        }
                    }
                }
            }
        };
        expansion.extend(deployment_info);

        Ok(expansion)
    }

    pub fn write_formatted<P1, P2>(
        self,
        bindings_folder: P1,
        all_derives: bool,
        output_folder: P2,
    ) -> anyhow::Result<()>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        let name = self.name.clone();
        let token_stream = self.generate(bindings_folder, all_derives)?;
        let mut buffer = String::new();
        write!(buffer, "{}", token_stream)?;
        let file: syn::File = syn::parse_file(&buffer)?;
        let formatted = prettyplease::unparse(&file);
        std::fs::write(
            output_folder.as_ref().join(name).with_extension("rs"),
            formatted,
        )?;
        Ok(())
    }
}

fn generate_binding(instance: &mut SolMacroGen, all_derives: bool) -> anyhow::Result<()> {
    let input = instance
        .get_sol_input()
        .map_err(|err| anyhow::anyhow!("{:?}", err))?
        .normalize_json()
        .map_err(|err| anyhow::anyhow!("{:?}", err))?;
    let SolInput {
        attrs: _,
        path: _,
        kind,
    } = input;

    let tokens = match kind {
        SolInputKind::Sol(mut file) => {
            let sol_attr: syn::Attribute = if all_derives {
                syn::parse_quote! {
                        #[sol(rpc, alloy_sol_types = alloy::sol_types, alloy_contract =
                alloy::contract, all_derives = true, extra_derives(serde::Serialize,
                serde::Deserialize))]     }
            } else {
                syn::parse_quote! {
                        #[sol(rpc, alloy_sol_types = alloy::sol_types, alloy_contract =
                alloy::contract)]     }
            };
            file.attrs.push(sol_attr);
            expand(file)?
        }
        _ => unreachable!(),
    };

    instance.expansion = Some(tokens);
    Ok(())
}

fn write_mod_name(contents: &mut String, name: &str) -> anyhow::Result<()> {
    if syn::parse_str::<syn::ItemMod>(&format!("pub mod {name};")).is_ok() {
        write!(contents, "pub mod {name};")?;
    } else {
        write!(contents, "pub mod r#{name};")?;
    }
    Ok(())
}

/// Wrapper to avoid destructuring the vector of tuples into three iterators.
///
/// The following code:
/// ```ignore
/// // We need to "destructure" the iterator into several due to
/// let chain_ids = self.networks.iter().map(|n| n.0);
/// let addresses = self.networks.iter().map(|n| n.1.0.clone());
/// let blocks = self.networks.iter().map(|n| match n.1.1 {
///     Some(block) => quote::quote! {Some (#block)},
///     None => quote::quote! {None},
/// });
/// // ...
/// quote::quote! {
///     pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
///         match chain_id {
///             #(#chain_id => Some((::alloy::primitives::address!(#address), #block_number)),)*
///             _ => None
///         }
///     }
/// }
/// ```
///
/// Becomes:
/// ```ignore
/// let networks = self.networks.into_iter().map(NetworkArm::from);
/// // ...
/// quote::quote! {
///     pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
///         match chain_id {
///             #( #networks ,)*
///             _ => None
///         }
///     }
/// }
/// ```
struct NetworkArm(u64, (String, Option<u64>));

impl ToTokens for NetworkArm {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let chain_id = self.0;
        let address = &self.1.0;
        let block_number = match self.1.1 {
            Some(block) => quote::quote! {Some (#block)},
            None => quote::quote! {None},
        };
        tokens.extend(quote::quote! {
           #chain_id => Some((::alloy::primitives::address!(#address), #block_number))
        });
    }
}

impl From<(u64, (String, Option<u64>))> for NetworkArm {
    fn from((chain_id, info): (u64, (String, Option<u64>))) -> Self {
        Self(chain_id, info)
    }
}

/// SolMacroGen implementation vendored from
/// <https://github.com/foundry-rs/foundry/blob/cc24b6b74978e72b7330ad7d4b39140e9ee33deb/crates/sol-macro-gen/src/sol_macro_gen.rs>
/// to avoid depending on forge-sol-macro-gen.
pub struct SolMacroGen {
    pub path: PathBuf,
    pub name: String,
    pub expansion: Option<TokenStream>,
}

impl SolMacroGen {
    pub fn new(path: PathBuf, name: String) -> Self {
        Self {
            path,
            name,
            expansion: None,
        }
    }

    pub fn get_sol_input(&self) -> Result<SolInput> {
        let path = self.path.to_string_lossy().into_owned();
        let name = proc_macro2::Ident::new(&self.name, Span::call_site());
        let tokens = quote::quote! {
            #[sol(ignore_unlinked)]
            #name,
            #path
        };

        let sol_input: SolInput = syn::parse2(tokens).context("failed to parse input")?;

        Ok(sol_input)
    }
}
