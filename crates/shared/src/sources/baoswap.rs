//! Baoswap baseline liquidity source implementation.

use super::uniswap_v2::macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::BaoswapFactory,
    init_code_digest: "0bae3ead48c325ce433426d2e8e6b07dac10835baec21e163760682ea3d3520d",
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract_mock::Mock;
    use model::TokenPair;

    #[tokio::test]
    async fn test_create2_sushiswap() {
        // xDai
        let (xdai_pair_provider, _) = get_liquidity_source(&Mock::new(100).web3()).await.unwrap();
        let xdai_pair = TokenPair::new(
            addr!("7f7440c5098462f833e123b44b8a03e1d9785bab"),
            addr!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"),
        )
        .unwrap();
        assert_eq!(
            xdai_pair_provider.pair_address(&xdai_pair),
            addr!("8746355882e10aae144d3709889dfaa39ff2a692")
        );
    }
}
