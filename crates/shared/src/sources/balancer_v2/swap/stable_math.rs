// TODO - incorporate this into baseline solver.
#![allow(dead_code)]
//! Module emulating some of the functions in the Balancer StableMath.sol
//! smart contract. The original contract code can be found at:
//! https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pkg/pool-stable/contracts/StableMath.sol

use super::error::Error;
use crate::sources::balancer_v2::swap::math::rounded_div;
use crate::sources::balancer_v2::swap::{fixed_point::Bfp, math::BalU256};
use ethcontract::U256;
use lazy_static::lazy_static;

lazy_static! {
    static ref AMP_PRECISION: U256 = U256::from(1000);
}

/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/9eb7e44a4e9ebbadfe3c6242a086118298cadc9f/pkg/pool-stable-phantom/contracts/StableMath.sol#L57-L85
fn calculate_invariant(
    amplification_parameter: U256,
    balances: &[Bfp],
    round_up: bool,
) -> Result<U256, Error> {
    let mut sum = U256::zero();
    let num_tokens_usize = balances.len();
    for balance_i in balances.iter() {
        sum = sum.badd(balance_i.as_uint256())?;
    }
    if sum.is_zero() {
        return Ok(sum);
    }

    let invariant = sum;
    let num_tokens = U256::from(num_tokens_usize);
    let amp_times_total = amplification_parameter.bmul(num_tokens)?;
    if round_up {
        convergence_loop_v1(invariant, amp_times_total, balances, num_tokens, sum)
    } else {
        convergence_loop_v2(invariant, amp_times_total, balances, num_tokens, sum)
    }
}

/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/ad1442113b26ec22081c2047e2ec95355a7f12ba/pkg/pool-stable/contracts/StableMath.sol#L78-L104
fn convergence_loop_v1(
    mut invariant: U256,
    amp_times_total: U256,
    balances: &[Bfp],
    num_tokens: U256,
    sum: U256,
) -> Result<U256, Error> {
    for _ in 0..255 {
        // If balances were empty, we would have returned on sum.is_zero()
        let mut p_d = balances[0].as_uint256().bmul(num_tokens)?;
        for balance in &balances[1..] {
            // P_D = Math.div(Math.mul(Math.mul(P_D, balances[j]), numTokens), invariant, roundUp);
            p_d = rounded_div(
                p_d.bmul(balance.as_uint256())?.bmul(num_tokens)?,
                invariant,
                true,
            )?
        }
        let prev_invariant = invariant;

        invariant = rounded_div(
            // invariant = Math.div(
            //     Math.mul(Math.mul(numTokens, invariant), invariant).add(
            //         Math.div(Math.mul(Math.mul(ampTimesTotal, sum), P_D), _AMP_PRECISION, roundUp)
            //     ),
            num_tokens
                .bmul(invariant)?
                .bmul(invariant)?
                .badd(rounded_div(
                    amp_times_total.bmul(sum)?.bmul(p_d)?,
                    *AMP_PRECISION,
                    true,
                )?)?,
            // Math.mul(numTokens + 1, invariant).add(
            //     // No need to use checked arithmetic for the amp precision, the amp is guaranteed to be at least 1
            //     Math.div(Math.mul(ampTimesTotal - _AMP_PRECISION, P_D), _AMP_PRECISION, !roundUp)
            // ),
            (num_tokens.badd(1.into())?)
                .bmul(invariant)?
                .badd(rounded_div(
                    (amp_times_total.bsub(*AMP_PRECISION)?).bmul(p_d)?,
                    *AMP_PRECISION,
                    false,
                )?)?,
            true,
        )?;

        match convergence_criteria(invariant, prev_invariant) {
            None => continue,
            Some(invariant) => return Ok(invariant),
        }
    }
    Err(Error::StableInvariantDidntConverge)
}

