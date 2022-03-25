use super::model::CostModel;
use crate::price_estimation::gas::*;
use primitive_types::{H160, U256};

pub struct GasModel {
    pub native_token: H160,
    pub gas_price: f64,
}

impl GasModel {
    pub fn cost_for_gas(&self, gas: U256) -> CostModel {
        CostModel {
            amount: U256::from_f64_lossy(self.gas_price) * gas,
            token: self.native_token,
        }
    }

    pub fn gp_order_cost(&self) -> CostModel {
        self.cost_for_gas(GAS_PER_ORDER.into())
    }

    pub fn zeroex_order_cost(&self) -> CostModel {
        self.cost_for_gas(GAS_PER_ZEROEX_ORDER.into())
    }

    pub fn uniswap_cost(&self) -> CostModel {
        self.cost_for_gas(GAS_PER_UNISWAP.into())
    }

    pub fn balancer_cost(&self) -> CostModel {
        self.cost_for_gas(GAS_PER_BALANCER_SWAP.into())
    }
}
