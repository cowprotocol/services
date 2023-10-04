//! Submodule containing helper methods to pre-process auction data before
//! passing it on to the solvers.

use {model::order::Order, primitive_types::U256};

pub fn has_at_least_one_user_order(orders: &[Order]) -> bool {
    orders
        .iter()
        .any(|order| !order.metadata.is_liquidity_order)
}

/// Drops pre-interactions of all partially fillable orders that have
/// already been executed. We do this to ensure that each interaction gets only
/// executed once.
pub fn filter_executed_pre_interactions(orders: &mut [Order]) {
    let zero = 0u32.into();

    let was_partially_executed = |order: &Order| {
        order.metadata.executed_buy_amount != zero
            || order.metadata.executed_sell_amount != zero
            || order.metadata.executed_sell_amount_before_fees != U256::zero()
            || order.metadata.executed_fee_amount != U256::zero()
    };

    for order in orders {
        if was_partially_executed(order) {
            order.interactions.pre.clear();
        }
    }
}
