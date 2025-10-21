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

crate::bindings!(
    ChainalysisOracle,
    crate::deployments! {
        MAINNET => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        ARBITRUM_ONE => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        BASE => address!("0x3A91A31cB3dC49b4db9Ce721F50a9D076c8D739B"),
        AVALANCHE => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        BNB => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        OPTIMISM => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        POLYGON => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
    }
);

crate::bindings!(
    IZeroex,
    crate::deployments! {
        MAINNET => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        SEPOLIA => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        ARBITRUM_ONE => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        BASE => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        AVALANCHE => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        BNB => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        OPTIMISM => address!("0xdef1abe32c034e558cdd535791643c58a13acc10"),
        POLYGON => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        // Not available on Lens
    }
);

crate::bindings!(ERC20Mintable);

crate::bindings!(GnosisSafe);
crate::bindings!(GnosisSafeCompatibilityFallbackHandler);
crate::bindings!(GnosisSafeProxy);
crate::bindings!(GnosisSafeProxyFactory);

crate::bindings!(BalancerV2BasePool);
crate::bindings!(BalancerV2BasePoolFactory);
crate::bindings!(BalancerV2WeightedPool);
crate::bindings!(BalancerV2StablePool);
crate::bindings!(BalancerV2ComposableStablePool);
crate::bindings!(BalancerV2LiquidityBootstrappingPool);
crate::bindings!(
    BalancerV2WeightedPoolFactory,
    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792>
        MAINNET => (address!("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"), 12272147)
        // Not available on Sepolia (only version ≥ 4)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV2WeightedPoolFactoryV3,
    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0x39f357b78c03954f0bcee2288bf3b223f454816c141ef20399a7bf38057254c4>
        MAINNET => (address!("0x5Dd94Da3644DDD055fcf6B3E1aa310Bb7801EB8b"), 16520627),
        // <https://gnosisscan.io/tx/0x2ac3d873b6f43de6dd77525c7e5b68a8fc3a1dee40303e1b6a680b0285b26091>
        GNOSIS => (address!("0xC128a9954e6c874eA3d62ce62B468bA073093F25"), 26226256),
        // <https://snowscan.xyz/tx/0xdf2c77743cc9287df2022cd6c5f9209ecfecde07371717ab0427d96042a88640>
        AVALANCHE => (address!("0x94f68b54191F62f781Fe8298A8A5Fa3ed772d227"), 26389236),
        // <https://optimistic.etherscan.io/tx/0xc5e79fb00b9a8e2c89b136aae0be098e58f8e832ede13e8079213a75c9cd9c08>
        OPTIMISM => (address!("0xA0DAbEBAAd1b243BBb243f933013d560819eB66f"), 72832703),
        // <https://polygonscan.com/tx/0x2bc079c0e725f43670898b474afedf38462feee72ef8e874a1efcec0736672fc>
        POLYGON => (address!("0x82e4cFaef85b1B6299935340c964C942280327f4"), 39036828),
        // <https://bscscan.com/tx/0x91107b9581e18ec0a4a575d4713bdd7b1fc08656c35522d216307930aa4de7b6>
        BNB => (address!("0x6e4cF292C5349c79cCd66349c3Ed56357dD11B46"), 25474982),
        // Not available on Sepolia (only version ≥ 4)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV2WeightedPoolFactoryV4,
    crate::deployments! {
        // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
        MAINNET => (address!("0x897888115Ada5773E02aA29F775430BFB5F34c51"), 16878323),
        // <https://gnosisscan.io/tx/0xcb6768bd92add227d46668357291e1d67c864769d353f9f0041c59ad2a3b21bf>
        GNOSIS => (address!("0x6CaD2ea22BFA7F4C14Aae92E47F510Cd5C509bc7"), 27055829),
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#pool-factories>
        // <https://sepolia.etherscan.io/tx/0x7dd392b586f1cdecfc635e7dd40ee1444a7836772811e59321fd4873ecfdf3eb>
        SEPOLIA => (address!("0x7920BFa1b2041911b354747CA7A6cDD2dfC50Cfd"), 3424893),
        // <https://arbiscan.io/tx/0x167fe7eb776d1be36b21402d8ae120088c393e28ae7ca0bd1defac84e0f2848b>
        ARBITRUM_ONE => (address!("0xc7E5ED1054A24Ef31D827E6F86caA58B3Bc168d7"), 72222060),
        // <https://basescan.org/tx/0x0732d3a45a3233a134d6e0e72a00ca7a971d82cdc51f71477892ac517bf0d4c9>
        BASE => (address!("0x4C32a8a8fDa4E24139B51b456B42290f51d6A1c4"), 1204869),
        // <https://snowscan.xyz/tx/0xa3fc8aab3b9baba3905045a53e52a47daafe79d4aa26d4fef5c51f3840aa55fa>
        AVALANCHE => (address!("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"), 27739006),
        // <https://optimistic.etherscan.io/tx/0xad915050179db368e43703f3ee1ec55ff5e5e5e0268c15f8839c9f360caf7b0b>
        OPTIMISM => (address!("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"), 82737545),
        // <https://polygonscan.com/tx/0x65e6b13231c2c5656357005a9e419ad6697178ae74eda1ea7522ecdafcf77136>
        OPTIMISM => (address!("0xFc8a407Bba312ac761D8BFe04CE1201904842B76"), 40611103),
        // <https://bscscan.com/tx/0xc7fada60761e3240332c4cbd169633f1828b2a15de23f0148db9d121afebbb4b>
        BNB => (address!("0x230a59F4d9ADc147480f03B0D3fFfeCd56c3289a"), 26665331),
        // Not available on Base and Lens
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
    }
);
crate::bindings!(
    BalancerV2WeightedPool2TokensFactory,
    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0xa5e6d73befaacc6fff0a4b99fd4eaee58f49949bcfb8262d91c78f24667fbfc9>
        MAINNET => (address!("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"), 12349891),
        ARBITRUM_ONE => (address!("0xCF0a32Bbef8F064969F21f7e02328FB577382018"), 222864),
        // <https://optimistic.etherscan.io/tx/0xd5754950d47179d822ea976a8b2af82ffa80e992cf0660b02c0c218359cc8987>
        OPTIMISM => (address!("0xdAE7e32ADc5d490a43cCba1f0c736033F2b4eFca"), 7005512),
        // <https://polygonscan.com/tx/0xb8ac851249cc95bc0943ef0732d28bbd53b0b36c7dd808372666acd8c5f26e1c>
        POLYGON => (address!("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"), 15832998),
        // Not available on Sepolia, Base, Avalanche, BNB and Lens
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
    }
);
crate::bindings!(
    BalancerV2StablePoolFactoryV2,
    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0xef36451947ebd97b72278face57a53806e90071f4c902259db2db41d0c9a143d>
        MAINNET => (address!("0x8df6efec5547e31b0eb7d1291b511ff8a2bf987c"), 14934936),
        // <https://gnosisscan.io/tx/0xe062237f0c8583375b10cf514d091781bfcd52d9ababbd324180770a5efbc6b1>
        GNOSIS => (address!("0xf23b4DB826DbA14c0e857029dfF076b1c0264843"), 25415344),
        ARBITRUM_ONE => (address!("0xEF44D6786b2b4d544b7850Fe67CE6381626Bf2D6"), 14244664),
        // <https://optimistic.etherscan.io/tx/0xcf9f0bd731ded0e513708200df28ac11d17246fb53fc852cddedf590e41c9c03>
        OPTIMISM => (address!("0xeb151668006CD04DAdD098AFd0a82e78F77076c3"), 11088891),
        // <https://polygonscan.com/tx/0xa2c41d014791888a29a9491204446c1b9b2f5dee3f3eb31ad03f290259067b44>
        POLYGON => (address!("0xcA96C4f198d343E251b1a01F3EBA061ef3DA73C1"), 29371951),
        // Not available on Sepolia, Base, Avalanche, BNB and Lens
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
    }
);
crate::bindings!(
    BalancerV2LiquidityBootstrappingPoolFactory,
    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0x665ac1c7c5290d70154d9dfc1d91dc2562b143aaa9e8a77aa13e7053e4fe9b7c>
        MAINNET => (address!("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE"), 12871780),
        ARBITRUM_ONE => (address!("0x142B9666a0a3A30477b052962ddA81547E7029ab"), 222870),
        // <https://polygonscan.com/tx/0xd9b5b9a9e6ea17a87f85574e93577e3646c9c2f9c8f38644f936949e6c853288>
        POLYGON => (address!("0x751A0bC0e3f75b38e01Cf25bFCE7fF36DE1C87DE"), 17116402),
        // Not available on Sepolia, Base, Avalanche, BNB, Optimism and Lens
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
    }
);
crate::bindings!(
    BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory,
    // <https://docs.balancer.fi/reference/contracts/deployment-addresses/mainnet.html#ungrouped-active-current-contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0x298381e567ff6643d9b32e8e7e9ff0f04a80929dce3e004f6fa1a0104b2b69c3>
        MAINNET => (address!("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"), 13730248),
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/gnosis.html#ungrouped-active-current-contracts>
        // <https://gnosis.blockscout.com/tx/0xbd56fefdb27e4ff1c0852e405f78311d6bc2befabaf6c87a405ab19de8c1506a>
        GNOSIS => (address!("0x85a80afee867aDf27B50BdB7b76DA70f1E853062"), 25415236),
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
        // <https://sepolia.etherscan.io/tx/0xe0e8feb509a8aa8a1eaa0b0c4b34395ff2fd880fb854fbeeccc0af1826e395c9>
        SEPOLIA => (address!("0x45fFd460cC6642B8D8Fb12373DFd77Ceb0f4932B"), 25415236),
        ARBITRUM_ONE => (address!("0x1802953277FD955f9a254B80Aa0582f193cF1d77"), 4859669),
        // <https://basescan.org/tx/0x0529de9dbe772f4b4f48da93ae2c2d2c46e3d3221ced9e0c4063a7a5bc47e874>
        BASE => (address!("0x0c6052254551EAe3ECac77B01DFcf1025418828f"), 1206531),
        // <https://snowscan.xyz/tx/0x33a75d83436ae9fcda4b4986713417bf3dc80d9ceb8d2541817846b1ac579d9f>
        AVALANCHE => (address!("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"), 26386552),
        // <https://bscscan.com/tx/0x8b964b97e6091bd41c93002c558d49adc26b8b31d2b30f3a33babbbbe8c55f47>
        BNB => (address!("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"), 22691243),
        // <https://optimistic.etherscan.io/tx/0x14fb43f051eebdec645abf0125e52348dc875b0887b689f8db026d75f9c78dda>
        OPTIMISM => (address!("0xf302f9F50958c5593770FDf4d4812309fF77414f"), 7005915),
        // <https://polygonscan.com/tx/0x125bc007a86d771f8dc8f5fa1017de6e5a11162a458a72f25814503404bbeb0b>
        POLYGON => (address!("0x41B953164995c11C81DA73D212ED8Af25741b7Ac"), 22067480),
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV2ComposableStablePoolFactory,
    crate::deployments! {
        // <https://etherscan.io/tx/0x3b9e93ae050e59b3ca3657958ca30d1fd13fbc43208f8f0aa01ae992294f9961>
        MAINNET => (address!("0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F"), 15485885),
        ARBITRUM_ONE => (address!("0xaEb406b0E430BF5Ea2Dc0B9Fe62E4E53f74B3a33"), 23227044),
        // <https://bscscan.com/tx/0x6c6e1c72c91c75714f698049f1c7b66d8f2baced54e0dd2522dfadff27b5ccf1>
        BNB => (address!("0xf302f9F50958c5593770FDf4d4812309fF77414f"), 22691193),
        // <https://optimistic.etherscan.io/tx/0xad2f330ad865dc7955376a3d9733486b38c53ba0d4757ad4e1b63b105401c506>
        OPTIMISM => (address!("0xf145caFB67081895EE80eB7c04A30Cf87f07b745"), 22182522),
        // <https://polygonscan.com/tx/0xe5d908be686056f1519663a407167c088924f60d29c799ec74438b9de891989e>
        POLYGON => (address!("0x136FD06Fa01eCF624C7F2B3CB15742c1339dC2c4"), 32774224),
        // Not available on Sepolia, Gnosis Chain, Base, Avalanche and Lens
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
    }
);
crate::bindings!(
    BalancerV2ComposableStablePoolFactoryV3,
    crate::deployments! {
        // <https://etherscan.io/tx/0xd8c9ba758cb318beb0c9525b7621280a22b6dfe02cf725a3ece0718598f260ef>
        MAINNET => (address!("0xdba127fBc23fb20F5929C546af220A991b5C6e01"), 16580899),
        // <https://gnosisscan.io/tx/0x2abd7c865f8ab432b340f7de897192c677ffa254908fdec14091e0cd06962963>
        GNOSIS => (address!("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"), 26365805),
        ARBITRUM_ONE => (address!("0x1c99324EDC771c82A0DCCB780CC7DDA0045E50e7"), 58948370),
        // <https://bscscan.com/tx/0xfe0c47c2b124a059d11704c1bd1815dcc554834ae0c2d11c433946226015619f>
        BNB => (address!("0xacAaC3e6D6Df918Bf3c809DFC7d42de0e4a72d4C"), 25475700),
        // <https://optimistic.etherscan.io/tx/0x2bb1c3fbf1f370c6e20ecda36b555de1a4426340908055c4274823e31f92210e>
        OPTIMISM => (address!("0xe2E901AB09f37884BA31622dF3Ca7FC19AA443Be"), 72832821),
        // <https://polygonscan.com/tx/0xb189a45eac7ea59c0bb638b5ae6c4c93f9877f31ce826e96b792a9154e7a32a7>
        POLYGON => (address!("0x7bc6C0E73EDAa66eF3F6E2f27b0EE8661834c6C9"), 39037615),
        // Not available on Sepolia (only version ≥ 4) and on Base (only version ≥ 5)
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html>
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV2ComposableStablePoolFactoryV4,
    crate::deployments! {
        // <https://etherscan.io/tx/0x3b61da162f3414c376cfe8b38d57ca6ba3c40b24446029ddab1187f4ae7c2bd7>
        MAINNET => (address!("0xfADa0f4547AB2de89D1304A668C39B3E09Aa7c76"), 16878679),
        // <https://gnosisscan.io/tx/0x2739416da7e44add08bdfb5e4e5a29ca981383b97162748887efcc5c1241b2f1>
        GNOSIS => (address!("0xD87F44Df0159DC78029AB9CA7D7e57E7249F5ACD"), 27056416),
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#deprecated-contracts>
        // <https://sepolia.etherscan.io/tx/0x9313a59ad9a95f2518076cbf4d0dc5f312e0b013a43a7ea4821cae2aa7a50aa2>
        SEPOLIA => (address!("0xA3fd20E29358c056B727657E83DFd139abBC9924"), 3425277),
        ARBITRUM_ONE => (address!("0x2498A2B0d6462d2260EAC50aE1C3e03F4829BA95"), 72235860),
        // <https://snowscan.xyz/tx/0x7b396102e767ec5f2bc06fb2c9d7fb704d0ddc537c04f28cb538c6de7cc4261e>
        AVALANCHE => (address!("0x3B1eb8EB7b43882b385aB30533D9A2BeF9052a98"), 29221425),
        // <https://bscscan.com/tx/0x2819b490b5e04e27d66476730411df8e572bc33038aa869a370ecfa852de0cbf>
        BNB => (address!("0x1802953277FD955f9a254B80Aa0582f193cF1d77"), 26666380),
        // <https://optimistic.etherscan.io/tx/0x5d6c515442188eb4af83524618333c0fbdab0df809af01c4e7a9e380f1841199>
        OPTIMISM => (address!("0x1802953277FD955f9a254B80Aa0582f193cF1d77"), 82748180),
        // <https://polygonscan.com/tx/0x2cea6a0683e67ebdb7d4a1cf1ad303126c5f228f05f8c9e2ccafdb1f5a024376>
        POLYGON => (address!("0x6Ab5549bBd766A43aFb687776ad8466F8b42f777"), 40613553),
        // Not available on Base and Lens
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/base.html>
    }
);
crate::bindings!(
    BalancerV2ComposableStablePoolFactoryV5,
    crate::deployments! {
        // <https://etherscan.io/tx/0x1fc28221925959c0713d04d9f9159255927ebb94b7fa76e4795db0e365643c07>
        MAINNET => (address!("0xDB8d758BCb971e482B2C45f7F8a7740283A1bd3A"), 17672478),
        // <https://gnosisscan.io/tx/0xcbf18b5a0d1f1fca9b30d08ab77d8554567c3bffa7efdd3add273073d20bb1e2>
        GNOSIS => (address!("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"), 28900564),
        // <https://docs.balancer.fi/reference/contracts/deployment-addresses/sepolia.html#ungrouped-active-current-contracts>
        // <https://sepolia.etherscan.io/tx/0x2c155dde7c480929991dd2a3344d9fdd20252f235370d46d0887b151dc0416bd>
        SEPOLIA => (address!("0xa523f47A933D5020b23629dDf689695AA94612Dc"), 3872211),
        ARBITRUM_ONE => (address!("0xA8920455934Da4D853faac1f94Fe7bEf72943eF1"), 110212282),
        // <https://basescan.org/tx/0x1d291ba796b0397d73581b17695cf0e53e61551e419c43d11d81198b00c2bfd3>
        BASE => (address!("0x8df317a729fcaA260306d7de28888932cb579b88"), 1204710),
        // <https://snowscan.xyz/tx/0x000659feb0831fc511f5c2ad12f3b2d466152b753c805fcb06e848701fd1b4b7>
        AVALANCHE => (address!("0xE42FFA682A26EF8F25891db4882932711D42e467"), 32478827),
        // <https://bscscan.com/tx/0x5bdfed936f82800e80543d5212cb287dceebb52c29133838acbe7e148bf1a447>
        BNB => (address!("0x4fb47126Fa83A8734991E41B942Ac29A3266C968"), 29877945),
        // <https://optimistic.etherscan.io/tx/0xa141b35dbbb18154e2452b1ae6ab7d82a6555724a878b5fccff40e18c8ae3484>
        OPTIMISM => (address!("0x043A2daD730d585C44FB79D2614F295D2d625412"), 106752707),
        // <https://polygonscan.com/tx/0xa3d9a1cf00eaca469d6f9ec2fb836bbbfdfbc3b0eeadc07619bb9e695bfdecb8>
        POLYGON => (address!("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b"), 44961548),
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV2ComposableStablePoolFactoryV6,
    crate::deployments! {
        // <https://etherscan.io/tx/0x4149cadfe5d3431205d9819fca44ed7a4c2b101adc51adc75cc4586dee237be8>
        MAINNET => (address!("0x5B42eC6D40f7B7965BE5308c70e2603c0281C1E9"), 19314764),
        // <https://gnosisscan.io/tx/0xc3fc1fb96712a0659b7e9e5f406f63bdf5cbd5df9e04f0372c28f75785036791>
        GNOSIS => (address!("0x47B489bf5836f83ABD928C316F8e39bC0587B020"), 32650879),
        // <https://sepolia.etherscan.io/tx/0x53aa3587002469b758e2bb87135d9599fd06e7be944fe61c7f82045c45328566>
        SEPOLIA => (address!("0x05503B3aDE04aCA81c8D6F88eCB73Ba156982D2B"), 5369821),
        // <https://arbiscan.io/tx/0xfa1e7642e135fb32dc14c990b851e5e7a0ac7a463e3a60c5003ae4142396f45e>
        ARBITRUM_ONE => (address!("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"), 184805448),
        // <https://basescan.org/tx/0x5d3342faf0368b939daa93247536afa26cc72c83de52ba7711ae1b8646688467>
        BASE => (address!("0x956CCab09898C0AF2aCa5e6C229c3aD4E93d9288"), 11099703),
        // <https://snowscan.xyz/tx/0x246248ad396826dbfbdc5360cb9cbbdb3a672efa08cc745d1670900888c58c7b>
        AVALANCHE => (address!("0xb9F8AB3ED3F3aCBa64Bc6cd2DcA74B7F38fD7B88"), 42186350),
        // <https://bscscan.com/tx/0x6784ab50138c7488bc14d4d9beb6a9e1ddc209a45f0a96b4ee98a7db84167dea>
        BNB => (address!("0x6B5dA774890Db7B7b96C6f44e6a4b0F657399E2e"), 36485719),
        // <https://optimistic.etherscan.io/tx/0xa38b696479f35a9751ca8b1f0ddeb160188b3146113975b8c2b657c2fe7d7fd2>
        OPTIMISM => (address!("0x4bdCc2fb18AEb9e2d281b0278D946445070EAda7"), 116694338),
        // <https://polygonscan.com/tx/0x7b9678ad538b1cd3f3a03e63455e7d49a1bc716ea42310fbf99df4bf93ecfdfa>
        POLYGON => (address!("0xEAedc32a51c510d35ebC11088fD5fF2b47aACF2E"), 53996258),
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV2Vault,
    crate::deployments! {
        // <https://etherscan.io/tx/0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7>
        MAINNET => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 12272146),
        // <https://gnosisscan.io/tx/0x21947751661e1b9197492f22779af1f5175b71dc7057869e5a8593141d40edf1>
        GNOSIS => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 24821598),
        // <https://sepolia.etherscan.io/tx/0xb22509c6725dd69a975ecb96a0c594901eeee6a279cc66d9d5191022a7039ee6>
        SEPOLIA => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 3418831),
        // <https://arbiscan.io/tx/0xe2c3826bd7b15ef8d338038769fe6140a44f1957a36b0f27ab321ab6c68d5a8e>
        ARBITRUM_ONE => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 222832),
        // <https://basescan.org/tx/0x0dc2e3d436424f2f038774805116896d31828c0bf3795a6901337bdec4e0dff6>
        BASE => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 1196036),
        // <https://snowscan.xyz/tx/0xc49af0372feb032e0edbba6988410304566b1fd65546c01ced620ac3c934120f>
        AVALANCHE => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 26386141),
        // <https://bscscan.com/tx/0x1de8caa6c54ff9a25600e26d80865d84c9cc4d33c2b98611240529ee7de5cd74>
        BNB => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 22691002),
        // <https://optimistic.etherscan.io/tx/0xa03cb990595df9eed6c5db17a09468cab534aed5f5589a06c0bb3d19dd2f7ce9>
        OPTIMISM => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 7003431),
        // <https://polygonscan.com/tx/0x66f275a2ed102a5b679c0894ced62c4ebcb2a65336d086a916eb83bd1fe5c8d2>
        POLYGON => (address!("0xBA12222222228d8Ba445958a75a0704d566BF2C8"), 15832990),
        // Not available on Lens
    }
);
crate::bindings!(
    BalancerV3BatchRouter,
    crate::deployments! {
        // <https://etherscan.io/tx/0x41cb8619fb92dd532eb09b0e81fd4ce1c6006a10924893f02909e36a317777f3>
        MAINNET => (address!("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1"), 21339510),
        // <https://gnosisscan.io/tx/0xeafddbace9f445266f851ef1d92928e3d01a4622a1a6780b41ac52d5872f12ab>
        GNOSIS => (address!("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b"), 37377506),
        // <https://sepolia.etherscan.io/tx/0x95ed8e1aaaa7bdc5881f3c8fc5a4914a66639bee52987c3a1ea88545083b0681>
        SEPOLIA => (address!("0xC85b652685567C1B074e8c0D4389f83a2E458b1C"), 7219301),
        // <https://arbiscan.io/tx/0xa7968c6bc0775208ffece789c6e5d09b0eea5f2c3ed2806e9bd94fb0b978ff0f>
        ARBITRUM_ONE => (address!("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E"), 297828544),
        // <https://basescan.org/tx/0x47b81146714630ce50445bfa28872a36973acedf785317ca423498810ec8e76c>
        BASE => (address!("0x85a80afee867aDf27B50BdB7b76DA70f1E853062"), 25347205),
        // <https://snowscan.xyz/tx/0x3bfaba7135ee2d67d98f20ee1aa4c8b7e81e47be64223376f3086bab429ac806>
        AVALANCHE => (address!("0xc9b36096f5201ea332Db35d6D195774ea0D5988f"), 59965747),
        // <https://optimistic.etherscan.io/tx/0xf370aab0d652f3e0f7c34e1a53e1afd98e86c487138300b0939d4e54b0088b67>
        OPTIMISM => (address!("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E"), 133969588),
        // Not available on Lens, Polygon, BNB
    }
);

