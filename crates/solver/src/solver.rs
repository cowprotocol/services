use {
    crate::liquidity::{ConstantProductOrder, WeightedProductOrder},
    anyhow::anyhow,
    ethcontract::{H160, U256},
    shared::{
        baseline_solver::BaselineSolvable,
        sources::{balancer_v2::swap::WeightedPoolRef, uniswap_v2::pool_fetching::Pool},
    },
    std::{fmt::Debug, str::FromStr},
};

// Wrapper type for AWS ARN identifiers
#[derive(Debug, Clone)]
pub struct Arn(pub String);

impl FromStr for Arn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        // Could be more strict here, but this should suffice to catch unintended
        // configuration mistakes
        if s.starts_with("arn:aws:kms:") {
            Ok(Self(s.to_string()))
        } else {
            Err(anyhow!("Invalid ARN identifier: {}", s))
        }
    }
}

impl BaselineSolvable for ConstantProductOrder {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        amm_to_pool(self).get_amount_out(out_token, input)
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        amm_to_pool(self).get_amount_in(in_token, output)
    }

    fn gas_cost(&self) -> usize {
        amm_to_pool(self).gas_cost()
    }
}

impl BaselineSolvable for WeightedProductOrder {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        amm_to_weighted_pool(self).get_amount_out(out_token, input)
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        amm_to_weighted_pool(self).get_amount_in(in_token, output)
    }

    fn gas_cost(&self) -> usize {
        amm_to_weighted_pool(self).gas_cost()
    }
}

fn amm_to_pool(amm: &ConstantProductOrder) -> Pool {
    Pool {
        address: amm.address,
        tokens: amm.tokens,
        reserves: amm.reserves,
        fee: amm.fee,
    }
}

fn amm_to_weighted_pool(amm: &WeightedProductOrder) -> WeightedPoolRef {
    WeightedPoolRef {
        reserves: &amm.reserves,
        swap_fee: amm.fee,
        version: amm.version,
    }
}