/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/9eb7e44a4e9ebbadfe3c6242a086118298cadc9f/pkg/pool-stable-phantom/contracts/StableMath.sol#L86-L119
fn convergence_loop_v2(
    mut invariant: U256,
    amp_times_total: U256,
    balances: &[Bfp],
    num_tokens: U256,
    sum: U256,
) -> Result<U256, Error> {
    for _ in 0..255 {
        // If balances were empty, we would have returned on sum.is_zero()
        let mut d_p = invariant;
        for balance in balances {
            // (d_p * invariant) / (balance * numTokens)
            d_p = d_p
                .bmul(invariant)?
                .bdiv_down(balance.as_uint256().bmul(num_tokens)?)?;
        }
        let prev_invariant = invariant;

        // ((ampTimesTotal * sum) / AMP_PRECISION + D_P * numTokens) * invariant
        let numerator = amp_times_total
            .bmul(sum)?
            .bdiv_down(*AMP_PRECISION)?
            .badd(d_p.bmul(num_tokens)?)?
            .bmul(invariant)?;
        // ((ampTimesTotal - _AMP_PRECISION) * invariant) / _AMP_PRECISION + (numTokens + 1) * D_P
        let denominator = amp_times_total
            .bsub(*AMP_PRECISION)?
            .bmul(invariant)?
            .bdiv_down(*AMP_PRECISION)?
            .badd(num_tokens.badd(1.into())?.bmul(d_p)?)?;
        invariant = numerator.bdiv_down(denominator)?;
        match convergence_criteria(invariant, prev_invariant) {
            None => continue,
            Some(invariant) => return Ok(invariant),
        }
    }
    Err(Error::StableInvariantDidntConverge)
}

/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/ad1442113b26ec22081c2047e2ec95355a7f12ba/pkg/pool-stable/contracts/StableMath.sol#L109-L147
pub fn calc_out_given_in(
    amplification_parameter: U256,
    balances: &mut [Bfp],
    token_index_in: usize,
    token_index_out: usize,
    token_amount_in: Bfp,
) -> Result<Bfp, Error> {
    // Ensure no index error at token indices provided.
    if token_index_out >= balances.len() || token_index_in >= balances.len() {
        return Err(Error::InvalidToken);
    }
    let invariant = calculate_invariant(amplification_parameter, balances, true)?;
    balances[token_index_in] = balances[token_index_in].add(token_amount_in)?;

    let final_balance_out = get_token_balance_given_invariant_and_all_other_balances(
        amplification_parameter,
        balances,
        invariant,
        token_index_out,
    )?;
    // No need to use checked arithmetic since `tokenAmountIn` was actually added to the same balance right before
    // calling `_getTokenBalanceGivenInvariantAndAllOtherBalances` which doesn't alter the balances array.
    balances[token_index_in] = balances[token_index_in]
        .sub(token_amount_in)
        .expect("will not underflow");

    balances[token_index_out]
        .sub(final_balance_out)?
        .sub(Bfp::from_wei(1.into()))
}

/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/ad1442113b26ec22081c2047e2ec95355a7f12ba/pkg/pool-stable/contracts/StableMath.sol#L152-L190
pub fn calc_in_given_out(
    amplification_parameter: U256,
    balances: &mut [Bfp],
    token_index_in: usize,
    token_index_out: usize,
    token_amount_out: Bfp,
) -> Result<Bfp, Error> {
    // Ensure no index error at token indices provided.
    if token_index_out >= balances.len() || token_index_in >= balances.len() {
        return Err(Error::InvalidToken);
    }
    let invariant = calculate_invariant(amplification_parameter, balances, true)?;
    balances[token_index_out] = balances[token_index_out].sub(token_amount_out)?;

    let final_balance_in = get_token_balance_given_invariant_and_all_other_balances(
        amplification_parameter,
        balances,
        invariant,
        token_index_in,
    )?;

    // No need to use checked arithmetic since `tokenAmountOut` was actually subtracted from the same balance right
    // before calling `_getTokenBalanceGivenInvariantAndAllOtherBalances` which doesn't alter the balances array.
    balances[token_index_out] = balances[token_index_out]
        .add(token_amount_out)
        .expect("Will not overflow");

    final_balance_in
        .sub(balances[token_index_in])?
        .add(Bfp::from_wei(1.into()))
}

