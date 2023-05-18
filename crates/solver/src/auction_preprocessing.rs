//! Submodule containing helper methods to pre-process auction data before
//! passing it on to the solvers.

use model::order::Order;

pub fn has_at_least_one_user_order(orders: &[Order]) -> bool {
    orders
        .iter()
        .any(|order| !order.metadata.is_liquidity_order)
}
