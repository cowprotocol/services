use crate::{
    baseline_solver::BaselineSolvable,
    sources::balancer_v2::{
        pool_fetching::{StablePool, TokenState, WeightedPool, WeightedTokenState},
        swap::math::BalU256,
    },
};
use error::Error;
use ethcontract::{H160, U256};
use fixed_point::Bfp;
use std::collections::HashMap;

mod error;
pub mod fixed_point;
mod math;
mod stable_math;
mod weighted_math;

const WEIGHTED_SWAP_GAS_COST: usize = 100_000;
// See https://dune.xyz/queries/219641 for cost of pure stable swaps
const STABLE_SWAP_GAS_COST: usize = 183_520;

fn add_swap_fee_amount(amount: U256, swap_fee: Bfp) -> Result<U256, Error> {
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BasePool.sol#L454-L457
    let amount_with_fees = Bfp::from_wei(amount).div_up(swap_fee.complement())?;
    Ok(amount_with_fees.as_uint256())
}

fn subtract_swap_fee_amount(amount: U256, swap_fee: Bfp) -> Result<U256, Error> {
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BasePool.sol#L462-L466
    let amount = Bfp::from_wei(amount);
    let fee_amount = amount.mul_up(swap_fee)?;
    let amount_without_fees = amount.sub(fee_amount)?;
    Ok(amount_without_fees.as_uint256())
}

impl TokenState {
    /// Converts the stored balance into its internal representation as a
    /// Balancer fixed point number.
    fn upscaled_balance(&self) -> Option<Bfp> {
        self.upscale(self.balance)
    }

    fn scaling_exponent_as_factor(&self) -> Option<U256> {
        U256::from(10).checked_pow(self.scaling_exponent.into())
    }

    /// Scales the input token amount to the value that is used by the Balancer
    /// contract to execute math operations.
    fn upscale(&self, amount: U256) -> Option<Bfp> {
        amount
            .checked_mul(self.scaling_exponent_as_factor()?)
            .map(Bfp::from_wei)
    }

    /// Returns the token amount corresponding to the internal Balancer
    /// representation for the same amount.
    /// Based on contract code here:
    /// https://github.com/balancer-labs/balancer-v2-monorepo/blob/c18ff2686c61a8cbad72cdcfc65e9b11476fdbc3/pkg/pool-utils/contracts/BasePool.sol#L560-L562
    fn downscale_up(&self, amount: Bfp) -> Result<U256, Error> {
        let scaling_factor = self
            .scaling_exponent_as_factor()
            .ok_or(Error::MulOverflow)?;
        amount.as_uint256().bdiv_up(scaling_factor)
    }

    /// Similar to downscale up above, but rounded down, this is just checked div.
    /// https://github.com/balancer-labs/balancer-v2-monorepo/blob/c18ff2686c61a8cbad72cdcfc65e9b11476fdbc3/pkg/pool-utils/contracts/BasePool.sol#L542-L544
    fn downscale_down(&self, amount: Bfp) -> Option<U256> {
        amount
            .as_uint256()
            .checked_div(self.scaling_exponent_as_factor()?)
    }
}

/// Weighted pool data as a reference used for computing input and output amounts.
pub struct WeightedPoolRef<'a> {
    pub reserves: &'a HashMap<H160, WeightedTokenState>,
    pub swap_fee: Bfp,
}

impl BaselineSolvable for WeightedPoolRef<'_> {
    fn get_amount_out(&self, out_token: H160, (in_amount, in_token): (U256, H160)) -> Option<U256> {
        // Note that the output of this function does not depend on the pool
        // specialization. All contract branches compute this amount with:
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BaseMinimalSwapInfoPool.sol#L62-L75
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;

        let in_amount_minus_fees = subtract_swap_fee_amount(in_amount, self.swap_fee).ok()?;

        let out_amount = weighted_math::calc_out_given_in(
            in_reserves.common.upscaled_balance()?,
            in_reserves.weight,
            out_reserves.common.upscaled_balance()?,
            out_reserves.weight,
            in_reserves.common.upscale(in_amount_minus_fees)?,
        )
        .ok()?;
        out_reserves.common.downscale_down(out_amount)
    }

    fn get_amount_in(&self, in_token: H160, (out_amount, out_token): (U256, H160)) -> Option<U256> {
        // Note that the output of this function does not depend on the pool
        // specialization. All contract branches compute this amount with:
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BaseMinimalSwapInfoPool.sol#L75-L88
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;

        let in_amount = weighted_math::calc_in_given_out(
            in_reserves.common.upscaled_balance()?,
            in_reserves.weight,
            out_reserves.common.upscaled_balance()?,
            out_reserves.weight,
            out_reserves.common.upscale(out_amount)?,
        )
        .ok()?;
        let amount_in_before_fee = in_reserves.common.downscale_up(in_amount).ok()?;
        add_swap_fee_amount(amount_in_before_fee, self.swap_fee).ok()
    }

    fn gas_cost(&self) -> usize {
        WEIGHTED_SWAP_GAS_COST
    }
}

