use crate::{encoding::EncodedSettlement, settlement::Settlement};
use anyhow::Result;
use ethcontract::U256;
use num::BigRational;
use primitive_types::H160;
use shared::conversions::U256Ext;
use std::{collections::HashMap, time::Duration};

// Return None if the result is an error or there are no settlements remaining after removing
// settlements with no trades.
pub fn filter_bad_settlements(
    (solver_name, solver_result): (&'static str, Result<Vec<Settlement>>),
) -> Option<(&'static str, Vec<Settlement>)> {
    let mut settlements = match solver_result {
        Ok(settlements) => settlements,
        Err(err) => {
            tracing::error!("solver {} error: {:?}", solver_name, err);
            return None;
        }
    };
    settlements.retain(|settlement| !settlement.trades().is_empty());
    if settlements.is_empty() {
        return None;
    }
    Some((solver_name, settlements))
}

// A single solver produces multiple settlements
#[derive(Debug)]
pub struct SolverWithSettlements {
    pub name: &'static str,
    pub settlements: Vec<Settlement>,
}

impl From<SolverWithSettlements> for Vec<SettlementWithSolver> {
    fn from(instance: SolverWithSettlements) -> Self {
        let name = instance.name;
        instance
            .settlements
            .into_iter()
            .map(|settlement| SettlementWithSolver { name, settlement })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct SettlementWithSolver {
    pub name: &'static str,
    pub settlement: Settlement,
}

impl From<SettlementWithSolver> for EncodedSettlement {
    fn from(instance: SettlementWithSolver) -> Self {
        instance.settlement.into()
    }
}

impl From<SettlementWithSolver> for Settlement {
    fn from(instance: SettlementWithSolver) -> Self {
        instance.settlement
    }
}

impl SettlementWithSolver {
    pub fn without_onchain_liquidity(&self) -> Self {
        Self {
            name: self.name,
            settlement: self.settlement.without_onchain_liquidity(),
        }
    }
}

// Each individual settlement has an objective value.
#[derive(Debug, Clone)]
pub struct RatedSettlement {
    pub settlement: SettlementWithSolver,
    pub surplus: BigRational,     // In wei.
    pub solver_fees: BigRational, // In wei.
    pub gas_estimate: U256,       // In gas units.
    pub gas_price: BigRational,   // In wei per gas unit.
}

// Helper function for RatedSettlement to allow unit testing objective value computation
// without a Settlement.
fn compute_objective_value(
    surplus: &BigRational,
    solver_fees: &BigRational,
    gas_estimate: &BigRational,
    gas_price: &BigRational,
) -> BigRational {
    let cost = gas_estimate * gas_price;
    surplus + solver_fees - cost
}

impl RatedSettlement {
    pub fn objective_value(&self) -> BigRational {
        let gas_estimate = self.gas_estimate.to_big_rational();
        compute_objective_value(
            &self.surplus,
            &self.solver_fees,
            &gas_estimate,
            &self.gas_price,
        )
    }
}

impl From<RatedSettlement> for EncodedSettlement {
    fn from(instance: RatedSettlement) -> Self {
        instance.settlement.into()
    }
}

impl From<RatedSettlement> for Settlement {
    fn from(instance: RatedSettlement) -> Self {
        instance.settlement.into()
    }
}

impl RatedSettlement {
    pub fn without_onchain_liquidity(&self) -> Self {
        RatedSettlement {
            settlement: self.settlement.without_onchain_liquidity(),
            surplus: self.surplus.clone(),
            solver_fees: self.solver_fees.clone(),
            gas_estimate: self.gas_estimate, // TODO: This becomes an overestimate!
            gas_price: self.gas_price.clone(),
        }
    }
}

// Takes the settlements of a single solver and adds a merged settlement.
pub fn merge_settlements(
    max_merged_settlements: usize,
    prices: &HashMap<H160, BigRational>,
    name: &'static str,
    mut settlements: Vec<Settlement>,
) -> SolverWithSettlements {
    settlements.sort_by_cached_key(|a| -a.total_surplus(prices));

    if let Some(settlement) =
        merge_at_most_settlements(max_merged_settlements, settlements.clone().into_iter())
    {
        settlements.push(settlement);
    }
    SolverWithSettlements { name, settlements }
}

// Goes through the settlements in order and tries to merge a number of them. Keeps going on merge
// error.
fn merge_at_most_settlements(
    max_merges: usize,
    mut settlements: impl Iterator<Item = Settlement>,
) -> Option<Settlement> {
    let mut merged = settlements.next()?;
    let mut merge_count = 1;
    while merge_count < max_merges {
        let next = match settlements.next() {
            Some(settlement) => settlement,
            None => break,
        };
        merged = match merged.clone().merge(next) {
            Ok(settlement) => settlement,
            Err(err) => {
                tracing::debug!("failed to merge settlement: {:?}", err);
                continue;
            }
        };
        merge_count += 1;
    }
    if merge_count > 1 {
        Some(merged)
    } else {
        None
    }
}

pub fn filter_settlements_without_old_orders(
    min_order_age: Duration,
    settlements: &mut Vec<SettlementWithSolver>,
) {
    let settle_orders_older_than =
        chrono::offset::Utc::now() - chrono::Duration::from_std(min_order_age).unwrap();
    settlements.retain(|settlement| {
        settlement
            .settlement
            .trades()
            .iter()
            .any(|trade| trade.order.order_meta_data.creation_date <= settle_orders_older_than)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::Trade;
    use maplit::hashmap;
    use model::order::{Order, OrderCreation, OrderKind, OrderMetaData, OrderUid};
    use num::rational::BigRational;
    use num::traits::FromPrimitive;
    use primitive_types::U256;
    use std::collections::HashSet;

    #[test]
    fn merges_settlements_with_highest_objective_value() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let prices = hashmap! { token0 => 1.into(), token1 => 1.into()};
        let prices_rational = hashmap! {
            token0 => BigRational::from_u8(1).unwrap(),
            token1 => BigRational::from_u8(1).unwrap()
        };
        fn uid(number: u8) -> OrderUid {
            OrderUid([number; 56])
        }

        let trade = |executed_amount, uid_: u8| Trade {
            sell_token_index: 0,
            buy_token_index: 1,
            executed_amount,
            order: Order {
                order_meta_data: OrderMetaData {
                    uid: uid(uid_),
                    ..Default::default()
                },
                order_creation: OrderCreation {
                    sell_token: token0,
                    buy_token: token1,
                    sell_amount: executed_amount,
                    buy_amount: 1.into(),
                    kind: OrderKind::Buy,
                    ..Default::default()
                },
            },
        };
        let settlement = |executed_amount: U256, order_uid: u8| {
            Settlement::with_trades(prices.clone(), vec![trade(executed_amount, order_uid)])
        };

        let settlements = vec![
            settlement(1.into(), 1),
            settlement(2.into(), 2),
            settlement(3.into(), 3),
        ];
        let settlements = merge_settlements(2, &prices_rational, "", settlements).settlements;

        assert_eq!(settlements.len(), 4);
        assert!(settlements.iter().any(|settlement| {
            let trades = settlement.trades();
            let uids: HashSet<OrderUid> = trades
                .iter()
                .map(|trade| trade.order.order_meta_data.uid)
                .collect();
            uids.len() == 2 && uids.contains(&uid(2)) && uids.contains(&uid(3))
        }));
    }

    #[test]
    fn merge_continues_on_error() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let settlement0 = Settlement::new(hashmap! {token0 => 0.into()});
        let settlement1 = Settlement::new(hashmap! {token0 => 2.into()});
        let settlement2 = Settlement::new(hashmap! {token0 => 0.into(), token1 => 1.into()});
        let settlements = vec![settlement0, settlement1, settlement2];

        // Can't merge 0 with 1 because token0 clearing prices is different.
        let merged = merge_at_most_settlements(2, settlements.into_iter()).unwrap();
        assert_eq!(merged.clearing_price(token0), Some(0.into()));
        assert_eq!(merged.clearing_price(token1), Some(1.into()));
    }

    #[test]
    fn merge_does_nothing_on_max_1_merge() {
        let token0 = H160::from_low_u64_be(0);
        let settlement = Settlement::new(hashmap! {token0 => 0.into()});
        let settlements = vec![settlement.clone(), settlement];
        assert!(merge_at_most_settlements(1, settlements.into_iter()).is_none());
    }

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
        let obj_value1 =
            super::compute_objective_value(&surplus1, &solver_fees, &gas_estimate1, &gas_price);

        assert_eq!(
            obj_value1,
            BigRational::from_integer(1_001_000_000_000_000_000_u128.into())
        );

        // Objective value 2 is 1.01 - 5e5 * 10e-9 = 1.005 ETH
        let obj_value2 =
            super::compute_objective_value(&surplus2, &solver_fees, &gas_estimate2, &gas_price);

        assert_eq!(
            obj_value2,
            BigRational::from_integer(1_005_000_000_000_000_000_u128.into())
        );

        assert!(obj_value1 < obj_value2);

        // Case 2: objective value 1 = objective value 2

        // Gas price is 30 gwei
        let gas_price = BigRational::from_integer(30_000_000_000_u128.into());

        // Objective value 1 is 1.004 - 3e5 * 30e-9 = 0.995 ETH
        let obj_value1 =
            super::compute_objective_value(&surplus1, &solver_fees, &gas_estimate1, &gas_price);

        assert_eq!(
            obj_value1,
            BigRational::from_integer(995_000_000_000_000_000_u128.into())
        );

        // Objective value 2 is 1.01 - 5e5 * 30e-9 = 0.995 ETH
        let obj_value2 =
            super::compute_objective_value(&surplus2, &solver_fees, &gas_estimate2, &gas_price);

        assert_eq!(
            obj_value2,
            BigRational::from_integer(995_000_000_000_000_000_u128.into())
        );

        assert!(obj_value1 == obj_value2);

        // Case 3: objective value 1 > objective value 2

        // Gas price is 50 gwei
        let gas_price = BigRational::from_integer(50_000_000_000_u128.into());

        // Objective value 1 is 1.004 - 3e5 * 50e-9 = 0.989 ETH
        let obj_value1 =
            super::compute_objective_value(&surplus1, &solver_fees, &gas_estimate1, &gas_price);

        assert_eq!(
            obj_value1,
            BigRational::from_integer(989_000_000_000_000_000_u128.into())
        );

        // Objective value 2 is 1.01 - 5e5 * 50e-9 = 0.985 ETH
        let obj_value2 =
            super::compute_objective_value(&surplus2, &solver_fees, &gas_estimate2, &gas_price);

        assert_eq!(
            obj_value2,
            BigRational::from_integer(985_000_000_000_000_000_u128.into())
        );

        assert!(obj_value1 > obj_value2);
    }
}
