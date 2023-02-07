//! Submodule containing helper methods to pre-process auction data before
//! passing it on to the solvers.

use crate::liquidity::LimitOrder;

pub fn has_at_least_one_user_order(orders: &[LimitOrder]) -> bool {
    orders.iter().any(|order| !order.is_liquidity_order())
}

pub fn has_at_least_one_mature_order(orders: &[LimitOrder]) -> bool {
    orders.iter().any(|order| order.is_mature)
}
