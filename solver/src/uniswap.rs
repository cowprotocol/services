use anyhow::Result;
use contracts::{UniswapV2Factory, UniswapV2Pair};
use model::TokenPair;
use primitive_types::H160;

pub struct Pool {
    pub address: H160,
    pub token_pair: TokenPair,
    pub reserve0: u128,
    pub reserve1: u128,
}

impl Pool {
    /// Retrieve the uniswap pool information of a token pair.
    pub async fn from_token_pair(
        factory: &UniswapV2Factory,
        token_pair: &TokenPair,
    ) -> Result<Option<Self>> {
        // Note that in the pair contract token0 always has the lower address as in TokenPair:
        // https://github.com/Uniswap/uniswap-v2-core/blob/4dd59067c76dea4a0e8e4bfdda41877a6b16dedc/contracts/UniswapV2Factory.sol#L25
        let uniswap_pair_address = factory
            .get_pair(token_pair.get().0, token_pair.get().1)
            .call()
            .await?;
        if uniswap_pair_address.is_zero() {
            return Ok(None);
        }
        let pair_contract = UniswapV2Pair::at(&factory.raw_instance().web3(), uniswap_pair_address);
        let reserves = pair_contract.get_reserves().call().await?;
        Ok(Some(Pool {
            address: uniswap_pair_address,
            token_pair: *token_pair,
            reserve0: reserves.0,
            reserve1: reserves.1,
        }))
    }
}
