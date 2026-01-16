use alloy::primitives::keccak256;

fn u64_be(x: u64) -> [u8; 8] {
    x.to_be_bytes()
}

/// Trait for types that can be serialized to 32 big-endian bytes (U256-like).
pub trait ToBeBytes32 {
    fn to_be_bytes_32(&self) -> [u8; 32];
}

/// Trait for Buy/Sell side encoding.
pub trait HashableSide {
    fn to_byte(&self) -> u8;
}

/// Trait for traded order data needed for hashing.
pub trait HashableTradedOrder {
    fn side_byte(&self) -> u8;
    fn sell_token(&self) -> &[u8];
    fn sell_amount(&self) -> [u8; 32];
    fn buy_token(&self) -> &[u8];
    fn buy_amount(&self) -> [u8; 32];
    fn executed_sell(&self) -> [u8; 32];
    fn executed_buy(&self) -> [u8; 32];
}

/// Trait for solution data needed for hashing.
pub trait HashableSolution {
    type OrderId: AsRef<[u8]> + Ord;
    type Order: HashableTradedOrder;
    type TokenAddr: AsRef<[u8]> + Ord;

    fn solution_id(&self) -> u64;
    fn solver_address(&self) -> &[u8];
    fn orders(&self) -> impl Iterator<Item = (&Self::OrderId, &Self::Order)>;
    fn prices(&self) -> impl Iterator<Item = (&Self::TokenAddr, [u8; 32])>;
}

fn encode_traded_order<O: HashableTradedOrder>(buf: &mut Vec<u8>, order: &O) {
    buf.push(order.side_byte());
    buf.extend_from_slice(order.sell_token());
    buf.extend_from_slice(&order.sell_amount());
    buf.extend_from_slice(order.buy_token());
    buf.extend_from_slice(&order.buy_amount());
    buf.extend_from_slice(&order.executed_sell());
    buf.extend_from_slice(&order.executed_buy());
}

pub fn hash_solution<S: HashableSolution>(sol: &S) -> [u8; 32] {
    let mut buf = Vec::new();

    buf.extend_from_slice(&u64_be(sol.solution_id()));
    buf.extend_from_slice(sol.solver_address());

    let mut orders: Vec<_> = sol.orders().collect();
    orders.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));
    buf.extend_from_slice(&u64_be(orders.len() as u64));
    for (uid, order) in orders {
        buf.extend_from_slice(uid.as_ref());
        encode_traded_order(&mut buf, order);
    }

    let mut prices: Vec<_> = sol.prices().collect();
    prices.sort_by(|(t1, _), (t2, _)| t1.cmp(t2));
    buf.extend_from_slice(&u64_be(prices.len() as u64));
    for (token, price_bytes) in prices {
        buf.extend_from_slice(token.as_ref());
        buf.extend_from_slice(&price_bytes);
    }

    keccak256(&buf).0
}
