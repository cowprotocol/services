//! Honeyswap baseline liquidity source implementation.

use super::uniswap_v2::macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::HoneyswapFactory,
    init_code_digest: "3f88503e8580ab941773b59034fb4b2a63e86dbc031b3633a925533ad3ed2b93",
}

#[cfg(test)]
mod tests {
    use {super::*, ethcontract_mock::Mock, model::TokenPair};

    #[tokio::test]
    async fn test_create2_xdai() {
        // https://info.honeyswap.org/pair/0x4505b262dc053998c10685dc5f9098af8ae5c8ad
        let (xdai_pair_provider, _) = get_liquidity_source(&Mock::new(100).web3()).await.unwrap();
        let xdai_pair = TokenPair::new(
            addr!("71850b7e9ee3f13ab46d67167341e4bdc905eef9"),
            addr!("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
        )
        .unwrap();
        assert_eq!(
            xdai_pair_provider.pair_address(&xdai_pair),
            addr!("4505b262dc053998c10685dc5f9098af8ae5c8ad")
        );
    }
}
