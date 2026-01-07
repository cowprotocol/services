use {
    crate::{
        baseline_solver::BaselineSolvable,
        conversions::U256Ext,
        sources::balancer_v2::pool_fetching::{
            AmplificationParameter,
            StablePool,
            TokenState,
            WeightedPool,
            WeightedPoolVersion,
            WeightedTokenState,
        },
    },
    alloy::primitives::{Address, U256},
    error::Error,
    fixed_point::Bfp,
    std::collections::BTreeMap,
};

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
    fn upscaled_balance(&self) -> Result<Bfp, Error> {
        self.upscale(self.balance)
    }

    /// Scales the input token amount to the value that is used by the Balancer
    /// contract to execute math operations.
    fn upscale(&self, amount: U256) -> Result<Bfp, Error> {
        Bfp::from_wei(amount).mul_down(self.scaling_factor)
    }

    /// Returns the token amount corresponding to the internal Balancer
    /// representation for the same amount.
    /// Based on contract code here:
    /// https://github.com/balancer-labs/balancer-v2-monorepo/blob/c18ff2686c61a8cbad72cdcfc65e9b11476fdbc3/pkg/pool-utils/contracts/BasePool.sol#L560-L562
    fn downscale_up(&self, amount: Bfp) -> Result<U256, Error> {
        Ok(amount.div_up(self.scaling_factor)?.as_uint256())
    }

    /// Similar to downscale up above, but rounded down, this is just checked
    /// div. https://github.com/balancer-labs/balancer-v2-monorepo/blob/c18ff2686c61a8cbad72cdcfc65e9b11476fdbc3/pkg/pool-utils/contracts/BasePool.sol#L542-L544
    fn downscale_down(&self, amount: Bfp) -> Result<U256, Error> {
        Ok(amount.div_down(self.scaling_factor)?.as_uint256())
    }
}

/// Weighted pool data as a reference used for computing input and output
/// amounts.
#[derive(Debug)]
pub struct WeightedPoolRef<'a> {
    pub reserves: &'a BTreeMap<Address, WeightedTokenState>,
    pub swap_fee: Bfp,
    pub version: WeightedPoolVersion,
}

impl WeightedPoolRef<'_> {
    fn get_amount_out_inner(
        &self,
        out_token: Address,
        in_amount: U256,
        in_token: Address,
    ) -> Option<U256> {
        // Note that the output of this function does not depend on the pool
        // specialization. All contract branches compute this amount with:
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BaseMinimalSwapInfoPool.sol#L62-L75
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;

        let in_amount_minus_fees = subtract_swap_fee_amount(in_amount, self.swap_fee).ok()?;

        let calc_out_given_in = match self.version {
            WeightedPoolVersion::V0 => weighted_math::calc_out_given_in,
            WeightedPoolVersion::V3Plus => weighted_math::calc_out_given_in_v3,
        };
        let out_amount = calc_out_given_in(
            in_reserves.common.upscaled_balance().ok()?,
            in_reserves.weight,
            out_reserves.common.upscaled_balance().ok()?,
            out_reserves.weight,
            in_reserves.common.upscale(in_amount_minus_fees).ok()?,
        )
        .ok()?;
        out_reserves.common.downscale_down(out_amount).ok()
    }
}

impl BaselineSolvable for WeightedPoolRef<'_> {
    async fn get_amount_out(
        &self,
        out_token: Address,
        (in_amount, in_token): (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        self.get_amount_out_inner(out_token, in_amount, in_token)
    }

    async fn get_amount_in(
        &self,
        in_token: Address,
        (out_amount, out_token): (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        // Note that the output of this function does not depend on the pool
        // specialization. All contract branches compute this amount with:
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/core/contracts/pools/BaseMinimalSwapInfoPool.sol#L75-L88
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;

        let calc_in_given_out = match self.version {
            WeightedPoolVersion::V0 => weighted_math::calc_in_given_out,
            WeightedPoolVersion::V3Plus => weighted_math::calc_in_given_out_v3,
        };
        let in_amount = calc_in_given_out(
            in_reserves.common.upscaled_balance().ok()?,
            in_reserves.weight,
            out_reserves.common.upscaled_balance().ok()?,
            out_reserves.weight,
            out_reserves.common.upscale(out_amount).ok()?,
        )
        .ok()?;
        let amount_in_before_fee = in_reserves.common.downscale_up(in_amount).ok()?;
        let in_amount = add_swap_fee_amount(amount_in_before_fee, self.swap_fee).ok()?;

        converge_in_amount(in_amount, out_amount, |x| {
            self.get_amount_out_inner(out_token, x, in_token)
        })
    }

    async fn gas_cost(&self) -> usize {
        WEIGHTED_SWAP_GAS_COST
    }
}

/// Stable pool data as a reference used for computing input and output amounts.
#[derive(Debug)]
pub struct StablePoolRef<'a> {
    pub address: Address,
    pub reserves: &'a BTreeMap<Address, TokenState>,
    pub swap_fee: Bfp,
    pub amplification_parameter: AmplificationParameter,
}

