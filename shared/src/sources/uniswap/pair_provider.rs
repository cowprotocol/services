use contracts::{SushiswapV2Factory, UniswapV2Factory};
use ethcontract::H160;
use hex_literal::hex;
use model::TokenPair;
use web3::signing::keccak256;

const UNISWAP_PAIR_INIT_CODE: [u8; 32] =
    hex!("96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f");
const HONEYSWAP_PAIR_INIT_CODE: [u8; 32] =
    hex!("3f88503e8580ab941773b59034fb4b2a63e86dbc031b3633a925533ad3ed2b93");
const SUSHI_PAIR_INIT_CODE: [u8; 32] =
    hex!("e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303");

pub trait AmmPairProvider: Send + Sync + 'static {
    fn pair_address(&self, pair: &TokenPair) -> H160;
}

pub struct UniswapPairProvider {
    pub factory: UniswapV2Factory,
    pub chain_id: u64,
}

impl AmmPairProvider for UniswapPairProvider {
    fn pair_address(&self, pair: &TokenPair) -> H160 {
        let init_hash = match self.chain_id {
            100 => HONEYSWAP_PAIR_INIT_CODE,
            _ => UNISWAP_PAIR_INIT_CODE,
        };
        pair_address(pair, self.factory.address(), init_hash)
    }
}

pub struct SushiswapPairProvider {
    pub factory: SushiswapV2Factory,
}

impl AmmPairProvider for SushiswapPairProvider {
    fn pair_address(&self, pair: &TokenPair) -> H160 {
        pair_address(pair, self.factory.address(), SUSHI_PAIR_INIT_CODE)
    }
}

fn pair_address(pair: &TokenPair, factory_address: H160, init_hash: [u8; 32]) -> H160 {
    // https://uniswap.org/docs/v2/javascript-SDK/getting-pair-addresses/
    let mut packed = [0u8; 40];
    packed[0..20].copy_from_slice(pair.get().0.as_fixed_bytes());
    packed[20..40].copy_from_slice(pair.get().1.as_fixed_bytes());
    let salt = keccak256(&packed);
    create2(factory_address, &salt, &init_hash)
}

fn create2(address: H160, salt: &[u8; 32], init_hash: &[u8; 32]) -> H160 {
    let mut preimage = [0xff; 85];
    preimage[1..21].copy_from_slice(address.as_fixed_bytes());
    preimage[21..53].copy_from_slice(salt);
    preimage[53..85].copy_from_slice(init_hash);
    H160::from_slice(&keccak256(&preimage)[12..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create2_mainnet() {
        // https://info.uniswap.org/pair/0x3e8468f66d30fc99f745481d4b383f89861702c6
        let mainnet_factory = H160::from_slice(&hex!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"));
        let pair = TokenPair::new(
            H160::from_slice(&hex!("6810e776880c02933d47db1b9fc05908e5386b96")),
            H160::from_slice(&hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        )
        .unwrap();
        assert_eq!(
            pair_address(&pair, mainnet_factory, UNISWAP_PAIR_INIT_CODE),
            H160::from_slice(&hex!("3e8468f66d30fc99f745481d4b383f89861702c6"))
        );
    }

    #[test]
    fn test_create2_xdai() {
        // https://info.honeyswap.org/pair/0x4505b262dc053998c10685dc5f9098af8ae5c8ad
        let mainnet_factory = H160::from_slice(&hex!("A818b4F111Ccac7AA31D0BCc0806d64F2E0737D7"));
        let pair = TokenPair::new(
            H160::from_slice(&hex!("71850b7e9ee3f13ab46d67167341e4bdc905eef9")),
            H160::from_slice(&hex!("e91d153e0b41518a2ce8dd3d7944fa863463a97d")),
        )
        .unwrap();
        assert_eq!(
            pair_address(&pair, mainnet_factory, HONEYSWAP_PAIR_INIT_CODE),
            H160::from_slice(&hex!("4505b262dc053998c10685dc5f9098af8ae5c8ad"))
        );
    }

    #[test]
    fn test_create2_sushiswap() {
        // https://sushiswap.vision/pair/0x41328fdba556c8c969418ccccb077b7b8d932aa5
        let mainnet_factory = H160::from_slice(&hex!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"));
        let pair = TokenPair::new(
            H160::from_slice(&hex!("6810e776880c02933d47db1b9fc05908e5386b96")),
            H160::from_slice(&hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        )
        .unwrap();
        assert_eq!(
            pair_address(&pair, mainnet_factory, SUSHI_PAIR_INIT_CODE),
            H160::from_slice(&hex!("41328fdba556c8c969418ccccb077b7b8d932aa5"))
        );
        // Rinkeby & xDai
        let rinkeby_and_xdai_factory =
            H160::from_slice(&hex!("c35DADB65012eC5796536bD9864eD8773aBc74C4"));

        let xdai_pair = TokenPair::new(
            H160::from_slice(&hex!("6a023ccd1ff6f2045c3309768ead9e68f978f6e1")),
            H160::from_slice(&hex!("d3d47d5578e55c880505dc40648f7f9307c3e7a8")),
        )
        .unwrap();
        assert_eq!(
            pair_address(&xdai_pair, rinkeby_and_xdai_factory, SUSHI_PAIR_INIT_CODE),
            H160::from_slice(&hex!("3d0af734a22bfce7122dbc6f37464714557ef41f"))
        );

        let rinkeby_pair = TokenPair::new(
            H160::from_slice(&hex!("b98Dd87589e460425Cfb5b535d2402E57579Bf40")),
            H160::from_slice(&hex!("d0593E8bafB8Ec2e70ceb1882617a42cfDFbfEbF")),
        )
        .unwrap();
        assert_eq!(
            pair_address(
                &rinkeby_pair,
                rinkeby_and_xdai_factory,
                SUSHI_PAIR_INIT_CODE
            ),
            H160::from_slice(&hex!("7e22b2c7469789cf11e59fc8ddd56cf6109e0dd1"))
        );
    }
}
