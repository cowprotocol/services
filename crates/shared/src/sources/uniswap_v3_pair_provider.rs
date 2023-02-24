use {ethcontract::H160, hex_literal::hex, model::TokenPair, web3::signing::keccak256};

/// Calculates deterministic Uniswapv3 pool address.
/// https://github.com/Uniswap/v3-periphery/blob/main/contracts/libraries/PoolAddress.sol
pub fn pair_address(factory: &H160, pair: &TokenPair, fee: u32) -> H160 {
    const INIT: [u8; 32] = hex!("e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54");

    let (token0, token1) = pair.get();
    let mut buffer = [0u8; 32 * 3];
    buffer[12..32].copy_from_slice(&token0.0);
    buffer[44..64].copy_from_slice(&token1.0);
    buffer[92..96].copy_from_slice(&fee.to_be_bytes());
    let hash = keccak256(&buffer);

    let mut buffer = [0u8; 1 + 20 + 32 + 32];
    buffer[0] = 0xff;
    buffer[1..21].copy_from_slice(&factory.0);
    buffer[21..53].copy_from_slice(&hash);
    buffer[53..85].copy_from_slice(&INIT);
    let hash = keccak256(&buffer);

    H160::from_slice(&hash[12..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_pool() {
        // https://v3.info.uniswap.org/home#/pools/0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8
        let result = pair_address(
            &addr!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            &TokenPair::new(testlib::tokens::WETH, testlib::tokens::USDC).unwrap(),
            3000,
        );
        assert_eq!(result, addr!("8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8"));
    }
}
