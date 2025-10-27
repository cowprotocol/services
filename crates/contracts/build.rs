use contracts_generate::{Contract, Contracts, networks};

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

pub mod networks {
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

    std::fs::create_dir_all("src/bindings").unwrap();
    Contracts::new()
        .add_contract(Contract::new("IZeroex").with_networks(networks![
            networks::MAINNET => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            networks::SEPOLIA => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            networks::ARBITRUM_ONE => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            networks::BASE => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            networks::AVALANCHE => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            networks::BNB => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            networks::OPTIMISM => "0xdef1abe32c034e558cdd535791643c58a13acc10",
            networks::POLYGON => "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
        ]))
        .add_contract(Contract::new("ERC20Mintable"))
        .add_contract(Contract::new("GnosisSafe"))
        .add_contract(Contract::new("GnosisSafeCompatibilityFallbackHandler"))
        .add_contract(Contract::new("GnosisSafeProxy"))
        .add_contract(Contract::new("GnosisSafeProxyFactory"))
        .add_contract(Contract::new("BalancerV2Authorizer"))
        .add_contract(Contract::new("BalancerV2BasePool"))
        .add_contract(Contract::new("BalancerV2BasePoolFactory"))
        .add_contract(Contract::new("BalancerV2WeightedPool"))
        .add_contract(Contract::new("BalancerV2StablePool"))
        .add_contract(Contract::new("BalancerV2ComposableStablePool"))
        .add_contract(Contract::new("BalancerV2LiquidityBootstrappingPool"))
        .add_contract(Contract::new("BalancerV2WeightedPoolFactory").with_networks(networks![
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
            // <https://etherscan.io/tx/0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792>
            networks::MAINNET => ("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9", 12272147),
        ]))
        .add_contract(Contract::new("BalancerV2WeightedPoolFactoryV3").with_networks(networks![
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
            // <https://etherscan.io/tx/0x39f357b78c03954f0bcee2288bf3b223f454816c141ef20399a7bf38057254c4>
            networks::MAINNET => ("0x5Dd94Da3644DDD055fcf6B3E1aa310Bb7801EB8b", 16520627),
            // <https://gnosisscan.io/tx/0x2ac3d873b6f43de6dd77525c7e5b68a8fc3a1dee40303e1b6a680b0285b26091>
            networks::GNOSIS => ("0xC128a9954e6c874eA3d62ce62B468bA073093F25", 26226256),
            // <https://snowscan.xyz/tx/0xdf2c77743cc9287df2022cd6c5f9209ecfecde07371717ab0427d96042a88640>
            networks::AVALANCHE => ("0x94f68b54191F62f781Fe8298A8A5Fa3ed772d227", 26389236),
            // <https://optimistic.etherscan.io/tx/0xc5e79fb00b9a8e2c89b136aae0be098e58f8e832ede13e8079213a75c9cd9c08>
            networks::OPTIMISM => ("0xA0DAbEBAAd1b243BBb243f933013d560819eB66f", 72832703),
            // <https://polygonscan.com/tx/0x2bc079c0e725f43670898b474afedf38462feee72ef8e874a1efcec0736672fc>
            networks::POLYGON => ("0x82e4cFaef85b1B6299935340c964C942280327f4", 39036828),
            // <https://bscscan.com/tx/0x91107b9581e18ec0a4a575d4713bdd7b1fc08656c35522d216307930aa4de7b6>
            networks::BNB => ("0x6e4cF292C5349c79cCd66349c3Ed56357dD11B46", 25474982),
        ]))
        .add_contract(Contract::new("BalancerV2WeightedPoolFactoryV4").with_networks(networks![
            // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
            networks::MAINNET => ("0x897888115Ada5773E02aA29F775430BFB5F34c51", 16878323),
            // <https://gnosisscan.io/tx/0xcb6768bd92add227d46668357291e1d67c864769d353f9f0041c59ad2a3b21bf>
            networks::GNOSIS => ("0x6CaD2ea22BFA7F4C14Aae92E47F510Cd5C509bc7", 27055829),
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#pool-factories>
            // <https://sepolia.etherscan.io/tx/0x7dd392b586f1cdecfc635e7dd40ee1444a7836772811e59321fd4873ecfdf3eb>
            networks::SEPOLIA => ("0x7920BFa1b2041911b354747CA7A6cDD2dfC50Cfd", 3424893),
            // <https://arbiscan.io/tx/0x167fe7eb776d1be36b21402d8ae120088c393e28ae7ca0bd1defac84e0f2848b>
            networks::ARBITRUM_ONE => ("0xc7E5ED1054A24Ef31D827E6F86caA58B3Bc168d7", 72222060),
            // <https://basescan.org/tx/0x0732d3a45a3233a134d6e0e72a00ca7a971d82cdc51f71477892ac517bf0d4c9>
            networks::BASE => ("0x4C32a8a8fDa4E24139B51b456B42290f51d6A1c4", 1204869),
            // <https://snowscan.xyz/tx/0xa3fc8aab3b9baba3905045a53e52a47daafe79d4aa26d4fef5c51f3840aa55fa>
            networks::AVALANCHE => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 27739006),
            // <https://optimistic.etherscan.io/tx/0xad915050179db368e43703f3ee1ec55ff5e5e5e0268c15f8839c9f360caf7b0b>
            networks::OPTIMISM => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 82737545),
            // <https://polygonscan.com/tx/0x65e6b13231c2c5656357005a9e419ad6697178ae74eda1ea7522ecdafcf77136>
            networks::POLYGON => ("0xFc8a407Bba312ac761D8BFe04CE1201904842B76", 40611103),
            // <https://bscscan.com/tx/0xc7fada60761e3240332c4cbd169633f1828b2a15de23f0148db9d121afebbb4b>
            networks::BNB => ("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a", 26665331),
        ]))
        .add_contract(Contract::new("BalancerV2WeightedPool2TokensFactory").with_networks(networks![
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
            // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
            networks::MAINNET => ("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0", 12349891),
            networks::ARBITRUM_ONE => ("0xCF0a32Bbef8F064969F21f7e02328FB577382018", 222864),
            // <https://optimistic.etherscan.io/tx/0xd5754950d47179d822ea976a8b2af82ffa80e992cf0660b02c0c218359cc8987>
            networks::OPTIMISM => ("0xdAE7e32ADc5d490a43cCba1f0c736033F2b4eFca", 7005512),
            // <https://polygonscan.com/tx/0xb8ac851249cc95bc0943ef0732d28bbd53b0b36c7dd808372666acd8c5f26e1c>
            networks::POLYGON => ("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9", 15832998),
        ]))
        .add_contract(Contract::new("BalancerV2LiquidityBootstrappingPoolFactory").with_networks(networks![
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
            // <https://etherscan.io/tx/0x665ac1c7c5290d70154d9dfc1d91dc2562b143aaa9e8a77aa13e7053e4fe9b7c>
            networks::MAINNET => ("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE", 12871780),
            networks::ARBITRUM_ONE => ("0x142B9666a0a3A30477b052962ddA81547E7029ab", 222870),
            // <https://polygonscan.com/tx/0xd9b5b9a9e6ea17a87f85574e93577e3646c9c2f9c8f38644f936949e6c853288>
            networks::POLYGON => ("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE", 17116402),
        ]))
        .add_contract(Contract::new("BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory").with_networks(networks![
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
            // <https://etherscan.io/tx/0x298381e567ff6643d9b32e8e7e9ff0f04a80929dce3e004f6fa1a0104b2b69c3>
            networks::MAINNET => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 13730248),
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/gnosis.html#ungrouped-active-current-contracts>
            // <https://gnosis.blockscout.com/tx/0xbd56fefdb27e4ff1c0852e405f78311d6bc2befabaf6c87a405ab19de8c1506a>
            networks::GNOSIS => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 25415236),
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
            // <https://sepolia.etherscan.io/tx/0xe0e8feb509a8aa8a1eaa0b0c4b34395ff2fd880fb854fbeeccc0af1826e395c9>
            networks::SEPOLIA => ("0x45fFd460cC6642B8D8Fb12373DFd77Ceb0f4932B", 25415236),
            networks::ARBITRUM_ONE => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 4859669),
            // <https://basescan.org/tx/0x0529de9dbe772f4b4f48da93ae2c2d2c46e3d3221ced9e0c4063a7a5bc47e874>
            networks::BASE => ("0x0c6052254551EAe3ECac77B01DFcf1025418828f", 1206531),
            // <https://snowscan.xyz/tx/0x33a75d83436ae9fcda4b4986713417bf3dc80d9ceb8d2541817846b1ac579d9f>
            networks::AVALANCHE => ("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e", 26386552),
            // <https://bscscan.com/tx/0x8b964b97e6091bd41c93002c558d49adc26b8b31d2b30f3a33babbbbe8c55f47>
            networks::BNB => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 22691243),
            // <https://optimistic.etherscan.io/tx/0x14fb43f051eebdec645abf0125e52348dc875b0887b689f8db026d75f9c78dda>
            networks::OPTIMISM => ("0xf302f9F50958c5593770FDf4d4812309fF77414f", 7005915),
            // <https://polygonscan.com/tx/0x125bc007a86d771f8dc8f5fa1017de6e5a11162a458a72f25814503404bbeb0b>
            networks::POLYGON => ("0x41B953164995c11C81DA73D212ED8Af25741b7Ac", 22067480),
        ]))
        .add_contract(Contract::new("BalancerV2StablePoolFactoryV2").with_networks(networks![
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
            // <https://etherscan.io/tx/0xef36451947ebd97b72278face57a53806e90071f4c902259db2db41d0c9a143d>
            networks::MAINNET => ("0x8df6efec5547e31b0eb7d1291b511ff8a2bf987c", 14934936),
            // <https://gnosisscan.io/tx/0xe062237f0c8583375b10cf514d091781bfcd52d9ababbd324180770a5efbc6b1>
            networks::GNOSIS => ("0xf23b4DB826DbA14c0e857029dfF076b1c0264843", 25415344),
            networks::ARBITRUM_ONE => ("0xEF44D6786b2b4d544b7850Fe67CE6381626Bf2D6", 14244664),
            // <https://optimistic.etherscan.io/tx/0xcf9f0bd731ded0e513708200df28ac11d17246fb53fc852cddedf590e41c9c03>
            networks::OPTIMISM => ("0xeb151668006CD04DAdD098AFd0a82e78F77076c3", 11088891),
            // <https://polygonscan.com/tx/0xa2c41d014791888a29a9491204446c1b9b2f5dee3f3eb31ad03f290259067b44>
            networks::POLYGON => ("0xcA96C4f198d343E251b1a01F3EBA061ef3DA73C1", 29371951),
        ]))
        .add_contract(Contract::new("BalancerV2ComposableStablePoolFactory").with_networks(networks![
            // <https://etherscan.io/tx/0x3b9e93ae050e59b3ca3657958ca30d1fd13fbc43208f8f0aa01ae992294f9961>
            networks::MAINNET => ("0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F", 15485885),
            networks::ARBITRUM_ONE => ("0xaEb406b0E430BF5Ea2Dc0B9Fe62E4E53f74B3a33", 23227044),
            // <https://bscscan.com/tx/0x6c6e1c72c91c75714f698049f1c7b66d8f2baced54e0dd2522dfadff27b5ccf1>
            networks::BNB => ("0xf302f9F50958c5593770FDf4d4812309fF77414f", 22691193),
            // <https://optimistic.etherscan.io/tx/0xad2f330ad865dc7955376a3d9733486b38c53ba0d4757ad4e1b63b105401c506>
            networks::OPTIMISM => ("0xf145caFB67081895EE80eB7c04A30Cf87f07b745", 22182522),
            // <https://polygonscan.com/tx/0xe5d908be686056f1519663a407167c088924f60d29c799ec74438b9de891989e>
            networks::POLYGON => ("0x136FD06Fa01eCF624C7F2B3CB15742c1339dC2c4", 32774224),
        ]))
        .add_contract(Contract::new("BalancerV2ComposableStablePoolFactoryV3").with_networks(networks![
            // <https://etherscan.io/tx/0xd8c9ba758cb318beb0c9525b7621280a22b6dfe02cf725a3ece0718598f260ef>
            networks::MAINNET => ("0xdba127fBc23fb20F5929C546af220A991b5C6e01", 16580899),
            // <https://gnosisscan.io/tx/0x2abd7c865f8ab432b340f7de897192c677ffa254908fdec14091e0cd06962963>
            networks::GNOSIS => ("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD", 26365805),
            networks::ARBITRUM_ONE => ("0x1c99324EDC771c82A0DCCB780CC7DDA0045E50e7", 58948370),
            // <https://bscscan.com/tx/0xfe0c47c2b124a059d11704c1bd1815dcc554834ae0c2d11c433946226015619f>
            networks::BNB => ("0xacAaC3e6D6Df918Bf3c809DFC7d42de0e4a72d4C", 25475700),
            // <https://optimistic.etherscan.io/tx/0x2bb1c3fbf1f370c6e20ecda36b555de1a4426340908055c4274823e31f92210e>
            networks::OPTIMISM => ("0xe2E901AB09f37884BA31622dF3Ca7FC19AA443Be", 72832821),
            // <https://polygonscan.com/tx/0xb189a45eac7ea59c0bb638b5ae6c4c93f9877f31ce826e96b792a9154e7a32a7>
            networks::POLYGON => ("0x7bc6C0E73EDAa66eF3F6E2f27b0EE8661834c6C9", 39037615),
        ]))
        .add_contract(Contract::new("BalancerV2ComposableStablePoolFactoryV4").with_networks(networks![
            // <https://etherscan.io/tx/0x3b61da162f3414c376cfe8b38d57ca6ba3c40b24446029ddab1187f4ae7c2bd7>
            networks::MAINNET => ("0xfADa0f4547AB2de89D1304A668C39B3E09Aa7c76", 16878679),
            // <https://gnosisscan.io/tx/0x2739416da7e44add08bdfb5e4e5a29ca981383b97162748887efcc5c1241b2f1>
            networks::GNOSIS => ("0xD87F44Df0159DC78029AB9CA7D7e57E7249F5ACD", 27056416),
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#deprecated-contracts>
            // <https://sepolia.etherscan.io/tx/0x9313a59ad9a95f2518076cbf4d0dc5f312e0b013a43a7ea4821cae2aa7a50aa2>
            networks::SEPOLIA => ("0xA3fd20E29358c056B727657E83DFd139abBC9924", 3425277),
            networks::ARBITRUM_ONE => ("0x2498A2B0d6462d2260EAC50aE1C3e03F4829BA95", 72235860),
            // <https://snowscan.xyz/tx/0x7b396102e767ec5f2bc06fb2c9d7fb704d0ddc537c04f28cb538c6de7cc4261e>
            networks::AVALANCHE => ("0x3B1eb8EB7b43882b385aB30533D9A2BeF9052a98", 29221425),
            // <https://bscscan.com/tx/0x2819b490b5e04e27d66476730411df8e572bc33038aa869a370ecfa852de0cbf>
            networks::BNB => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 26666380),
            // <https://optimistic.etherscan.io/tx/0x5d6c515442188eb4af83524618333c0fbdab0df809af01c4e7a9e380f1841199>
            networks::OPTIMISM => ("0x1802953277FD955f9a254B80Aa0582f193cF1d77", 82748180),
            // <https://polygonscan.com/tx/0x2cea6a0683e67ebdb7d4a1cf1ad303126c5f228f05f8c9e2ccafdb1f5a024376>
            networks::POLYGON => ("0x6Ab5549bBd766A43aFb687776ad8466F8b42f777", 40613553),
        ]))
        .add_contract(Contract::new("BalancerV2ComposableStablePoolFactoryV5").with_networks(networks![
            // <https://etherscan.io/tx/0x1fc28221925959c0713d04d9f9159255927ebb94b7fa76e4795db0e365643c07>
            networks::MAINNET => ("0xDB8d758BCb971e482B2C45f7F8a7740283A1bd3A", 17672478),
            // <https://gnosisscan.io/tx/0xcbf18b5a0d1f1fca9b30d08ab77d8554567c3bffa7efdd3add273073d20bb1e2>
            networks::GNOSIS => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 28900564),
            // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
            // <https://sepolia.etherscan.io/tx/0x2c155dde7c480929991dd2a3344d9fdd20252f235370d46d0887b151dc0416bd>
            networks::SEPOLIA => ("0xa523f47A933D5020b23629dDf689695AA94612Dc", 3872211),
            networks::ARBITRUM_ONE => ("0xA8920455934Da4D853faac1f94Fe7bEf72943eF1", 110212282),
            // <https://basescan.org/tx/0x1d291ba796b0397d73581b17695cf0e53e61551e419c43d11d81198b00c2bfd3>
            networks::BASE => ("0x8df317a729fcaA260306d7de28888932cb579b88", 1204710),
            // <https://snowscan.xyz/tx/0x000659feb0831fc511f5c2ad12f3b2d466152b753c805fcb06e848701fd1b4b7>
            networks::AVALANCHE => ("0xE42FFA682A26EF8F25891db4882932711D42e467", 32478827),
            // <https://bscscan.com/tx/0x5bdfed936f82800e80543d5212cb287dceebb52c29133838acbe7e148bf1a447>
            networks::BNB => ("0x4fb47126Fa83A8734991E41B942Ac29A3266C968", 29877945),
            // <https://optimistic.etherscan.io/tx/0xa141b35dbbb18154e2452b1ae6ab7d82a6555724a878b5fccff40e18c8ae3484>
            networks::OPTIMISM => ("0x043A2daD730d585C44FB79D2614F295D2d625412", 106752707),
            // <https://polygonscan.com/tx/0xa3d9a1cf00eaca469d6f9ec2fb836bbbfdfbc3b0eeadc07619bb9e695bfdecb8>
            networks::POLYGON => ("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b", 44961548),
        ]))
        .add_contract(Contract::new("BalancerV2ComposableStablePoolFactoryV6").with_networks(networks![
            // <https://etherscan.io/tx/0x4149cadfe5d3431205d9819fca44ed7a4c2b101adc51adc75cc4586dee237be8>
            networks::MAINNET => ("0x5B42eC6D40f7B7965BE5308c70e2603c0281C1E9", 19314764),
            // <https://gnosisscan.io/tx/0xc3fc1fb96712a0659b7e9e5f406f63bdf5cbd5df9e04f0372c28f75785036791>
            networks::GNOSIS => ("0x47B489bf5836f83ABD928C316F8e39bC0587B020", 32650879),
            // <https://sepolia.etherscan.io/tx/0x53aa3587002469b758e2bb87135d9599fd06e7be944fe61c7f82045c45328566>
            networks::SEPOLIA => ("0x05503B3aDE04aCA81c8D6F88eCB73Ba156982D2B", 5369821),
            // <https://arbiscan.io/tx/0xfa1e7642e135fb32dc14c990b851e5e7a0ac7a463e3a60c5003ae4142396f45e>
            networks::ARBITRUM_ONE => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 184805448),
            // <https://basescan.org/tx/0x5d3342faf0368b939daa93247536afa26cc72c83de52ba7711ae1b8646688467>
            networks::BASE => ("0x956CCab09898C0AF2aCa5e6C229c3aD4E93d9288", 11099703),
            // <https://snowscan.xyz/tx/0x246248ad396826dbfbdc5360cb9cbbdb3a672efa08cc745d1670900888c58c7b>
            networks::AVALANCHE => ("0xb9F8AB3ED3F3aCBa64Bc6cd2DcA74B7F38fD7B88", 42186350),
            // <https://bscscan.com/tx/0x6784ab50138c7488bc14d4d9beb6a9e1ddc209a45f0a96b4ee98a7db84167dea>
            networks::BNB => ("0x6B5dA774890Db7B7b96C6f44e6a4b0F657399E2e", 36485719),
            // <https://optimistic.etherscan.io/tx/0xa38b696479f35a9751ca8b1f0ddeb160188b3146113975b8c2b657c2fe7d7fd2>
            networks::OPTIMISM => ("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7", 116694338),
            // <https://polygonscan.com/tx/0x7b9678ad538b1cd3f3a03e63455e7d49a1bc716ea42310fbf99df4bf93ecfdfa>
            networks::POLYGON => ("0xEAedc32a51c510d35ebC11088fD5fF2b47aACF2E", 53996258),
        ]))
        .add_contract(Contract::new("BalancerV2Vault").with_networks(networks![
            // Balancer addresses can be obtained from:
            // <https://github.com/balancer/balancer-subgraph-v2/blob/master/networks.yaml>
            // <https://etherscan.io/tx/0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7>
            networks::MAINNET => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 12272146),
            // <https://gnosisscan.io/tx/0x21947751661e1b9197492f22779af1f5175b71dc7057869e5a8593141d40edf1>
            networks::GNOSIS => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 24821598),
            // <https://sepolia.etherscan.io/tx/0xb22509c6725dd69a975ecb96a0c594901eeee6a279cc66d9d5191022a7039ee6>
            networks::SEPOLIA => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 3418831),
            // <https://arbiscan.io/tx/0xe2c3826bd7b15ef8d338038769fe6140a44f1957a36b0f27ab321ab6c68d5a8e>
            networks::ARBITRUM_ONE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 222832),
            // <https://basescan.org/tx/0x0dc2e3d436424f2f038774805116896d31828c0bf3795a6901337bdec4e0dff6>
            networks::BASE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 1196036),
            // <https://snowscan.xyz/tx/0xc49af0372feb032e0edbba6988410304566b1fd65546c01ced620ac3c934120f>
            networks::AVALANCHE => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 26386141),
            // <https://bscscan.com/tx/0x1de8caa6c54ff9a25600e26d80865d84c9cc4d33c2b98611240529ee7de5cd74>
            networks::BNB => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 22691002),
            // <https://optimistic.etherscan.io/tx/0xa03cb990595df9eed6c5db17a09468cab534aed5f5589a06c0bb3d19dd2f7ce9>
            networks::OPTIMISM => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 7003431),
            // <https://polygonscan.com/tx/0x66f275a2ed102a5b679c0894ced62c4ebcb2a65336d086a916eb83bd1fe5c8d2>
            networks::POLYGON => ("0xBA12222222228d8Ba445958a75a0704d566BF2C8", 15832990),
        ]))
        .add_contract(Contract::new("BalancerV3BatchRouter").with_networks(networks![
            // <https://etherscan.io/tx/0x41cb8619fb92dd532eb09b0e81fd4ce1c6006a10924893f02909e36a317777f3>
            networks::MAINNET => ("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1", 21339510),
            // <https://gnosisscan.io/tx/0xeafddbace9f445266f851ef1d92928e3d01a4622a1a6780b41ac52d5872f12ab>
            networks::GNOSIS => ("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b", 37377506),
            // <https://sepolia.etherscan.io/tx/0x95ed8e1aaaa7bdc5881f3c8fc5a4914a66639bee52987c3a1ea88545083b0681>
            networks::SEPOLIA => ("0xC85b652685567C1B074e8c0D4389f83a2E458b1C", 7219301),
            // <https://arbiscan.io/tx/0xa7968c6bc0775208ffece789c6e5d09b0eea5f2c3ed2806e9bd94fb0b978ff0f>
            networks::ARBITRUM_ONE => ("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E", 297828544),
            // <https://basescan.org/tx/0x47b81146714630ce50445bfa28872a36973acedf785317ca423498810ec8e76c>
            networks::BASE => ("0x85a80afee867aDf27B50BdB7b76DA70f1E853062", 25347205),
            // <https://snowscan.xyz/tx/0x3bfaba7135ee2d67d98f20ee1aa4c8b7e81e47be64223376f3086bab429ac806>
            networks::AVALANCHE => ("0xc9b36096f5201ea332Db35d6D195774ea0D5988f", 59965747),
            // <https://optimistic.etherscan.io/tx/0xf370aab0d652f3e0f7c34e1a53e1afd98e86c487138300b0939d4e54b0088b67>
            networks::OPTIMISM => ("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E", 133969588),
        ]))
        .add_contract(Contract::new("ChainalysisOracle").with_networks(networks![
            networks::MAINNET => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            networks::ARBITRUM_ONE => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            networks::BASE => "0x3A91A31cB3dC49b4db9Ce721F50a9D076c8D739B",
            networks::AVALANCHE => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            networks::BNB => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            networks::OPTIMISM => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
            networks::POLYGON => "0x40C57923924B5c5c5455c48D93317139ADDaC8fb",
        ]))
        .write_formatted(Path::new("artifacts"), false, Path::new("src/bindings"))
        .unwrap();

    generate_contract("ERC20");
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