#[derive(Debug)]
struct BalancesWithIndices {
    token_index_in: usize,
    token_index_out: usize,
    balances: Vec<Bfp>,
}

impl<'a> StablePoolRef<'a> {
    /// This method returns an iterator over the stable pool reserves while
    /// filtering out the BPT token for the pool (i.e. the pool address). This
    /// is used because composable stable pools include their own BPT token
    /// (i.e. the ERC-20 at the pool address) in its registered tokens (i.e. the
    /// ERC-20s that can be swapped over the Balancer V2 Vault), however, this
    /// token is ignored when computing input and output amounts for regular
    /// swaps.
    ///
    /// <https://etherscan.io/address/0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F#code#F2#L278>
    pub fn reserves_without_bpt(
        &self,
    ) -> impl Iterator<Item = (Address, TokenState)> + 'a + use<'a> {
        let bpt = self.address;
        self.reserves
            .iter()
            .map(|(token, state)| (*token, *state))
            .filter(move |&(token, _)| token != bpt)
    }

    fn upscale_balances_with_token_indices(
        &self,
        in_token: &Address,
        out_token: &Address,
    ) -> Result<BalancesWithIndices, Error> {
        let mut balances = vec![];
        let (mut token_index_in, mut token_index_out) = (0, 0);

        for (index, (token, balance)) in self.reserves_without_bpt().enumerate() {
            if token == *in_token {
                token_index_in = index;
            }
            if token == *out_token {
                token_index_out = index;
            }
            balances.push(balance.upscaled_balance()?)
        }
        Ok(BalancesWithIndices {
            token_index_in,
            token_index_out,
            balances,
        })
    }

    fn amplification_parameter_u256(&self) -> Option<U256> {
        self.amplification_parameter
            .with_base(*stable_math::AMP_PRECISION)
    }

    /// Comes from `_onRegularSwap(true, ...)`:
    /// https://etherscan.io/address/0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F#code#F2#L270
    fn regular_swap_given_in(
        &self,
        out_token: Address,
        (in_amount, in_token): (U256, Address),
    ) -> Option<U256> {
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;
        let BalancesWithIndices {
            token_index_in,
            token_index_out,
            mut balances,
        } = self
            .upscale_balances_with_token_indices(&in_token, &out_token)
            .ok()?;
        let in_amount_minus_fees = subtract_swap_fee_amount(in_amount, self.swap_fee).ok()?;
        let out_amount = stable_math::calc_out_given_in(
            self.amplification_parameter_u256()?,
            balances.as_mut_slice(),
            token_index_in,
            token_index_out,
            in_reserves.upscale(in_amount_minus_fees).ok()?,
        )
        .ok()?;
        out_reserves.downscale_down(out_amount).ok()
    }

    /// Comes from `_onRegularSwap(false, ...)`:
    /// https://etherscan.io/address/0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F#code#F2#L270
    fn regular_swap_given_out(
        &self,
        in_token: Address,
        (out_amount, out_token): (U256, Address),
    ) -> Option<U256> {
        let in_reserves = self.reserves.get(&in_token)?;
        let out_reserves = self.reserves.get(&out_token)?;
        let BalancesWithIndices {
            token_index_in,
            token_index_out,
            mut balances,
        } = self
            .upscale_balances_with_token_indices(&in_token, &out_token)
            .ok()?;
        let in_amount = stable_math::calc_in_given_out(
            self.amplification_parameter_u256()?,
            balances.as_mut_slice(),
            token_index_in,
            token_index_out,
            out_reserves.upscale(out_amount).ok()?,
        )
        .ok()?;
        let amount_in_before_fee = in_reserves.downscale_up(in_amount).ok()?;
        add_swap_fee_amount(amount_in_before_fee, self.swap_fee).ok()
    }

