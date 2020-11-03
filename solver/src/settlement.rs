use model::{Order, TokenPair, UserOrder};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub struct Settlement {
    pub token_pair: TokenPair,
    pub orders: Vec<UserOrder>,
    // TODO: sell amounts, AMM trades
}

// Assumes all orders have spending approved and enough balance.
pub fn find_settlement(
    _orders: &[Order],
    _nonces: &HashMap<TokenPair, u32>,
    _now: Instant,
    _max_order_age: Duration,
    // TODO: AMM price
) -> Option<Settlement> {
    // TODO: Find oldest order where nonce is correct. If it is older than max_order_age settle that pair.
    // TODO: implement solving algorithm
    // Not sure how we should handle the AMM prices for token pairs. If a token pair only has
    // unreasonably priced orders then we cannot settle it. But there could be a counter
    // unreasonable order that does allow settling.
    todo!()
}