/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/ad1442113b26ec22081c2047e2ec95355a7f12ba/pkg/pool-stable/contracts/StableMath.sol#L465-L516
fn get_token_balance_given_invariant_and_all_other_balances(
    amplification_parameter: U256,
    balances: &[Bfp],
    invariant: U256,
    token_index: usize,
) -> Result<Bfp, Error> {
    // Rounds result up overall
    let num_tokens_usize = balances.len();
    let num_tokens = U256::from(num_tokens_usize);
    let amp_times_total = amplification_parameter.bmul(num_tokens)?;
    // This is a private method that will never be called with empty balances.
    let mut sum = balances[0].as_uint256();
    let mut p_d = sum.bmul(num_tokens)?;
    for balance_j in &balances[1..] {
        // P_D = Math.divDown(Math.mul(Math.mul(P_D, balances[j]), balances.length), invariant);
        p_d = p_d
            .bmul(balance_j.as_uint256())?
            .bmul(num_tokens)?
            .bdiv_down(invariant)?;
        sum = sum.badd(balance_j.as_uint256())?;
    }
    // No need to use safe math: loop above implies `sum >= balances[tokenIndex]`
    sum -= balances[token_index].as_uint256();
    let inv2 = invariant.bmul(invariant)?;
    // remove the balance from c by multiplying it
    // uint256 c = Math.mul(
    //     Math.mul(Math.divUp(inv2, Math.mul(ampTimesTotal, P_D)), _AMP_PRECISION),
    //     balances[tokenIndex]
    // );
    let c = inv2
        .bdiv_up(amp_times_total.bmul(p_d)?)?
        .bmul(*AMP_PRECISION)?
        .bmul(balances[token_index].as_uint256())?;

    // uint256 b = sum.add(Math.mul(Math.divDown(invariant, ampTimesTotal), _AMP_PRECISION));
    let b = sum.badd(invariant.bdiv_down(amp_times_total)?.bmul(*AMP_PRECISION)?)?;
    // iterate to find the balance
    // multiply the first iteration outside the loop with `invariant` to set initial approximation.
    // uint256 tokenBalance = Math.divUp(inv2.add(c), invariant.add(b));
    let mut token_balance = inv2.badd(c)?.bdiv_up(invariant.badd(b)?)?;
    for _ in 0..255 {
        let prev_token_balance = token_balance;
        // tokenBalance = Math.divUp(
        //     Math.mul(tokenBalance, tokenBalance).add(c),
        //     Math.mul(tokenBalance, 2).add(b).sub(invariant)
        // );
        token_balance = token_balance
            .bmul(token_balance)?
            .badd(c)?
            .bdiv_up(token_balance.bmul(2.into())?.badd(b)?.bsub(invariant)?)?;
        match convergence_criteria(token_balance, prev_token_balance) {
            None => continue,
            Some(token_balance) => return Ok(Bfp::from_wei(token_balance)),
        }
    }
    Err(Error::StableInvariantDidntConverge)
}

fn convergence_criteria(curr_value: U256, prev_value: U256) -> Option<U256> {
    let one = U256::one();
    if curr_value > prev_value {
        if curr_value
            .bsub(prev_value)
            .expect("curr_value > prev_value")
            <= one
        {
            return Some(curr_value);
        }
    } else if prev_value
        .bsub(curr_value)
        .expect("prev_value >= curr_value")
        <= one
    {
        return Some(curr_value);
    }
    None
}

