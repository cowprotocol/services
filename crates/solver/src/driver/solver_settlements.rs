use {
    crate::settlement::Settlement,
    model::solver_competition::Score,
    num::BigRational,
    primitive_types::U256,
};

pub fn has_user_order(settlement: &Settlement) -> bool {
    settlement.user_trades().next().is_some()
}

// Each individual settlement has an objective value.
#[derive(Debug, Default, Clone)]
pub struct RatedSettlement {
    // Identifies a settlement during a run loop.
    pub id: usize,
    pub settlement: Settlement,
    pub surplus: BigRational,     // In wei.
    pub earned_fees: BigRational, // In wei.
    pub solver_fees: BigRational, // In wei.
    pub gas_estimate: U256,       // In gas units.
    pub gas_price: BigRational,   // In wei per gas unit.
    pub objective_value: BigRational,
    pub score: Score,   // auction based score.
    pub ranking: usize, // auction based ranking.
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::settlement::Trade,
        chrono::{offset::Utc, DateTime},
        model::order::{Order, OrderClass, OrderData, OrderMetadata, OrderUid},
    };

    fn trade(created_at: DateTime<Utc>, uid: u8, class: OrderClass) -> Trade {
        Trade {
            order: Order {
                data: OrderData {
                    sell_amount: 1.into(),
                    buy_amount: 1.into(),
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    creation_date: created_at,
                    uid: OrderUid([uid; 56]),
                    class,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 1.into(),
            ..Default::default()
        }
    }

    #[test]
    fn has_user_order_() {
        let order = |class| trade(Default::default(), 0, class);

        let settlement = Settlement::with_default_prices(vec![]);
        assert!(!has_user_order(&settlement));

        let settlement = Settlement::with_default_prices(vec![order(OrderClass::Limit)]);
        assert!(has_user_order(&settlement));

        let settlement = Settlement::with_default_prices(vec![order(OrderClass::Liquidity)]);
        assert!(!has_user_order(&settlement));

        let settlement = Settlement::with_default_prices(vec![order(OrderClass::Market)]);
        assert!(has_user_order(&settlement));

        let settlement = Settlement::with_default_prices(vec![
            order(OrderClass::Market),
            order(OrderClass::Liquidity),
        ]);
        assert!(has_user_order(&settlement));

        let settlement = Settlement::with_default_prices(vec![
            order(OrderClass::Liquidity),
            order(OrderClass::Limit),
        ]);
        assert!(has_user_order(&settlement));
    }
}
