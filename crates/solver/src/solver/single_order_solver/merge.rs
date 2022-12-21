use crate::settlement::{external_prices::ExternalPrices, Settlement};

// Takes the settlements of a single solver and adds a merged settlement.
pub fn merge_settlements(
    max_merged_settlements: usize,
    prices: &ExternalPrices,
    settlements: &mut Vec<Settlement>,
) {
    settlements.sort_by_cached_key(|a| -a.total_surplus(prices));

    if let Some(settlement) =
        merge_at_most_settlements(max_merged_settlements, settlements.clone().into_iter())
    {
        settlements.push(settlement);
    }
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::settlement::{external_prices::externalprices, Trade};

    use super::*;
    use maplit::hashmap;
    use model::order::{Order, OrderData, OrderKind, OrderMetadata, OrderUid};
    use num::{BigRational, One};
    use primitive_types::{H160, U256};

    #[test]
    fn merges_settlements_with_highest_objective_value() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let prices = hashmap! { token0 => 1u32.into(), token1 => 1u32.into()};
        let external_prices = externalprices! {
            native_token: token0,
            token1 => BigRational::one(),
        };
        fn uid(number: u8) -> OrderUid {
            OrderUid([number; 56])
        }

        let trade = |sell_amount, uid_: u8| Trade {
            executed_amount: 1.into(),
            order: Order {
                metadata: OrderMetadata {
                    uid: uid(uid_),
                    ..Default::default()
                },
                data: OrderData {
                    sell_token: token0,
                    buy_token: token1,
                    sell_amount,
                    buy_amount: 1.into(),
                    kind: OrderKind::Buy,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let settlement = |executed_amount: U256, order_uid: u8| {
            Settlement::with_trades(prices.clone(), vec![trade(executed_amount, order_uid)])
        };

        let mut settlements = vec![
            settlement(1.into(), 1),
            settlement(2.into(), 2),
            settlement(3.into(), 3),
        ];
        merge_settlements(2, &external_prices, &mut settlements);

        assert_eq!(settlements.len(), 4);
        assert!(settlements.iter().any(|settlement| {
            let uids: HashSet<OrderUid> = settlement
                .traded_orders()
                .map(|order| order.metadata.uid)
                .collect();
            uids.len() == 2 && uids.contains(&uid(2)) && uids.contains(&uid(3))
        }));
    }

    #[test]
    fn merge_continues_on_error() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let settlement0 = Settlement::new(hashmap! {token0 => 1.into(), token1 => 2.into()});
        let settlement1 = Settlement::new(hashmap! {token0 => 2.into(), token1 => 2.into()});
        let settlement2 = Settlement::new(hashmap! {token0 => 1.into(), token1 => 2.into()});
        let settlements = vec![settlement0, settlement1, settlement2];

        // Can't merge 0 with 1 because token0 and token1 clearing prices are different.
        let merged = merge_at_most_settlements(2, settlements.into_iter()).unwrap();
        assert_eq!(merged.clearing_price(token0), Some(1.into()));
        assert_eq!(merged.clearing_price(token1), Some(2.into()));
    }

    #[test]
    fn merge_does_nothing_on_max_1_merge() {
        let token0 = H160::from_low_u64_be(0);
        let settlement = Settlement::new(hashmap! {token0 => 0.into()});
        let settlements = vec![settlement.clone(), settlement];
        assert!(merge_at_most_settlements(1, settlements.into_iter()).is_none());
    }
}