    /// Comes from `_swapWithBpt`:
    // https://etherscan.io/address/0xf9ac7B9dF2b3454E841110CcE5550bD5AC6f875F#code#F2#L301
    fn swap_with_bpt(&self) -> Option<alloy::primitives::U256> {
        // TODO: We currently do not implement swapping with BPT for composable
        // stable pools.
        None
    }
}

impl StablePoolRef<'_> {
    fn get_amount_out_inner(
        &self,
        out_token: Address,
        in_amount: alloy::primitives::U256,
        in_token: Address,
    ) -> Option<alloy::primitives::U256> {
        if in_token == self.address || out_token == self.address {
            self.swap_with_bpt()
        } else {
            self.regular_swap_given_in(out_token, (in_amount, in_token))
        }
    }
}

impl BaselineSolvable for StablePoolRef<'_> {
    async fn get_amount_out(
        &self,
        out_token: Address,
        (in_amount, in_token): (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        self.get_amount_out_inner(out_token, in_amount, in_token)
    }

    async fn get_amount_in(
        &self,
        in_token: Address,
        (out_amount, out_token): (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        if in_token == self.address || out_token == self.address {
            self.swap_with_bpt()
        } else {
            let in_amount = self.regular_swap_given_out(in_token, (out_amount, out_token))?;
            converge_in_amount(in_amount, out_amount, |x| {
                self.get_amount_out_inner(out_token, x, in_token)
            })
        }
    }

    async fn gas_cost(&self) -> usize {
        STABLE_SWAP_GAS_COST
    }
}

/// Balancer V2 pools are "unstable", where if you compute an input amount large
/// enough to buy X tokens, selling the computed amount over the same pool in
/// the exact same state will yield X-𝛿 tokens. To work around this, for each
/// hop, we try to converge to some sell amount >= the required buy amount.
fn converge_in_amount(
    in_amount: U256,
    exact_out_amount: U256,
    get_amount_out: impl Fn(U256) -> Option<U256>,
) -> Option<U256> {
    let out_amount = get_amount_out(in_amount)?;
    if out_amount >= exact_out_amount {
        return Some(in_amount);
    }

    // If the computed output amount is not enough; we bump the sell amount a
    // bit. We start by approximating the out amount deficit to in tokens at the
    // trading price and multiply the amount to bump by 10 for each iteration.
    let mut bump = (exact_out_amount - out_amount)
        .checked_mul(in_amount)?
        .ceil_div(&out_amount.max(U256::ONE))
        .max(U256::ONE);

    for _ in 0..6 {
        let bumped_in_amount = in_amount.checked_add(bump)?;
        let out_amount = get_amount_out(bumped_in_amount)?;
        if out_amount >= exact_out_amount {
            return Some(bumped_in_amount);
        }

        bump *= U256::from(10);
    }

    None
}

impl WeightedPool {
    fn as_pool_ref(&self) -> WeightedPoolRef<'_> {
        WeightedPoolRef {
            reserves: &self.reserves,
            swap_fee: self.common.swap_fee,
            version: self.version,
        }
    }
}

impl BaselineSolvable for WeightedPool {
    async fn get_amount_out(
        &self,
        out_token: Address,
        input: (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        self.as_pool_ref().get_amount_out(out_token, input).await
    }

    async fn get_amount_in(
        &self,
        in_token: Address,
        output: (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        self.as_pool_ref().get_amount_in(in_token, output).await
    }

    async fn gas_cost(&self) -> usize {
        self.as_pool_ref().gas_cost().await
    }
}

impl StablePool {
    fn as_pool_ref(&self) -> StablePoolRef<'_> {
        StablePoolRef {
            address: self.common.address,
            reserves: &self.reserves,
            swap_fee: self.common.swap_fee,
            amplification_parameter: self.amplification_parameter,
        }
    }

    /// See [`StablePoolRef::reserves_without_bpt`].
    pub fn reserves_without_bpt(&self) -> impl Iterator<Item = (Address, TokenState)> + '_ {
        self.as_pool_ref().reserves_without_bpt()
    }
}

