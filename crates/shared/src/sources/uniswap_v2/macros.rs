//! Macros for implementing Uniswap-like liquidity.

macro_rules! impl_uniswap_like_liquidity {
    (
        factory: $factory:ty,
        init_code_digest: $init_code:literal,
    ) => {
        pub const INIT_CODE_DIGEST: [u8; 32] = ::hex_literal::hex!($init_code);

        /// Creates the pair provider for the specified Web3 instance.
        pub async fn get_pair_provider(
            web3: &$crate::Web3,
        ) -> ::anyhow::Result<$crate::sources::uniswap_v2::pair_provider::PairProvider> {
            let factory = <$factory>::deployed(web3).await?;
            Ok($crate::sources::uniswap_v2::pair_provider::PairProvider {
                factory: factory.address(),
                init_code_digest: INIT_CODE_DIGEST,
            })
        }
    };
}

pub(crate) use impl_uniswap_like_liquidity;
