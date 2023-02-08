use {
    crate::settlement::{external_prices::ExternalPrices, Settlement},
    num::BigRational,
    number_conversions::u256_to_big_rational,
    primitive_types::U256,
};

#[derive(Debug)]
pub struct Inputs {
    pub surplus_given: BigRational,
    pub fees_taken: BigRational,
    pub settlement_cost: BigRational,
}

impl Inputs {
    pub fn from_settlement(
        settlement: &Settlement,
        prices: &ExternalPrices,
        gas_price: &BigRational,
        gas_amount: &U256,
    ) -> Self {
        let gas_amount = u256_to_big_rational(gas_amount);

        Self {
            surplus_given: settlement.total_surplus(prices),
            fees_taken: settlement.total_scaled_unsubsidized_fees(prices),
            settlement_cost: gas_amount * gas_price,
        }
    }

    pub fn objective_value(&self) -> BigRational {
        &self.surplus_given + &self.fees_taken - &self.settlement_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::redundant_clone)]
    #[test]
    fn compute_objective_value() {
        // Surplus1 is 1.003 ETH
        let surplus1 = BigRational::from_integer(1_003_000_000_000_000_000_u128.into());

        // Surplus2 is 1.009 ETH
        let surplus2 = BigRational::from_integer(1_009_000_000_000_000_000_u128.into());

        // Fees is 0.001 ETH
        let solver_fees = BigRational::from_integer(1_000_000_000_000_000_u128.into());

        let gas_estimate1 = BigRational::from_integer(300_000.into());
        let gas_estimate2 = BigRational::from_integer(500_000.into());

        // Three cases when using three different gas prices:

        // Case 1: objective value 1 < objective value 2

        // Gas price is 10 gwei
        let gas_price = BigRational::from_integer(10_000_000_000_u128.into());

        // Objective value 1 is 1.004 - 3e5 * 10e-9 = 1.001 ETH
        let obj_value1 = Inputs {
            surplus_given: surplus1.clone(),
            fees_taken: solver_fees.clone(),
            settlement_cost: gas_estimate1.clone() * gas_price.clone(),
        }
        .objective_value();

        assert_eq!(
            obj_value1,
            BigRational::from_integer(1_001_000_000_000_000_000_u128.into())
        );

        // Objective value 2 is 1.01 - 5e5 * 10e-9 = 1.005 ETH
        let obj_value2 = Inputs {
            surplus_given: surplus2.clone(),
            fees_taken: solver_fees.clone(),
            settlement_cost: gas_estimate2.clone() * gas_price.clone(),
        }
        .objective_value();

        assert_eq!(
            obj_value2,
            BigRational::from_integer(1_005_000_000_000_000_000_u128.into())
        );

        assert!(obj_value1 < obj_value2);

        // Case 2: objective value 1 = objective value 2

        // Gas price is 30 gwei
        let gas_price = BigRational::from_integer(30_000_000_000_u128.into());

        // Objective value 1 is 1.004 - 3e5 * 30e-9 = 0.995 ETH
        let obj_value1 = Inputs {
            surplus_given: surplus1.clone(),
            fees_taken: solver_fees.clone(),
            settlement_cost: gas_estimate1.clone() * gas_price.clone(),
        }
        .objective_value();

        assert_eq!(
            obj_value1,
            BigRational::from_integer(995_000_000_000_000_000_u128.into())
        );

        // Objective value 2 is 1.01 - 5e5 * 30e-9 = 0.995 ETH
        let obj_value2 = Inputs {
            surplus_given: surplus2.clone(),
            fees_taken: solver_fees.clone(),
            settlement_cost: gas_estimate2.clone() * gas_price.clone(),
        }
        .objective_value();

        assert_eq!(
            obj_value2,
            BigRational::from_integer(995_000_000_000_000_000_u128.into())
        );

        assert!(obj_value1 == obj_value2);

        // Case 3: objective value 1 > objective value 2

        // Gas price is 50 gwei
        let gas_price = BigRational::from_integer(50_000_000_000_u128.into());

        // Objective value 1 is 1.004 - 3e5 * 50e-9 = 0.989 ETH
        let obj_value1 = Inputs {
            surplus_given: surplus1.clone(),
            fees_taken: solver_fees.clone(),
            settlement_cost: gas_estimate1.clone() * gas_price.clone(),
        }
        .objective_value();

        assert_eq!(
            obj_value1,
            BigRational::from_integer(989_000_000_000_000_000_u128.into())
        );

        // Objective value 2 is 1.01 - 5e5 * 50e-9 = 0.985 ETH
        let obj_value2 = Inputs {
            surplus_given: surplus2.clone(),
            fees_taken: solver_fees.clone(),
            settlement_cost: gas_estimate2.clone() * gas_price.clone(),
        }
        .objective_value();

        assert_eq!(
            obj_value2,
            BigRational::from_integer(985_000_000_000_000_000_u128.into())
        );

        assert!(obj_value1 > obj_value2);
    }
}
