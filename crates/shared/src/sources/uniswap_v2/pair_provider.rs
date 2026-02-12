use {
    alloy::primitives::{Address, keccak256},
    model::TokenPair, tracing::instrument,
};

#[derive(Clone, Copy, Debug)]
pub struct PairProvider {
    pub factory: Address,
    pub init_code_digest: [u8; 32],
}

impl PairProvider {
    #[instrument(skip_all)]
    pub fn pair_address(&self, pair: &TokenPair) -> Address {
        let (token0, token1) = pair.get();

        // https://uniswap.org/docs/v2/javascript-SDK/getting-pair-addresses/
        let salt = {
            let mut buffer = [0u8; 40];
            buffer[0..20].copy_from_slice(token0.as_slice());
            buffer[20..40].copy_from_slice(token1.as_slice());
            keccak256(buffer)
        };
        create2_target_address(self.factory, &salt, &self.init_code_digest)
    }
}

fn create2_target_address(
    creator: Address,
    salt: &[u8; 32],
    init_code_digest: &[u8; 32],
) -> Address {
    let mut preimage = [0xff; 85];
    preimage[1..21].copy_from_slice(creator.as_slice());
    preimage[21..53].copy_from_slice(salt);
    preimage[53..85].copy_from_slice(init_code_digest);
    Address::from_slice(&keccak256(preimage)[12..])
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::address, hex_literal::hex};

    #[test]
    fn test_create2_mainnet() {
        // https://info.uniswap.org/pair/0x3e8468f66d30fc99f745481d4b383f89861702c6
        let provider = PairProvider {
            factory: address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            init_code_digest: hex!(
                "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f"
            ),
        };
        let pair = TokenPair::new(testlib::tokens::GNO, testlib::tokens::WETH).unwrap();
        assert_eq!(
            provider.pair_address(&pair),
            address!("3e8468f66d30fc99f745481d4b383f89861702c6")
        );
    }
}
