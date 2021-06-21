use crate::{
    balancer::pool_storage::{PoolTokenState, WeightedPool},
    baseline_solver::BaselineSolvable,
    conversions::u256_to_big_int,
};
use error::Error;
use ethcontract::{H160, U256};
use fixed_point::Bfp;
use num::{BigRational, CheckedDiv};
use weighted_math::{calc_in_given_out, calc_out_given_in};

mod error;
pub mod fixed_point;
mod weighted_math;

const BALANCER_SWAP_GAS_COST: usize = 100_000;

impl PoolTokenState {
    /// Converts the stored balance into its internal representation as a
    /// Balancer fixed point number.
    fn upscaled_balance(&self) -> Option<Bfp> {
        self.upscale(self.balance)
    }

    /// Scales the input token amount to the value that is used by the Balancer
    /// contract to execute math operations.
    fn upscale(&self, amount: U256) -> Option<Bfp> {
        amount
            .checked_mul(U256::exp10(self.scaling_exponent as usize))
            .map(Bfp::from_wei)
    }

    /// Returns the token amount corresponding to the internal Balancer
    /// representation for the same amount.
    fn downscale(&self, amount: Bfp) -> Option<U256> {
        amount
            .as_uint256()
            .checked_div(U256::exp10(self.scaling_exponent as usize))
    }
}

impl WeightedPool {
    fn add_swap_fee_amount(&self, amount: U256) -> Result<U256, Error> {
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BasePool.sol#L454-L457
        Bfp::from_wei(amount)
            .div_up(Bfp::from_wei(self.swap_fee_percentage).complement())
            .map(|amount_with_fees| amount_with_fees.as_uint256())
    }

    fn subtract_swap_fee_amount(&self, amount: U256) -> Result<U256, Error> {
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BasePool.sol#L462-L466
        let amount = Bfp::from_wei(amount);
        let fee_amount = amount.mul_up(Bfp::from_wei(self.swap_fee_percentage))?;
        amount
            .sub(fee_amount)
            .map(|amount_without_fees| amount_without_fees.as_uint256())
    }
}

impl BaselineSolvable for WeightedPool {
    fn get_amount_out(&self, out_token: H160, in_amount: U256, in_token: H160) -> Option<U256> {
        // Note that the output of this function does not depend on the pool
        // specialization. All contract branches compute this amount with:
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BaseMinimalSwapInfoPool.sol#L62-L75
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;

        let in_amount_minus_fees = self.subtract_swap_fee_amount(in_amount).ok()?;

        calc_out_given_in(
            in_reserves.upscaled_balance()?,
            in_reserves.weight,
            out_reserves.upscaled_balance()?,
            out_reserves.weight,
            in_reserves.upscale(in_amount_minus_fees)?,
        )
        .ok()
        .map(|bfp| out_reserves.downscale(bfp))
        .flatten()
    }

    fn get_amount_in(&self, out_token: H160, out_amount: U256, in_token: H160) -> Option<U256> {
        // Note that the output of this function does not depend on the pool
        // specialization. All contract branches compute this amount with:
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BaseMinimalSwapInfoPool.sol#L75-L88
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;

        let amount_in = calc_in_given_out(
            in_reserves.upscaled_balance()?,
            in_reserves.weight,
            out_reserves.upscaled_balance()?,
            out_reserves.weight,
            out_reserves.upscale(out_amount)?,
        )
        .ok()
        .map(|bfp| in_reserves.downscale(bfp))
        .flatten()?;

        self.add_swap_fee_amount(amount_in).ok()
    }

    fn get_spot_price(&self, base_token: H160, quote_token: H160) -> Option<BigRational> {
        // https://balancer.fi/whitepaper.pdf#spot-price
        let PoolTokenState {
            weight: base_weight,
            balance: base_balance,
            ..
        } = self.reserves.get(&base_token)?;
        let PoolTokenState {
            weight: quote_weight,
            balance: quote_balance,
            ..
        } = self.reserves.get(&quote_token)?;
        if base_weight.is_zero() || quote_weight.is_zero() {
            return None;
        }

        // note: no need to scale, as the balances are already stored in token
        // units and weights are all rescaled by the same amount.
        let base_rate = BigRational::new(
            u256_to_big_int(base_balance),
            u256_to_big_int(&base_weight.as_uint256()),
        );
        let quote_rate = BigRational::new(
            u256_to_big_int(quote_balance),
            u256_to_big_int(&quote_weight.as_uint256()),
        );
        quote_rate.checked_div(&base_rate)
    }

