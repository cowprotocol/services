//! SushiSwap baseline liquidity source implementation.

use super::uniswap_v2::pair_provider::PairProvider;
use crate::Web3;
use anyhow::Result;
use contracts::SushiSwapFactory;
use hex_literal::hex;

const INIT_CODE_DIGEST: [u8; 32] =
    hex!("e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303");

/// Creates the pair provider for the specified Web3 instance.
pub async fn get_pair_provider(web3: &Web3) -> Result<PairProvider> {
    let factory = SushiSwapFactory::deployed(web3).await?;
    Ok(PairProvider {
        factory: factory.address(),
        init_code_digest: INIT_CODE_DIGEST,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract_mock::Mock;
    use model::TokenPair;

    #[tokio::test]
    async fn test_create2_sushiswap() {
        // https://sushiswap.vision/pair/0x41328fdba556c8c969418ccccb077b7b8d932aa5
        let mainnet_pair_provider = get_pair_provider(&Mock::new(1).web3()).await.unwrap();
        let mainnet_pair = TokenPair::new(
            addr!("6810e776880c02933d47db1b9fc05908e5386b96"),
            addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        )
        .unwrap();
        assert_eq!(
            mainnet_pair_provider.pair_address(&mainnet_pair),
            addr!("41328fdba556c8c969418ccccb077b7b8d932aa5")
        );

        // Rinkeby
        let rinkeby_pair_provider = get_pair_provider(&Mock::new(4).web3()).await.unwrap();
        let rinkeby_pair = TokenPair::new(
            addr!("b98Dd87589e460425Cfb5b535d2402E57579Bf40"),
            addr!("d0593E8bafB8Ec2e70ceb1882617a42cfDFbfEbF"),
        )
        .unwrap();
        assert_eq!(
            rinkeby_pair_provider.pair_address(&rinkeby_pair),
            addr!("7e22b2c7469789cf11e59fc8ddd56cf6109e0dd1")
        );

        // xDai
        let xdai_pair_provider = get_pair_provider(&Mock::new(100).web3()).await.unwrap();
        let xdai_pair = TokenPair::new(
            addr!("6a023ccd1ff6f2045c3309768ead9e68f978f6e1"),
            addr!("d3d47d5578e55c880505dc40648f7f9307c3e7a8"),
        )
        .unwrap();
        assert_eq!(
            xdai_pair_provider.pair_address(&xdai_pair),
            addr!("3d0af734a22bfce7122dbc6f37464714557ef41f")
        );
    }
}