/// We mimic the smart contract tests from this source:
/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/master/pkg/pool-stable/test/StableMath.test.ts
/// These tests verify that the integer approximations made by the contract functions on the EVM are
/// converging in the same way as their algebraic/polynomial evaluations when using floating point arithmetic.
/// We implement the floating point evaluation methods here in the test and use those
/// to compare with the output of analogous contract methods. Cross-reference to the TS code:
/// https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pvt/helpers/src/models/pools/stable/math.ts
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::swap::fixed_point::Bfp;
    use ethcontract::U256;
    use std::str::FromStr;

    // interpreted from
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pvt/helpers/src/models/pools/stable/math.ts#L53
    fn calculate_analytic_invariant_two_tokens(
        balance_0: f64,
        balance_1: f64,
        amplification_parameter: f64,
    ) -> f64 {
        let sum = balance_0 + balance_1;
        let prod = balance_0 * balance_1;
        let amplification_coefficient = amplification_parameter / 2.;
        let q = amplification_coefficient * -16. * sum * prod;
        let p = (amplification_coefficient - (1.0 / 4.)) * 16. * prod;

        let c = (((((q.powi(2)) / 4.) + (p.powi(3) / 27.)).powf(0.5)) - (q / 2.)).powf(1. / 3.);
        c - (p / (c * 3.))
    }

    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pvt/helpers/src/models/pools/stable/math.ts#L10
    fn calculate_invariant_approx(balances: Vec<f64>, amplification_parameter: f64) -> f64 {
        let total_coins = balances.len() as f64;
        let sum: f64 = balances.iter().sum();
        if sum == 0. {
            return sum;
        }

        let mut inv = sum;
        let amp_times_total = amplification_parameter * total_coins;
        for _ in 0..255 {
            let mut p_d = balances[0] * total_coins;
            for balance in &balances[1..] {
                p_d = (p_d * balance * total_coins) / inv;
            }
            let pre_inv = inv;
            let a = total_coins * inv * inv;
            let b = amp_times_total * sum * p_d;
            let x = (total_coins + 1.) * inv;
            let y = (amp_times_total - 1.) * p_d;
            inv = (a + b) / (x + y);
            // Equality with the precision of 1
            if (inv - pre_inv).abs() < 1. {
                break;
            }
        }
        inv
    }

    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pvt/helpers/src/models/pools/stable/math.ts#L368
    fn get_tb_given_inv_and_other_balances(
        balances: Vec<f64>,
        amplification_parameter: f64,
        invariant: f64,
        token_index: usize,
    ) -> f64 {
        let mut sum = 0.;
        let mut prod = 1.;
        let num_tokens_usize = balances.len();
        let num_tokens = num_tokens_usize as f64;
        for (i, balance) in balances.iter().enumerate() {
            if i != token_index {
                sum += balance;
                prod *= balance;
            }
        }
        let b = (invariant / (amplification_parameter * num_tokens)) + sum - invariant;
        let c = (invariant.powi(num_tokens_usize as i32 + 1) * -1.)
            / (amplification_parameter * num_tokens.powi(num_tokens_usize as i32 + 1) * prod);
        let x = b.powi(2) - (c * 4.);
        let root_x = x.powf(0.5);
        let neg_b = b * -1.;
        (neg_b + root_x) / 2.
    }

    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pvt/helpers/src/models/pools/stable/math.ts#L108
    fn calc_in_given_out_approx(
        mut balances: Vec<f64>,
        amplification_parameter: f64,
        token_index_in: usize,
        token_index_out: usize,
        token_amount_out: f64,
    ) -> f64 {
        let invariant = calculate_invariant_approx(balances.clone(), amplification_parameter);
        balances[token_index_out] -= token_amount_out;
        let final_balance_in = get_tb_given_inv_and_other_balances(
            balances.clone(),
            amplification_parameter,
            invariant,
            token_index_in,
        );
        final_balance_in - balances[token_index_in]
    }

    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/stable-deployment/pvt/helpers/src/models/pools/stable/math.ts#L86
    fn calc_out_given_in_approx(
        mut balances: Vec<f64>,
        amplification_parameter: f64,
        token_index_in: usize,
        token_index_out: usize,
        token_amount_in: f64,
    ) -> f64 {
        let invariant = calculate_invariant_approx(balances.clone(), amplification_parameter);
        balances[token_index_in] += token_amount_in;
        let final_balance_out = get_tb_given_inv_and_other_balances(
            balances.clone(),
            amplification_parameter,
            invariant,
            token_index_out,
        );
        balances[token_index_out] - final_balance_out
    }

    #[test]
    fn invariant_two_tokens_ok() {
        let amp = 100.;
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let balances = vec![Bfp::from(10), Bfp::from(12)];
        let max_relative_error = 0.001;
        let expected = calculate_analytic_invariant_two_tokens(
            balances[0].to_f64_lossy(),
            balances[1].to_f64_lossy(),
            amp,
        );
        for round_up in &[true, false] {
            let result = calculate_invariant(amplification_parameter, &balances, *round_up).unwrap();
            assert!((result.to_f64_lossy() / 1e18 - expected)
                .abs()
                .le(&max_relative_error));
        }
    }

    #[test]
    fn invariant_two_tokens_err() {
        let amp = 5000.;
        let balances: Vec<Bfp> = vec!["0.00001", "1200000", "300"]
            .iter()
            .map(|x| Bfp::from_str(x).unwrap())
            .collect();
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        assert_eq!(
            calculate_invariant(amplification_parameter, balances.as_slice(), true)
                .unwrap_err()
                .to_string(),
            "BAL#321: StableInvariantDidntConverge"
        );
    }

    #[test]
    fn invariant_converges_at_extreme_values() {
        let amp = 5000.;
        let balances: Vec<Bfp> = vec!["0.00001", "1200000", "300"]
            .iter()
            .map(|x| Bfp::from_str(x).unwrap())
            .collect();
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let result =
            calculate_invariant(amplification_parameter, balances.as_slice(), false).unwrap();
        let float_balances = balances.iter().map(|x| x.to_f64_lossy()).collect();
        let expected = calculate_invariant_approx(float_balances, amp);
        let max_relative_error = 0.001;
        assert!((result.to_f64_lossy() / 1e18 - expected)
            .abs()
            .le(&max_relative_error));
    }

    #[test]
    fn invariant_three_tokens_ok() {
        let amp = 100.;
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let balances = vec![Bfp::from(10), Bfp::from(12), Bfp::from(14)];
        let float_balances = balances.iter().map(|x| x.to_f64_lossy()).collect();
        let expected = calculate_invariant_approx(float_balances, amp);
        let max_relative_error = 0.001;
        for round_up in &[true, false] {
            let result = calculate_invariant(amplification_parameter, &balances, *round_up).unwrap();
            assert!((result.to_f64_lossy() / 1e18 - expected)
                .abs()
                .le(&max_relative_error));
        }
    }

    #[test]
    fn in_given_out_two_tokens() {
        let amp = 100.;
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let mut balances = [Bfp::from(10), Bfp::from(12)];
        let float_balances = balances.iter().map(|x| x.to_f64_lossy()).collect();
        let token_index_in = 0;
        let token_index_out = 1;
        let amount_out = Bfp::from(1);
        let result = calc_in_given_out(
            amplification_parameter,
            &mut balances,
            token_index_in,
            token_index_out,
            amount_out,
        )
        .unwrap();
        let expected = calc_in_given_out_approx(
            float_balances,
            amp,
            token_index_in,
            token_index_out,
            amount_out.to_f64_lossy(),
        );
        let max_relative_error = 0.001;
        assert!((result.to_f64_lossy() - expected)
            .abs()
            .le(&max_relative_error));
    }

    #[test]
    fn in_given_out_three_tokens() {
        let amp = 100.;
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let mut balances = [Bfp::from(10), Bfp::from(12), Bfp::from(14)];
        let float_balances = balances.iter().map(|x| x.to_f64_lossy()).collect();
        let token_index_in = 0;
        let token_index_out = 1;
        let amount_out = Bfp::from(1);
        let result = calc_in_given_out(
            amplification_parameter,
            &mut balances,
            token_index_in,
            token_index_out,
            amount_out,
        )
        .unwrap();
        let expected = calc_in_given_out_approx(
            float_balances,
            amp,
            token_index_in,
            token_index_out,
            amount_out.to_f64_lossy(),
        );
        let max_relative_error = 0.001;
        assert!((result.to_f64_lossy() - expected)
            .abs()
            .le(&max_relative_error));
    }

    #[test]
    fn out_given_in_two_tokens() {
        let amp = 100.;
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let mut balances = [Bfp::from(10), Bfp::from(12)];
        let float_balances = balances.iter().map(|x| x.to_f64_lossy()).collect();
        let token_index_in = 0;
        let token_index_out = 1;
        let token_amount_in = Bfp::from(1);
        let result = calc_out_given_in(
            amplification_parameter,
            &mut balances,
            token_index_in,
            token_index_out,
            token_amount_in,
        )
        .unwrap();
        let expected = calc_out_given_in_approx(
            float_balances,
            amp,
            token_index_in,
            token_index_out,
            token_amount_in.to_f64_lossy(),
        );
        let max_relative_error = 0.001;
        assert!((result.to_f64_lossy() - expected)
            .abs()
            .le(&max_relative_error));
    }

    #[test]
    fn out_given_in_three_tokens() {
        let amp = 100.;
        let amplification_parameter = U256::from_f64_lossy(amp * AMP_PRECISION.to_f64_lossy());
        let mut balances = [Bfp::from(10), Bfp::from(12), Bfp::from(14)];
        let float_balances = balances.iter().map(|x| x.to_f64_lossy()).collect();
        let token_index_in = 0;
        let token_index_out = 1;
        let token_amount_in = Bfp::from(1);
        let result = calc_out_given_in(
            amplification_parameter,
            &mut balances,
            token_index_in,
            token_index_out,
            token_amount_in,
        )
        .unwrap();
        let expected = calc_out_given_in_approx(
            float_balances,
            amp,
            token_index_in,
            token_index_out,
            token_amount_in.to_f64_lossy(),
        );
        let max_relative_error = 0.001;
        assert!((result.to_f64_lossy() - expected)
            .abs()
            .le(&max_relative_error));
    }
}
