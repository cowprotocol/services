use ethcontract::H160;
use model::TokenPair;
use web3::signing::keccak256;

#[derive(Clone, Debug)]
pub struct PairProvider {
    pub factory: H160,
    pub init_code_digest: [u8; 32],
}

impl PairProvider {
    pub fn pair_address(&self, pair: &TokenPair) -> H160 {
        let (H160(token0), H160(token1)) = pair.get();

        // https://uniswap.org/docs/v2/javascript-SDK/getting-pair-addresses/
        let salt = {
            let mut buffer = [0u8; 40];
            buffer[0..20].copy_from_slice(&token0);
            buffer[20..40].copy_from_slice(&token1);
            keccak256(&buffer)
        };
        create2_target_address(self.factory, &salt, &self.init_code_digest)
    }
}

fn create2_target_address(creator: H160, salt: &[u8; 32], init_code_digest: &[u8; 32]) -> H160 {
    let mut preimage = [0xff; 85];
    preimage[1..21].copy_from_slice(creator.as_fixed_bytes());
    preimage[21..53].copy_from_slice(salt);
    preimage[53..85].copy_from_slice(init_code_digest);
    H160::from_slice(&keccak256(&preimage)[12..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn test_create2_mainnet() {
        // https://info.uniswap.org/pair/0x3e8468f66d30fc99f745481d4b383f89861702c6
        let provider = PairProvider {
            factory: addr!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            init_code_digest: hex!(
                "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f"
            ),
        };
        let pair = TokenPair::new(testlib::tokens::GNO, testlib::tokens::WETH).unwrap();
        assert_eq!(
            provider.pair_address(&pair),
            addr!("3e8468f66d30fc99f745481d4b383f89861702c6")
        );
    }
}
