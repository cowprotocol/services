//! SushiSwap baseline liquidity source implementation.

use super::uniswap_v2::macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::SushiSwapFactory,
    init_code_digest: "e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303",
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract_mock::Mock;
    use model::TokenPair;

    #[tokio::test]
    async fn test_create2_sushiswap() {
        // https://sushiswap.vision/pair/0x41328fdba556c8c969418ccccb077b7b8d932aa5
        let (mainnet_pair_provider, _) = get_liquidity_source(&Mock::new(1).web3()).await.unwrap();
        let mainnet_pair = TokenPair::new(testlib::tokens::GNO, testlib::tokens::WETH).unwrap();
        assert_eq!(
            mainnet_pair_provider.pair_address(&mainnet_pair),
            addr!("41328fdba556c8c969418ccccb077b7b8d932aa5")
        );

        // Görli
        let (goerli_pair_provider, _) = get_liquidity_source(&Mock::new(5).web3()).await.unwrap();
        let goerli_pair = TokenPair::new(
            addr!("D87Ba7A50B2E7E660f678A895E4B72E7CB4CCd9C"),
            addr!("dc31Ee1784292379Fbb2964b3B9C4124D8F89C60"),
        )
        .unwrap();
        assert_eq!(
            goerli_pair_provider.pair_address(&goerli_pair),
            addr!("11985F5AbD9Dbda8DA77de82A474201683E39555")
        );

        // Gnosis Chain
        let (gnosis_pair_provider, _) = get_liquidity_source(&Mock::new(100).web3()).await.unwrap();
        let gnosis_pair = TokenPair::new(
            addr!("6a023ccd1ff6f2045c3309768ead9e68f978f6e1"),
            addr!("d3d47d5578e55c880505dc40648f7f9307c3e7a8"),
        )
        .unwrap();
        assert_eq!(
            gnosis_pair_provider.pair_address(&gnosis_pair),
            addr!("3d0af734a22bfce7122dbc6f37464714557ef41f")
        );
    }
}