/// Stable pool data as a reference used for computing input and output amounts.
pub struct StablePoolRef<'a> {
    pub reserves: &'a HashMap<H160, TokenState>,
    pub swap_fee: Bfp,
    pub amplification_parameter: U256,
}

#[derive(Debug)]
struct BalancesWithIndices {
    token_index_in: usize,
    token_index_out: usize,
    balances: Vec<Bfp>,
}

impl StablePoolRef<'_> {
    // TODO - https://github.com/gnosis/gp-v2-services/pull/1225#discussion_r739033527
    // Based on this discussion, it remains to verify that the non-deterministic ordering
    // of the Balance array returned by this method cannot give rise to any undesired
    // rounding/precision errors in the functions which operate on this. Specifically,
    // the internal methods
    // - calculate_invariant and
    // - get_token_balance_given_invariant_and_all_other_balances
    // which perform balancer-arithmetic on the balances array from inside calc_X_given_Y
    // See issue for this task here: https://github.com/gnosis/gp-v2-services/issues/1332
    fn upscale_balances_with_token_indices(
        &self,
        in_token: &H160,
        out_token: &H160,
    ) -> Option<BalancesWithIndices> {
        let mut balances = vec![];
        let (mut token_index_in, mut token_index_out) = (0, 0);
        for (index, (token, balance)) in self.reserves.iter().enumerate() {
            if token == in_token {
                token_index_in = index;
            }
            if token == out_token {
                token_index_out = index;
            }
            balances.push(balance.upscaled_balance()?)
        }
        Some(BalancesWithIndices {
            token_index_in,
            token_index_out,
            balances,
        })
    }
}

impl BaselineSolvable for StablePoolRef<'_> {
    /// Stable pools use the BaseGeneralPool.sol for these methods, called from within `onSwap`
    /// https://github.com/balancer-labs/balancer-v2-monorepo/blob/589542001aeca5bdc120404874fe0137f6a4c749/pkg/pool-utils/contracts/BaseGeneralPool.sol#L31-L44

    /// This comes from `swapGivenIn`
    /// https://github.com/balancer-labs/balancer-v2-monorepo/blob/589542001aeca5bdc120404874fe0137f6a4c749/pkg/pool-utils/contracts/BaseGeneralPool.sol#L46-L63
    fn get_amount_out(&self, out_token: H160, (in_amount, in_token): (U256, H160)) -> Option<U256> {
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;
        let BalancesWithIndices {
            token_index_in,
            token_index_out,
            mut balances,
        } = self.upscale_balances_with_token_indices(&in_token, &out_token)?;
        let in_amount_minus_fees = subtract_swap_fee_amount(in_amount, self.swap_fee).ok()?;
        let out_amount = stable_math::calc_out_given_in(
            self.amplification_parameter,
            balances.as_mut_slice(),
            token_index_in,
            token_index_out,
            in_reserves.upscale(in_amount_minus_fees)?,
        )
        .ok()?;
        out_reserves.downscale_down(out_amount)
    }

    /// Comes from `swapGivenOut`:
    /// https://github.com/balancer-labs/balancer-v2-monorepo/blob/589542001aeca5bdc120404874fe0137f6a4c749/pkg/pool-utils/contracts/BaseGeneralPool.sol#L65-L82
    fn get_amount_in(&self, in_token: H160, (out_amount, out_token): (U256, H160)) -> Option<U256> {
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;
        let BalancesWithIndices {
            token_index_in,
            token_index_out,
            mut balances,
        } = self.upscale_balances_with_token_indices(&in_token, &out_token)?;
        let in_amount = stable_math::calc_in_given_out(
            self.amplification_parameter,
            balances.as_mut_slice(),
            token_index_in,
            token_index_out,
            out_reserves.upscale(out_amount)?,
        )
        .ok()?;
        let amount_in_before_fee = in_reserves.downscale_up(in_amount).ok()?;
        add_swap_fee_amount(amount_in_before_fee, self.swap_fee).ok()
    }

    fn gas_cost(&self) -> usize {
        STABLE_SWAP_GAS_COST
    }
}

