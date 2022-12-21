//! Swapr baseline liquidity source implementation.

mod reader;

use super::uniswap_v2::macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::SwaprFactory,
    init_code_digest: "d306a548755b9295ee49cc729e13ca4a45e00199bbd890fa146da43a50571776",
    pool_reader: reader::SwaprPoolReader,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract_mock::Mock;
    use model::TokenPair;

    #[tokio::test]
    async fn test_create2_mainnet() {
        let (mainnet_pair_provider, _) = get_liquidity_source(&Mock::new(1).web3()).await.unwrap();
        let mainnet_pair = TokenPair::new(
            addr!("a1d65E8fB6e87b60FECCBc582F7f97804B725521"),
            addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        )
        .unwrap();
        assert_eq!(
            mainnet_pair_provider.pair_address(&mainnet_pair),
            addr!("b0Dc4B36e0B4d2e3566D2328F6806EA0B76b4F13")
        );
    }

    #[tokio::test]
    async fn test_create2_xdai() {
        let (xdai_pair_provider, _) = get_liquidity_source(&Mock::new(100).web3()).await.unwrap();
        let xdai_pair = TokenPair::new(
            addr!("6A023CCd1ff6F2045C3309768eAd9E68F978f6e1"),
            addr!("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
        )
        .unwrap();
        assert_eq!(
            xdai_pair_provider.pair_address(&xdai_pair),
            addr!("1865d5445010e0baf8be2eb410d3eae4a68683c2")
        );
    }
}
