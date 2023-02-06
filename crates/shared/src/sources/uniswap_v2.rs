//! Uniswap V2 baseline liquidity source implementation.

pub mod macros;
pub mod pair_provider;
pub mod pool_cache;
pub mod pool_fetching;

use macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::UniswapV2Factory,
    init_code_digest: "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
}

#[cfg(test)]
mod tests {
    use {super::*, ethcontract_mock::Mock, model::TokenPair};

    #[tokio::test]
    async fn test_create2_uniswapv2() {
        // https://info.uniswap.org/pair/0x3e8468f66d30fc99f745481d4b383f89861702c6
        let (mainnet_pair_provider, _) = get_liquidity_source(&Mock::new(1).web3()).await.unwrap();
        let mainnet_pair = TokenPair::new(testlib::tokens::GNO, testlib::tokens::WETH).unwrap();
        assert_eq!(
            mainnet_pair_provider.pair_address(&mainnet_pair),
            addr!("3e8468f66d30fc99f745481d4b383f89861702c6")
        );

        // Görli
        let (goerli_pair_provider, _) = get_liquidity_source(&Mock::new(5).web3()).await.unwrap();
        let goerli_pair = TokenPair::new(
            addr!("02ABBDbAaa7b1BB64B5c878f7ac17f8DDa169532"),
            addr!("3430d04E42a722c5Ae52C5Bffbf1F230C2677600"),
        )
        .unwrap();
        assert_eq!(
            goerli_pair_provider.pair_address(&goerli_pair),
            // https://goerli.etherscan.io/tx/0xd52899a351c83da758b944972b08f9fe1b856d723a9b2fae2a080fd83e29f386#eventlog
            addr!("638F259D0A59e1d3b9e9f7E7dd1CB591C754005b")
        );
    }
}