// UniV2
crate::bindings!(
    BaoswapRouter,
    crate::deployments! {
       // https://gnosisscan.io/tx/0xdcbfa037f2c6c7456022df0632ec8d6a75d0f9a195238eec679d5d26895eb7b1
       GNOSIS => (address!("0x6093AeBAC87d62b1A5a4cEec91204e35020E38bE"))
    }
);
crate::bindings!(
    HoneyswapRouter,
    crate::deployments! {
        GNOSIS => (address!("0x1C232F01118CB8B424793ae03F870aa7D0ac7f77"))
    }
);
crate::bindings!(
    PancakeRouter,
    crate::deployments! {
        // <https://etherscan.io/tx/0x6e441248a9835ca10a3c29a19f2e1ed61d2e35f3ecb3a5b9e4ee170d62a22d16>
        MAINNET => (address!("0xEfF92A263d31888d860bD50809A8D171709b7b1c")),
        // <https://arbiscan.io/tx/0x4a2da73cbfcaafb0347e4525307a095e38bf7532435cb0327d1f5ee2ee15a011>
        ARBITRUM_ONE => (address!("0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb")),
        // <https://basescan.org/tx/0xda322aef5776698ac6da56be1ffaa0f9994a983cdeb9f2aeaba47437809ae6ef>
        BASE => (address!("0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb")),
        // <https://bscscan.com/tx/0x1bfbff8411ed44e609d911476b0d35a28284545b690902806ea0a7ff0453e931>
        BNB => (address!("0x10ED43C718714eb63d5aA57B78B54704E256024E"))
    }
);
crate::bindings!(
    SushiSwapRouter,
    // <https://docs.sushi.com/contracts/cpamm>
    crate::deployments! {
        // <https://etherscan.io/tx/0x4ff39eceee7ba9a63736eae38be69b10347975ff5fa4d9b85743a51e1c384094>
        MAINNET => (address!("0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f")),
        // <https://gnosisscan.io/tx/0x8b45ccbc2afd0132ef8b636064e0e745ff93b53942a56e320bb930666dd0fb18>
        GNOSIS => (address!("0x1b02da8cb0d097eb8d57a175b88c7d8b47997506")),
        // <https://arbiscan.io/tx/0x40b22402bcac46330149ac9848f8bddd02b0a1e79d4a71934655a634051be1a1>
        ARBITRUM_ONE => (address!("0x1b02da8cb0d097eb8d57a175b88c7d8b47997506")),
        // <https://basescan.org/tx/0xbb673c483292e03d202e95a023048b8bda459bf12402e7688f7e10be8b4dc67d>
        BASE => (address!("0x6bded42c6da8fbf0d2ba55b2fa120c5e0c8d7891")),
        // <https://snowtrace.io/tx/0x8185bcd3cc8544f8767e5270c4d7eb1e9b170fc0532fc4f0d7e7a1018e1f13ba>
        AVALANCHE => (address!("0x1b02da8cb0d097eb8d57a175b88c7d8b47997506")),
        // <https://bscscan.com/tx/0xf22f827ae797390f6e478b0a11aa6e92d6da527f47130ef70d313ff0e0b2a83f>
        BNB => (address!("0x1b02da8cb0d097eb8d57a175b88c7d8b47997506")),
        // <https://optimistic.etherscan.io/tx/0x88be6cc83f5bfccb8196db351866bac5c99ab8f7b451ea9975319ba05c3bf8f7>
        OPTIMISM => (address!("0x2abf469074dc0b54d793850807e6eb5faf2625b1")),
        // <https://polygonscan.com/tx/0x3dcf8fc780ae6fbe40b1ae57927a8fb405f54cbe89d0021a781a100d2086e5ba>
        POLYGON => (address!("0x1b02da8cb0d097eb8d57a175b88c7d8b47997506")),
        // Not available on Lens
    }
);
crate::bindings!(
    SwaprRouter,
    // <https://swapr.gitbook.io/swapr/contracts>
    crate::deployments! {
        // <https://etherscan.io/tx/0x3f4ccc676637906db24caf043c89dafce959321c02266c6a4ab706fcec79a5f7>
        MAINNET => address!("0xb9960d9bca016e9748be75dd52f02188b9d0829f"),
        // <https://gnosisscan.io/tx/0x0406e774caced468b8f84d7c7ed9b6e9c324601af38f44e385aecf7a7d01feb4>
        GNOSIS => address!("0xE43e60736b1cb4a75ad25240E2f9a62Bff65c0C0"),
        // <https://arbiscan.io/tx/0x09771774fc138775472910f6bb0f2e03ff74e1e32a658e9c3e4d8f59f6431ba8>
        ARBITRUM_ONE => address!("0x530476d5583724A89c8841eB6Da76E7Af4C0F17E"),
        // Not available on Base and Lens
    }
);
crate::bindings!(ISwaprPair);
crate::bindings!(
    TestnetUniswapV2Router02,
    crate::deployments! {
        // <https://sepolia.etherscan.io/tx/0x2bf9a91a42d53e161897d9c581f798df9db6fb00587803dde7e7b8859118d821>
        SEPOLIA => address!("0x86dcd3293C53Cf8EFd7303B57beb2a3F671dDE98"),
    }
);
crate::bindings!(
    UniswapV2Factory,
    // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/factory>
    crate::deployments! {
        // <https://etherscan.io/tx/0xc31d7e7e85cab1d38ce1b8ac17e821ccd47dbde00f9d57f2bd8613bff9428396>
        MAINNET => address!("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
        // <https://gnosisscan.io/tx/0x446de52c460bed3f49a4342eab247bb4b2fe2993962c284fb9bc14a983c7a3d4>
        GNOSIS => address!("0xA818b4F111Ccac7AA31D0BCc0806d64F2E0737D7"),
        // <https://arbiscan.io/tx/0x83b597d54496c0b64d66a3b9a65c312e406262511c908f702ef06755d13ab2f3>
        ARBITRUM_ONE => address!("0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9"),
        // <https://basescan.org/tx/0x3c94031f81d9afe3beeb8fbcf4dcf1bd5b5688b86081d94e3d0231514dc00d31>
        BASE => address!("0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6"),
        // <https://sepolia.etherscan.io/tx/0x0a5e26b22f6b470857957a1d5a92ad4a7d3c5e7cf254ddd80edfe23df70eae71>
        SEPOLIA => address!("0xF62c03E08ada871A0bEb309762E260a7a6a880E6"),
        // <https://snowtrace.io/tx/0xd06a069b11fc0c998b404c5736957cc16c71cf1f7dbf8a7d4244c84036ea6edd>
        AVALANCHE => address!("0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C"),
        // <https://bscscan.com/tx/0x7305a4bddc54eee158f245a09526969697ac1a9f56d090b124ebfc85ff71a5cf>
        BNB => address!("0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6"),
        // <https://optimistic.etherscan.io/tx/0xf7227dcbbfa4ea2bb2634f2a1f364a64b028f9e9e393974fea8d435cd097c72e>
        OPTIMISM => address!("0x0c3c1c532F1e39EdF36BE9Fe0bE1410313E074Bf"),
        // <https://polygonscan.com/tx/0x712ac56155a301fca4b7a761e232233f41a104865a74b1a59293835da355292a>
        POLYGON => address!("0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C"),
        // Not available on Lens
    }
);
crate::bindings!(
    UniswapV2Router02,
    // <https://docs.uniswap.org/contracts/v2/reference/smart-contracts/router-02>
    crate::deployments! {
        // <https://etherscan.io/tx/0x4fc1580e7f66c58b7c26881cce0aab9c3509afe6e507527f30566fbf8039bcd0>
        MAINNET => address!("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        // <https://gnosisscan.io/tx/0xfcc495cdb313b48bbb0cd0a25cb2e8fd512eb8fb0b15f75947a9d5668e47a918>
        GNOSIS => address!("0x1C232F01118CB8B424793ae03F870aa7D0ac7f77"),
        // <https://arbiscan.io/tx/0x630cd9d56a85e1bac7795d254fef861304a6838e28869badef19f19defb48ba6>
        ARBITRUM_ONE => address!("0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
        // <https://basescan.org/tx/0x039224ce16ebe5574f51da761acbdfbd21099d6230c39fcd8ff566bbfd6a50a9>
        BASE => address!("0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
        // <https://sepolia.etherscan.io/tx/0x92674b51681d2e99e71e03bd387bc0f0e79f2412302b49ed5626d1fa2311bab9>
        SEPOLIA => address!("0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3"),
        // <https://snowtrace.io/tx/0x7372f1eedf9d32fb4185d486911f44542723dae766eea04bc3f14724bae9552e>
        AVALANCHE => address!("0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
        // <https://bscscan.com/tx/0x9e940f846abea7dcc1f0bd5c261f405c104628c855346f8cac966f52905ee0fa>
        BNB => address!("0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24"),
        // <https://optimistic.etherscan.io/tx/0x2dcb9a76100e5be49e89085b87bd447b1966a9d823d5985e1a8197834c60e6bd>
        OPTIMISM => address!("0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2"),
        // <https://polygonscan.com/tx/0x66186e0cacd2f6b3ad2eae586bd331daafd0572eb80bf71be694181858198025>
        POLYGON => address!("0xedf6066a2b290C185783862C7F4776A2C8077AD1"),
        // Not available on Lens
    }
);
crate::bindings!(IUniswapLikeRouter);
crate::bindings!(IUniswapLikePair);
crate::bindings!(UniswapV3Pool);

crate::bindings!(
    HooksTrampoline,
    // <https://github.com/cowprotocol/hooks-trampoline/blob/993427166ade6c65875b932f853776299290ac4b/networks.json>
    crate::deployments! {
        MAINNET  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        // Gnosis is using the old instance of the hook trampoline since it's hardcoded in gnosis pay rebalance integration.
        GNOSIS  => address!("0x01DcB88678aedD0C4cC9552B20F4718550250574"),
        SEPOLIA  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        ARBITRUM_ONE  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        BASE  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        AVALANCHE  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        BNB  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        OPTIMISM  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        POLYGON  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
        LENS  => address!("0x60Bf78233f48eC42eE3F101b9a05eC7878728006"),
    }
);

crate::bindings!(
    CoWSwapEthFlow,
    crate::deployments! {
        // <https://etherscan.io/tx/0x0247e3c15f59a52b099f192265f1c1e6227f48a280717b3eefd7a5d9d0c051a1>
        MAINNET => (address!("0x40a50cf069e992aa4536211b23f286ef88752187"), 16169866),
        // <https://gnosisscan.io/tx/0x6280e079f454fbb5de3c52beddd64ca2b5be0a4b3ec74edfd5f47e118347d4fb>
        GNOSIS => (address!("0x40a50cf069e992aa4536211b23f286ef88752187"), 25414331),
        // <https://github.com/cowprotocol/ethflowcontract/blob/v1.1.0-artifacts/networks.prod.json#L11-L14>
        // <https://sepolia.etherscan.io/tx/0x558a7608a770b5c4f68fffa9b02e7908a40f61b557b435ea768a4c62cb79ae25>
        SEPOLIA => (address!("0x0b7795E18767259CC253a2dF471db34c72B49516"), 4718739),
        // <https://arbiscan.io/tx/0xa4066ca77bbe1f21776b4c26315ead3b1c054b35814b49e0c35afcbff23e1b8d>
        ARBITRUM_ONE => (address!("0x6DFE75B5ddce1ADE279D4fa6BD6AeF3cBb6f49dB"), 204747458),
        // <https://basescan.org/tx/0xc3555c4b065867cbf34382438e1bbaf8ee39eaf10fb0c70940c8955962e76e2c>
        BASE => (address!("0x3C3eA1829891BC9bEC3d06A81d5d169e52a415e3"), 21490258),
        // <https://snowscan.xyz/tx/0x71a2ed9754247210786effa3269bc6eb68b7521b5052ac9f205af7ac364f608f>
        AVALANCHE => (address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"), 60496408),
        // <https://bscscan.com/tx/0x959a60a42d36e0efd247b3cf19ed9d6da503d01bce6f87ed31e5e5921111222e>
        BNB => (address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"), 48411237),
        // <https://optimistic.etherscan.io/tx/0x0644f10f7ae5448240fc592ad21abf4dabac473a9d80904af5f7865f2d6509e2>
        OPTIMISM => (address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"), 134607215),
        // <https://polygonscan.com/tx/0xc3781c19674d97623d13afc938fca94d53583f4051020512100e84fecd230f91>
        POLYGON => (address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"), 71296258),
        // <https://explorer.lens.xyz/tx/0xc59b5ffadb40158f9390b1d77f19346dbe9214b27f26346dfa2990ad379a1a32>
        LENS => (address!("0xFb337f8a725A142f65fb9ff4902d41cc901de222"), 3007173),
    }
);
crate::bindings!(CoWSwapOnchainOrders);

// Used in the gnosis/solvers repo for the balancer solver
crate::bindings!(
    BalancerQueries,
    crate::deployments! {
        // <https://etherscan.io/tx/0x30799534f3a0ab8c7fa492b88b56e9354152ffaddad15415184a3926c0dd9b09>
        MAINNET => (address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"), 15188261),
        // <https://arbiscan.io/tx/0x710d93aab52b6c10197eab20f9d6db1af3931f9890233d8832268291ef2f54b3>
        ARBITRUM_ONE => (address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"), 18238624),
        // <https://optimistic.etherscan.io/tx/0xf3b2aaf3e12c7de0987dc99a26242b227b9bc055342dda2e013dab0657d6f9f1>
        OPTIMISM => (address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"), 15288107),
        // <https://basescan.org/tx/0x425d04ee79511c17d06cd96fe1df9e0727f7e7d46b31f36ecaa044ada6a0d29a>
        BASE => (address!("0x300Ab2038EAc391f26D9F895dc61F8F66a548833"), 1205869),
        // <https://gnosisscan.io/tx/0x5beb3051d393aac24cb236dc850c644f345af65c4927030bd1033403e2f2e503>
        GNOSIS => (address!("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"), 24821845),
        // <https://polygonscan.com/tx/0x0b74f5c230f9b7df8c7a7f0d1ebd5e6c3fab51a67a9bcc8f05c350180041682e>
        POLYGON => (address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"), 30988035),
        // <https://snowtrace.io/tx/0xf484e1efde47209bad5f72642bcb8d8e2a4092a5036434724ffa2d039e93a1bf?chainid=43114>
        AVALANCHE => (address!("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"), 26387068),
        // Not available on Lens
    }
);

crate::bindings!(
    ILiquoriceSettlement,
    crate::deployments! {
        // <https://liquorice.gitbook.io/liquorice-docs/links/smart-contracts>
        MAINNET => address!("0x0448633eb8B0A42EfED924C42069E0DcF08fb552"),
        ARBITRUM_ONE => address!("0x0448633eb8B0A42EfED924C42069E0DcF08fb552"),
    }
);

crate::bindings!(
    FlashLoanRouter,
    crate::deployments! {
        MAINNET => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
        GNOSIS => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
        SEPOLIA => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
        ARBITRUM_ONE => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
        BASE => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
        POLYGON => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
        AVALANCHE => address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
    }
);

pub mod support {
    // Support contracts used for trade and token simulations.
    crate::bindings!(AnyoneAuthenticator);
    crate::bindings!(Solver);
    crate::bindings!(Spardose);
    crate::bindings!(Trader);
    // Support contract used for solver fee simulations in the gnosis/solvers repo.
    crate::bindings!(Swapper);
    crate::bindings!(
        Signatures,
        crate::deployments! {
            MAINNET => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            ARBITRUM_ONE => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            BASE => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            AVALANCHE => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            BNB => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            OPTIMISM => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            POLYGON => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            LENS => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            GNOSIS => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
            SEPOLIA => address!("0x8262d639c38470F38d2eff15926F7071c28057Af"),
        }
    );
    // Support contracts used for various order simulations.
    crate::bindings!(
        Balances,
        crate::deployments! {
            MAINNET => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            ARBITRUM_ONE => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            BASE => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            AVALANCHE => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            BNB => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            OPTIMISM => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            POLYGON => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            LENS => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            GNOSIS => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            SEPOLIA => address!("0x3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
        }
    );
}

pub mod test {
    // Test Contract for using up a specified amount of gas.
    crate::bindings!(GasHog);
    // Test Contract for incrementing arbitrary counters.
    crate::bindings!(Counter);
}

pub use alloy::providers::DynProvider as Provider;

/// Extension trait to attach some useful functions to the contract instance.
pub trait InstanceExt: Sized {
    /// Crates a contract instance at the expected address for the current
    /// network.
    fn deployed(
        provider: &Provider,
    ) -> impl std::future::Future<Output = anyhow::Result<Self>> + Send;

    /// Returns the block number at which the contract was deployed, if known.
    fn deployed_block(
        &self,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<u64>>> + Send;
}

/// Build a `HashMap<u64, (Address, Option<u64>)>` from entries like:
/// `CHAIN_ID => address!("0x…")`                // block = None
/// `CHAIN_ID => (address!("0x…"), 12_345_678)`  // block = Some(…)
#[macro_export]
macro_rules! deployments {
    (@acc $m:ident; ) => {};

    // Tuple form with trailing comma: CHAIN => (addr, block),
    (@acc $m:ident; $chain:expr => ( $addr:expr, $block:expr ), $($rest:tt)* ) => {
        $m.insert($chain, ($addr, Some($block)));
        $crate::deployments!(@acc $m; $($rest)*);
    };

    // Address-only form with trailing comma: CHAIN => addr,
    (@acc $m:ident; $chain:expr => $addr:expr, $($rest:tt)* ) => {
        $m.insert($chain, ($addr, None::<u64>));
        $crate::deployments!(@acc $m; $($rest)*);
    };

    // Tuple form without trailing comma (last entry).
    (@acc $m:ident; $chain:expr => ( $addr:expr, $block:expr ) ) => {
        $m.insert($chain, ($addr, Some($block)));
    };

    // Address-only form without trailing comma (last entry).
    (@acc $m:ident; $chain:expr => $addr:expr ) => {
        $m.insert($chain, ($addr, None::<u64>));
    };

    ( $($rest:tt)* ) => {{
        let mut m = ::std::collections::HashMap::new();
        $crate::deployments!(@acc m; $($rest)*);
        m
    }};
}

#[macro_export]
macro_rules! bindings {
    ($contract:ident $(, $deployment_info:expr)?) => {
        paste::paste! {
            // Generate the main bindings in a private module. That allows
            // us to re-export all items in our own module while also adding
            // some items ourselves.
            #[allow(non_snake_case)]
            mod [<$contract Private>] {
                alloy::sol!(
                    #[allow(missing_docs, clippy::too_many_arguments)]
                    #[sol(rpc, all_derives)]
                    $contract,
                    concat!("./artifacts/", stringify!($contract), ".json"),
                );
            }

            #[allow(non_snake_case)]
            pub mod $contract {
                use alloy::providers::DynProvider;

                pub use super::[<$contract Private>]::*;
                pub type Instance = $contract::[<$contract Instance>]<DynProvider>;

                $(
                use {
                    std::sync::LazyLock,
                    anyhow::Result,
                    std::collections::HashMap,
                    alloy::{
                        providers::Provider,
                        primitives::{address, Address},
                    },
                    anyhow::Context,
                    $crate::alloy::networks::*,
                };

                static DEPLOYMENT_INFO: LazyLock<HashMap<u64, (Address, Option<u64>)>> = LazyLock::new(|| {
                    $deployment_info
                });

                /// Returns the contract's deployment address (if one exists) for the given chain.
                pub fn deployment_address(chain_id: &u64) -> Option<alloy::primitives::Address> {
                    DEPLOYMENT_INFO.get(chain_id).map(|(addr, _)| *addr)
                }

                impl $crate::alloy::InstanceExt for Instance {
                    fn deployed(provider: &DynProvider) -> impl Future<Output = Result<Self>> + Send {
                        async move {
                            let chain_id = provider
                                .get_chain_id()
                                .await
                                .context("could not fetch current chain id")?;

                            let (address, _deployed_block) = *DEPLOYMENT_INFO
                                .get(&chain_id)
                                .with_context(|| format!("no deployment info for chain {chain_id:?}"))?;

                            Ok(Instance::new(
                                address,
                                provider.clone(),
                            ))
                        }
                    }

                    fn deployed_block(&self) -> impl Future<Output = Result<Option<u64>>> + Send {
                        async move {
                            let chain_id = self
                                .provider()
                                .get_chain_id()
                                .await
                                .context("could not fetch current chain id")?;
                            if let Some((_address, deployed_block)) = DEPLOYMENT_INFO.get(&chain_id) {
                                return Ok(*deployed_block);
                            }
                            Ok(None)
                        }
                    }
                }
                )*
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::networks::*;
    use super::*;

    #[test]
    fn test_has_address() {
        assert!(BaoswapRouter::deployment_address(&GNOSIS).is_some());
        assert!(HoneyswapRouter::deployment_address(&GNOSIS).is_some());

        for chain_id in &[MAINNET, ARBITRUM_ONE, BASE, BNB] {
            assert!(PancakeRouter::deployment_address(chain_id).is_some());
        }

        for chain_id in &[
            MAINNET,
            GNOSIS,
            ARBITRUM_ONE,
            BASE,
            AVALANCHE,
            BNB,
            OPTIMISM,
            POLYGON,
        ] {
            assert!(SushiSwapRouter::deployment_address(chain_id).is_some());
        }

        for chain_id in &[MAINNET, GNOSIS, ARBITRUM_ONE] {
            assert!(SwaprRouter::deployment_address(chain_id).is_some());
        }

        assert!(TestnetUniswapV2Router02::deployment_address(&SEPOLIA).is_some());

        for chain_id in &[
            MAINNET,
            GNOSIS,
            ARBITRUM_ONE,
            BASE,
            SEPOLIA,
            AVALANCHE,
            BNB,
            OPTIMISM,
            POLYGON,
        ] {
            assert!(UniswapV2Factory::deployment_address(chain_id).is_some());
            assert!(UniswapV2Router02::deployment_address(chain_id).is_some());
        }
    }
}