    fn gas_cost(&self) -> usize {
        BALANCER_SWAP_GAS_COST
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_pool_with(
        tokens: Vec<H160>,
        balances: Vec<U256>,
        weights: Vec<Bfp>,
        scaling_exps: Vec<u8>,
        swap_fee_percentage: U256,
    ) -> WeightedPool {
        let mut reserves = HashMap::new();
        for i in 0..tokens.len() {
            let (token, balance, weight, scaling_exponent) =
                (tokens[i], balances[i], weights[i], scaling_exps[i]);
            reserves.insert(
                token,
                PoolTokenState {
                    balance,
                    weight,
                    scaling_exponent,
                },
            );
        }
        WeightedPool {
            pool_id: Default::default(),
            pool_address: H160::zero(),
            reserves,
            swap_fee_percentage,
        }
    }

    #[test]
    fn get_amount_out() {
        // Values obtained from this transaction:
        // https://dashboard.tenderly.co/tx/main/0xa9f571c9bfd4289bd4bd270465d73e1b7e010622ed089d54d81ec63a0365ec22/debugger
        let crv = H160::repeat_byte(21);
        let sdvecrv_dao = H160::repeat_byte(42);
        let b = create_pool_with(
            vec![crv, sdvecrv_dao],
            vec![
                1_850_304_144_768_426_873_445_489_i128.into(),
                95_671_347_892_391_047_965_654_i128.into(),
            ],
            vec!["0.9".parse().unwrap(), "0.1".parse().unwrap()],
            vec![0, 0],
            2_000_000_000_000_000_i128.into(),
        );

        assert_eq!(
            b.get_amount_out(crv, 227_937_106_828_652_254_870_i128.into(), sdvecrv_dao)
                .unwrap(),
            488_192_591_864_344_551_330_i128.into()
        );
    }

    #[test]
    fn get_amount_in() {
        // Values obtained from this transaction:
        // https://dashboard.tenderly.co/tx/main/0xafc3dd6a636a85d9c1976dfa5aee33f78e6ee902f285c9d4cf80a0014aa2a052/debugger
        let weth = H160::repeat_byte(21);
        let tusd = H160::repeat_byte(42);
        let b = create_pool_with(
            vec![weth, tusd],
            vec![60_000_000_000_000_000_i128.into(), 250_000_000_i128.into()],
            vec!["0.5".parse().unwrap(), "0.5".parse().unwrap()],
            vec![0, 12],
            1_000_000_000_000_000_i128.into(),
        );

        assert_eq!(
            b.get_amount_in(tusd, 5_000_000_i128.into(), weth).unwrap(),
            1_225_715_511_430_411_i128.into()
        );
    }

    #[test]
    fn balanced_spot_price() {
        let weth = H160::repeat_byte(21);
        let usdc = H160::repeat_byte(42);
        let b = create_pool_with(
            vec![weth, usdc],
            vec![U256::exp10(22), U256::exp10(22) * U256::from(2500)],
            vec!["0.5".parse().unwrap(), "0.5".parse().unwrap()],
            vec![0, 12],
            0.into(),
        );

        assert_eq!(
            b.get_spot_price(weth, usdc).unwrap(),
            BigRational::new(2500.into(), 1.into())
        );
        assert_eq!(
            b.get_spot_price(usdc, weth).unwrap(),
            BigRational::new(1.into(), 2500.into())
        );
        assert_eq!(b.get_spot_price(weth, H160::zero()), None);
        assert_eq!(b.get_spot_price(H160::zero(), usdc), None);
        assert_eq!(b.get_spot_price(H160::zero(), H160::zero()), None);
    }

    #[test]
    fn unbalanced_spot_price() {
        let weth = H160::repeat_byte(21);
        let dai = H160::repeat_byte(42);
        let b = create_pool_with(
            vec![weth, dai],
            vec![U256::exp10(22), U256::exp10(22) * U256::from(7500)],
            vec!["0.25".parse().unwrap(), "0.75".parse().unwrap()],
            vec![0, 0],
            0.into(),
        );

        assert_eq!(
            b.get_spot_price(weth, dai).unwrap(),
            BigRational::new(2500.into(), 1.into())
        );
        assert_eq!(
            b.get_spot_price(dai, weth).unwrap(),
            BigRational::new(1.into(), 2500.into())
        );
    }
}
