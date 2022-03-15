//! Macros for implementing Uniswap-like liquidity.

macro_rules! impl_uniswap_like_liquidity {
    (
        factory: $factory:ty,
        init_code_digest: $init_code:literal,
    ) => {
        impl_uniswap_like_liquidity!(
            factory: $factory,
            init_code_digest: $init_code,
            pool_reader: $crate::sources::uniswap_v2::pool_fetching::DefaultPoolReader,
        );
    };
    (
        factory: $factory:ty,
        init_code_digest: $init_code:literal,
        pool_reader: $pool_reader:ty,
    ) => {
        pub const INIT_CODE_DIGEST: [u8; 32] = ::hex_literal::hex!($init_code);

        /// Creates the pair provider and pool fetcher for the specified Web3
        /// instance.
        pub async fn get_liquidity_source(
            web3: &$crate::Web3,
        ) -> ::anyhow::Result<(
            $crate::sources::uniswap_v2::pair_provider::PairProvider,
            ::std::sync::Arc<dyn $crate::sources::uniswap_v2::pool_fetching::PoolFetching>,
        )> {
            use $crate::sources::uniswap_v2::pool_fetching::PoolReading;

            let factory = <$factory>::deployed(web3).await?;
            let provider = $crate::sources::uniswap_v2::pair_provider::PairProvider {
                factory: factory.address(),
                init_code_digest: INIT_CODE_DIGEST,
            };
            let fetcher = $crate::sources::uniswap_v2::pool_fetching::PoolFetcher {
                pool_reader: <$pool_reader>::for_pair_provider(provider.clone(), web3.clone()),
                web3: web3.clone(),
            };

            Ok((provider, ::std::sync::Arc::new(fetcher)))
        }
    };
}

pub(crate) use impl_uniswap_like_liquidity;