impl StablePool {
    fn as_pool_ref(&self) -> StablePoolRef {
        StablePoolRef {
            reserves: &self.reserves,
            swap_fee: self.common.swap_fee,
            amplification_parameter: self.amplification_parameter.as_u256(),
        }
    }
}

impl WeightedPool {
    fn as_pool_ref(&self) -> WeightedPoolRef {
        WeightedPoolRef {
            reserves: &self.reserves,
            swap_fee: self.common.swap_fee,
        }
    }
}

impl BaselineSolvable for WeightedPool {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        self.as_pool_ref().get_amount_out(out_token, input)
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        self.as_pool_ref().get_amount_in(in_token, output)
    }

    fn gas_cost(&self) -> usize {
        self.as_pool_ref().gas_cost()
    }
}

impl BaselineSolvable for StablePool {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        self.as_pool_ref().get_amount_out(out_token, input)
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        self.as_pool_ref().get_amount_in(in_token, output)
    }

    fn gas_cost(&self) -> usize {
        self.as_pool_ref().gas_cost()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::pool_fetching::{AmplificationParameter, CommonPoolState};
    use std::collections::HashMap;

    fn create_weighted_pool_with(
        tokens: Vec<H160>,
        balances: Vec<U256>,
        weights: Vec<Bfp>,
        scaling_exps: Vec<u8>,
        swap_fee: U256,
    ) -> WeightedPool {
        let mut reserves = HashMap::new();
        for i in 0..tokens.len() {
            let (token, balance, weight, scaling_exponent) =
                (tokens[i], balances[i], weights[i], scaling_exps[i]);
            reserves.insert(
                token,
                WeightedTokenState {
                    common: TokenState {
                        balance,
                        scaling_exponent,
                    },
                    weight,
                },
            );
        }
        WeightedPool {
            common: CommonPoolState {
                id: Default::default(),
                address: H160::zero(),
                swap_fee: Bfp::from_wei(swap_fee),
                paused: true,
            },
            reserves,
        }
    }

    fn create_stable_pool_with(
        tokens: Vec<H160>,
        balances: Vec<U256>,
        amplification_parameter: AmplificationParameter,
        scaling_exps: Vec<u8>,
        swap_fee: U256,
    ) -> StablePool {
        let mut reserves = HashMap::new();
        for i in 0..tokens.len() {
            let (token, balance, scaling_exponent) = (tokens[i], balances[i], scaling_exps[i]);
            reserves.insert(
                token,
                TokenState {
                    balance,
                    scaling_exponent,
                },
            );
        }
        StablePool {
            common: CommonPoolState {
                id: Default::default(),
                address: H160::zero(),
                swap_fee: Bfp::from_wei(swap_fee),
                paused: true,
            },
            reserves,
            amplification_parameter,
        }
    }

    #[test]
    fn downscale() {
        let token_state = TokenState {
            balance: Default::default(),
            scaling_exponent: 12,
        };
        let input = Bfp::from_wei(900_546_079_866_630_330_575_i128.into());
        assert_eq!(
            token_state.downscale_up(input).unwrap(),
            U256::from(900_546_080_u128)
        );
        assert_eq!(
            token_state.downscale_down(input).unwrap(),
            U256::from(900_546_079_u128)
        );
    }

    #[test]
    fn weighted_get_amount_out() {
        // Values obtained from this transaction:
        // https://dashboard.tenderly.co/tx/main/0xa9f571c9bfd4289bd4bd270465d73e1b7e010622ed089d54d81ec63a0365ec22/debugger
        let crv = H160::repeat_byte(21);
        let sdvecrv_dao = H160::repeat_byte(42);
        let b = create_weighted_pool_with(
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
            b.get_amount_out(crv, (227_937_106_828_652_254_870_i128.into(), sdvecrv_dao))
                .unwrap(),
            488_192_591_864_344_551_330_i128.into()
        );
    }

    #[test]
    fn weighted_get_amount_in() {
        // Values obtained from this transaction:
        // https://dashboard.tenderly.co/tx/main/0xafc3dd6a636a85d9c1976dfa5aee33f78e6ee902f285c9d4cf80a0014aa2a052/debugger
        let weth = H160::repeat_byte(21);
        let tusd = H160::repeat_byte(42);
        let b = create_weighted_pool_with(
            vec![weth, tusd],
            vec![60_000_000_000_000_000_i128.into(), 250_000_000_i128.into()],
            vec!["0.5".parse().unwrap(), "0.5".parse().unwrap()],
            vec![0, 12],
            1_000_000_000_000_000_i128.into(),
        );

        assert_eq!(
            b.get_amount_in(weth, (5_000_000_i128.into(), tusd))
                .unwrap(),
            1_225_715_511_430_411_i128.into()
        );
    }

    #[test]
    fn construct_balances_and_token_indices() {
        let tokens: Vec<_> = (1..=3).map(H160::from_low_u64_be).collect();
        let balances = (1..=3).map(|n| n.into()).collect();
        let pool = create_stable_pool_with(
            tokens.clone(),
            balances,
            AmplificationParameter::new(1.into(), 1.into()).unwrap(),
            vec![18, 18, 18],
            1.into(),
        );

        for token_i in tokens.iter() {
            for token_j in tokens.iter() {
                let res_ij = pool
                    .as_pool_ref()
                    .upscale_balances_with_token_indices(token_i, token_j)
                    .unwrap();
                assert_eq!(
                    res_ij.balances[res_ij.token_index_in],
                    pool.reserves
                        .get(token_i)
                        .unwrap()
                        .upscaled_balance()
                        .unwrap()
                );
                assert_eq!(
                    res_ij.balances[res_ij.token_index_out],
                    pool.reserves
                        .get(token_j)
                        .unwrap()
                        .upscaled_balance()
                        .unwrap()
                );
            }
        }
    }

    #[test]
    fn stable_get_amount_out() {
        // Test based on actual swap.
        // https://dashboard.tenderly.co/tx/main/0x75be93fff064ad46b423b9e20cee09b0ae7f741087f43e4187d4f4cf59f54229/debugger
        // Token addresses are irrelevant for computation.
        let dai = H160::from_low_u64_be(1);
        let usdc = H160::from_low_u64_be(2);
        let tusd = H160::from_low_u64_be(3);
        let tokens = vec![dai, usdc, tusd];
        let scaling_exps = vec![0, 12, 12];
        let amplification_parameter = AmplificationParameter::new(570.into(), 1000.into()).unwrap();
        let balances = vec![
            40_927_687_702_846_622_465_144_342_i128.into(),
            59_448_574_675_062_i128.into(),
            55_199_308_926_456_i128.into(),
        ];
        let swap_fee_percentage = 300_000_000_000_000u128.into();
        let pool = create_stable_pool_with(
            tokens,
            balances,
            amplification_parameter,
            scaling_exps,
            swap_fee_percentage,
        );
        // Etherscan for amount verification:
        // https://etherscan.io/tx/0x75be93fff064ad46b423b9e20cee09b0ae7f741087f43e4187d4f4cf59f54229
        let amount_in = 1_886_982_823_746_269_817_650_i128.into();
        let amount_out = 1_887_770_905_i128;
        let res_out = pool.get_amount_out(usdc, (amount_in, dai));
        assert_eq!(res_out.unwrap(), amount_out.into());
    }

    #[test]
    fn stable_get_amount_in() {
        // Test based on actual swap.
        // https://dashboard.tenderly.co/tx/main/0x38487122158eef6b63570b5d3754ddc223c63af5c049d7b80acacb9e8ca89a63/debugger
        // Token addresses are irrelevant for computation.
        let dai = H160::from_low_u64_be(1);
        let usdc = H160::from_low_u64_be(2);
        let tusd = H160::from_low_u64_be(3);
        let tokens = vec![dai, usdc, tusd];
        let scaling_exps = vec![0, 12, 12];
        let amplification_parameter = AmplificationParameter::new(570.into(), 1000.into()).unwrap();
        let balances = vec![
            34_869_494_603_218_073_631_628_580_i128.into(),
            48_176_005_970_419_i128.into(),
            44_564_350_355_030_i128.into(),
        ];
        let swap_fee_percentage = 300_000_000_000_000u128.into();
        let pool = create_stable_pool_with(
            tokens,
            balances,
            amplification_parameter,
            scaling_exps,
            swap_fee_percentage,
        );
        // Etherscan for amount verification:
        // https://etherscan.io/tx/0x38487122158eef6b63570b5d3754ddc223c63af5c049d7b80acacb9e8ca89a63
        let amount_in = 900_816_325_i128;
        let amount_out = 900_000_000_000_000_000_000_u128.into();
        let res_out = pool.get_amount_in(usdc, (amount_out, dai));
        assert_eq!(res_out.unwrap(), amount_in.into());
    }
}