impl BaselineSolvable for StablePool {
    async fn get_amount_out(
        &self,
        out_token: Address,
        input: (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        self.as_pool_ref().get_amount_out(out_token, input).await
    }

    async fn get_amount_in(
        &self,
        in_token: Address,
        output: (alloy::primitives::U256, Address),
    ) -> Option<alloy::primitives::U256> {
        self.as_pool_ref().get_amount_in(in_token, output).await
    }

    async fn gas_cost(&self) -> usize {
        self.as_pool_ref().gas_cost().await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sources::balancer_v2::pool_fetching::{AmplificationParameter, CommonPoolState},
        alloy::primitives::Address,
    };

    fn create_weighted_pool_with(
        tokens: Vec<Address>,
        balances: Vec<U256>,
        weights: Vec<Bfp>,
        scaling_factors: Vec<Bfp>,
        swap_fee: U256,
    ) -> WeightedPool {
        let mut reserves = BTreeMap::new();
        for i in 0..tokens.len() {
            let (token, balance, weight, scaling_factor) =
                (tokens[i], balances[i], weights[i], scaling_factors[i]);
            reserves.insert(
                token,
                WeightedTokenState {
                    common: TokenState {
                        balance,
                        scaling_factor,
                    },
                    weight,
                },
            );
        }
        WeightedPool {
            common: CommonPoolState {
                id: Default::default(),
                address: Address::ZERO,
                swap_fee: Bfp::from_wei(swap_fee),
                paused: true,
            },
            reserves,
            version: Default::default(),
        }
    }

    fn create_stable_pool_with(
        tokens: Vec<Address>,
        balances: Vec<U256>,
        amplification_parameter: AmplificationParameter,
        scaling_factors: Vec<Bfp>,
        swap_fee: U256,
    ) -> StablePool {
        let mut reserves = BTreeMap::new();
        for i in 0..tokens.len() {
            let (token, balance, scaling_factor) = (tokens[i], balances[i], scaling_factors[i]);
            reserves.insert(
                token,
                TokenState {
                    balance,
                    scaling_factor,
                },
            );
        }
        StablePool {
            common: CommonPoolState {
                id: Default::default(),
                address: Address::ZERO,
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
            scaling_factor: Bfp::exp10(12),
        };
        let input = Bfp::from_wei(U256::from(900_546_079_866_630_330_575_u128));
        assert_eq!(
            token_state.downscale_up(input).unwrap(),
            U256::from(900_546_080_u128)
        );
        assert_eq!(
            token_state.downscale_down(input).unwrap(),
            U256::from(900_546_079_u128)
        );
    }

    #[tokio::test]
    async fn weighted_get_amount_out() {
        // Values obtained from this transaction:
        // https://dashboard.tenderly.co/tx/main/0xa9f571c9bfd4289bd4bd270465d73e1b7e010622ed089d54d81ec63a0365ec22/debugger
        let crv = Address::repeat_byte(21);
        let sdvecrv_dao = Address::repeat_byte(42);
        let b = create_weighted_pool_with(
            vec![crv, sdvecrv_dao],
            vec![
                U256::from(1_850_304_144_768_426_873_445_489_u128),
                U256::from(95_671_347_892_391_047_965_654_u128),
            ],
            vec![bfp!("0.9"), bfp!("0.1")],
            vec![Bfp::exp10(0), Bfp::exp10(0)],
            U256::from(2_000_000_000_000_000_u128),
        );

        assert_eq!(
            b.get_amount_out(
                crv,
                (
                    alloy::primitives::U256::from(227_937_106_828_652_254_870_u128),
                    sdvecrv_dao
                )
            )
            .await
            .unwrap(),
            alloy::primitives::U256::from(488_192_591_864_344_551_330_u128)
        );
    }

    #[tokio::test]
    async fn weighted_get_amount_in() {
        // Values obtained from this transaction:
        // https://dashboard.tenderly.co/tx/main/0xafc3dd6a636a85d9c1976dfa5aee33f78e6ee902f285c9d4cf80a0014aa2a052/debugger
        let weth = Address::repeat_byte(21);
        let tusd = Address::repeat_byte(42);
        let b = create_weighted_pool_with(
            vec![weth, tusd],
            vec![
                U256::from(60_000_000_000_000_000_u128),
                U256::from(250_000_000_u128),
            ],
            vec![bfp!("0.5"), bfp!("0.5")],
            vec![Bfp::exp10(0), Bfp::exp10(12)],
            U256::from(1_000_000_000_000_000_u128),
        );

        assert_eq!(
            b.get_amount_in(weth, (alloy::primitives::U256::from(5_000_000_u128), tusd))
                .await
                .unwrap(),
            alloy::primitives::U256::from(1_225_715_511_430_411_u128)
        );
    }

    #[test]
    fn construct_balances_and_token_indices() {
        let tokens: Vec<_> = (1..=3).map(Address::with_last_byte).collect();
        let balances = (1..=3).map(U256::from).collect();
        let pool = create_stable_pool_with(
            tokens.clone(),
            balances,
            AmplificationParameter::try_new(U256::ONE, U256::ONE).unwrap(),
            vec![Bfp::exp10(18), Bfp::exp10(18), Bfp::exp10(18)],
            U256::ONE,
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

    #[tokio::test]
    async fn stable_get_amount_out() {
        // Test based on actual swap.
        // https://dashboard.tenderly.co/tx/main/0x75be93fff064ad46b423b9e20cee09b0ae7f741087f43e4187d4f4cf59f54229/debugger
        // Token addresses are irrelevant for computation.
        let dai = Address::with_last_byte(1);
        let usdc = Address::with_last_byte(2);
        let tusd = Address::with_last_byte(3);
        let tokens = vec![dai, usdc, tusd];
        let scaling_exps = vec![Bfp::exp10(0), Bfp::exp10(12), Bfp::exp10(12)];
        let amplification_parameter =
            AmplificationParameter::try_new(U256::from(570000), U256::from(1000)).unwrap();
        let balances = vec![
            U256::from(40_927_687_702_846_622_465_144_342_u128),
            U256::from(59_448_574_675_062_u128),
            U256::from(55_199_308_926_456_u128),
        ];
        let swap_fee_percentage = U256::from(300_000_000_000_000_u128);
        let pool = create_stable_pool_with(
            tokens,
            balances,
            amplification_parameter,
            scaling_exps,
            swap_fee_percentage,
        );
        // Etherscan for amount verification:
        // https://etherscan.io/tx/0x75be93fff064ad46b423b9e20cee09b0ae7f741087f43e4187d4f4cf59f54229
        let amount_in = alloy::primitives::U256::from(1_886_982_823_746_269_817_650_u128);
        let amount_out = U256::from(1_887_770_905_u128);
        let res_out = pool.get_amount_out(usdc, (amount_in, dai)).await;
        assert_eq!(res_out.unwrap(), amount_out);
    }

    #[tokio::test]
    async fn stable_get_amount_in() {
        // Test based on actual swap.
        // https://dashboard.tenderly.co/tx/main/0x38487122158eef6b63570b5d3754ddc223c63af5c049d7b80acacb9e8ca89a63/debugger
        // Token addresses are irrelevant for computation.
        let dai = Address::with_last_byte(1);
        let usdc = Address::with_last_byte(2);
        let tusd = Address::with_last_byte(3);
        let tokens = vec![dai, usdc, tusd];
        let scaling_exps = vec![Bfp::exp10(0), Bfp::exp10(12), Bfp::exp10(12)];
        let amplification_parameter =
            AmplificationParameter::try_new(U256::from(570000), U256::from(1000)).unwrap();
        let balances = vec![
            U256::from(34_869_494_603_218_073_631_628_580_u128),
            U256::from(48_176_005_970_419_u128),
            U256::from(44_564_350_355_030_u128),
        ];
        let swap_fee_percentage = U256::from(300_000_000_000_000_u128);
        let pool = create_stable_pool_with(
            tokens,
            balances,
            amplification_parameter,
            scaling_exps,
            swap_fee_percentage,
        );
        // Etherscan for amount verification:
        // https://etherscan.io/tx/0x38487122158eef6b63570b5d3754ddc223c63af5c049d7b80acacb9e8ca89a63
        let amount_in = U256::from(900_816_325_u128);
        let amount_out = alloy::primitives::U256::from(900_000_000_000_000_000_000_u128);
        let res_out = pool.get_amount_in(usdc, (amount_out, dai)).await;
        assert_eq!(res_out.unwrap(), amount_in);
    }
}
