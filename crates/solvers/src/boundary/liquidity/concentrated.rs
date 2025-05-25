use {
    contracts::{
        ethcontract::{H160, I256, U256},
        uniswap_v3_pool,
    },
    ethrpc::Web3,
    model::TokenPair,
    shared::baseline_solver::BaselineSolvable,
};

#[derive(Debug)]
pub struct Pool {
    pub web3: Web3,
    pub address: H160,
    pub tokens: TokenPair,
}

// Computes input or output amounts via eth_calls. The implementation was based
// on these [docs](https://docs.uniswap.org/contracts/v3/reference/core/UniswapV3Pool#swap).
impl BaselineSolvable for Pool {
    async fn get_amount_out(
        &self,
        out_token: H160,
        (in_amount, in_token): (U256, H160),
    ) -> Option<U256> {
        let in_amount = I256::from_raw(in_amount);
        if TokenPair::new(out_token, in_token) != Some(self.tokens) || in_amount.is_negative() {
            // pool has wrong tokens or input amount would overflow
            return None;
        }

        let contract = uniswap_v3_pool::Contract::at(&self.web3, self.address);
        let zero_for_one = in_token == self.tokens.get().0;

        let (amount0, amount1) = contract
            .swap(
                H160::random(), // use random address since we only care about the amounts and not
                // the exact calldata here
                zero_for_one, // indicates whether we swap token0 for token1 or the other way
                in_amount,    // positive value indicates exact input
                U256::zero(), // disable price impact protection
                Default::default(), // don't pass additional data
            )
            .call()
            .await
            .ok()?;

        match zero_for_one {
            true => Some(abs(&amount1)),
            false => Some(abs(&amount0)),
        }
    }

    async fn get_amount_in(
        &self,
        in_token: H160,
        (out_amount, out_token): (U256, H160),
    ) -> Option<U256> {
        let out_amount = I256::from_raw(out_amount);
        if TokenPair::new(out_token, in_token) != Some(self.tokens) || out_amount.is_negative() {
            // pool has wrong tokens or out amount would overflow
            return None;
        }

        let contract = uniswap_v3_pool::Contract::at(&self.web3, self.address);
        let zero_for_one = in_token == self.tokens.get().0;

        let (amount0, amount1) = contract
            .swap(
                H160::random(), // use random address since we only care about the amounts and not
                // the exact calldata here
                zero_for_one, // indicates whether we swap token0 for token1 or the other way
                -out_amount,  // negative indicates exact output
                U256::zero(), // disable price impact protection
                Default::default(), // don't pass additional data
            )
            .call()
            .await
            .ok()?;
        match zero_for_one {
            true => Some(abs(&amount0)),
            false => Some(abs(&amount1)),
        }
    }

    async fn gas_cost(&self) -> usize {
        // TODO: research a reasonable approximation
        100_000
    }
}

fn abs(val: &I256) -> U256 {
    let mut bytes = [0_u8; 32];
    val.abs().to_big_endian(&mut bytes);
    U256::from_big_endian(&bytes)
}
