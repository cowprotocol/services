//! Submodule containing helper methods to pre-process auction data before passing it on to the solvers.

use crate::liquidity::LimitOrder;

// vk: I would like to extend this to also check that the order has minimum age but for this we need
// access to the creation date which is a more involved change.
pub fn has_at_least_one_user_order(orders: &[LimitOrder]) -> bool {
    orders.iter().any(|order| !order.is_liquidity_order)
}
