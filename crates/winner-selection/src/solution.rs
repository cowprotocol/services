//! Minimal solution and order data structures.
//!
//! These structs contain only the data needed for winner selection,
//! making them small enough to efficiently send to/from the Pod Service.

pub use state::{RankType, Unscored};
use {
    crate::{
        primitives::{OrderUid, Side},
        state,
    },
    alloy_primitives::{Address, U256, keccak256},
    std::collections::HashMap,
};
pub type Scored = state::Scored<U256>;
pub type Ranked = state::Ranked<U256>;

/// Minimal solution data needed for winner selection.
///
/// This contains only what's absolutely necessary to run the winner selection
/// algorithm.
#[derive(Debug, Clone)]
pub struct Solution<State> {
    /// Solution ID from solver (unique per solver).
    id: u64,

    /// Solver's submission address (used for identifying the solver).
    solver: Address,

    /// Orders executed in this solution.
    orders: Vec<Order>,

    /// Clearing prices proposed by the solver, keyed by token address.
    ///
    /// Not consumed by the arbitration algorithm — kept on the solution
    /// because they are part of the canonical hash used for tie-breaking
    /// across independent observers.
    prices: HashMap<Address, U256>,

    /// State marker (score and ranking information).
    state: State,
}

impl<T> Solution<T> {
    /// Get the solution ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the solver address.
    pub fn solver(&self) -> Address {
        self.solver
    }

    /// Get the orders.
    pub fn orders(&self) -> &[Order] {
        &self.orders
    }

    /// Get the clearing prices.
    pub fn prices(&self) -> &HashMap<Address, U256> {
        &self.prices
    }

    /// Deterministic 32-byte fingerprint of the solution payload.
    ///
    /// The hash is independent of the order in which `orders` and `prices`
    /// were inserted: both are sorted by their key (order uid / token
    /// address) before encoding. This is what lets the autopilot, the
    /// driver, and any third-party verifier reach the same tie-breaking
    /// decision when running the arbitrator over the same logical bids.
    pub fn canonical_hash(&self) -> [u8; 32] {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.id.to_be_bytes());
        buf.extend_from_slice(self.solver.as_slice());

        let mut orders: Vec<&Order> = self.orders.iter().collect();
        orders.sort_by_key(|o| o.uid.0);
        buf.extend_from_slice(&(orders.len() as u64).to_be_bytes());
        for order in orders {
            buf.extend_from_slice(&order.uid.0);
            buf.push(match order.side {
                Side::Buy => 0,
                Side::Sell => 1,
            });
            buf.extend_from_slice(order.sell_token.as_slice());
            buf.extend_from_slice(&order.sell_amount.to_be_bytes::<32>());
            buf.extend_from_slice(order.buy_token.as_slice());
            buf.extend_from_slice(&order.buy_amount.to_be_bytes::<32>());
            buf.extend_from_slice(&order.executed_sell.to_be_bytes::<32>());
            buf.extend_from_slice(&order.executed_buy.to_be_bytes::<32>());
        }

        let mut prices: Vec<(&Address, &U256)> = self.prices.iter().collect();
        prices.sort_by_key(|(token, _)| **token);
        buf.extend_from_slice(&(prices.len() as u64).to_be_bytes());
        for (token, price) in prices {
            buf.extend_from_slice(token.as_slice());
            buf.extend_from_slice(&price.to_be_bytes::<32>());
        }

        keccak256(&buf).0
    }
}

impl<State> state::HasState for Solution<State> {
    type Next<NewState> = Solution<NewState>;
    type State = State;

    fn with_state<NewState>(self, state: NewState) -> Self::Next<NewState> {
        Solution {
            id: self.id,
            solver: self.solver,
            orders: self.orders,
            prices: self.prices,
            state,
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl Solution<Unscored> {
    /// Create a new unscored solution.
    pub fn new(
        id: u64,
        solver: Address,
        orders: Vec<Order>,
        prices: HashMap<Address, U256>,
    ) -> Self {
        Self {
            id,
            solver,
            orders,
            prices,
            state: Unscored,
        }
    }
}

/// Minimal order data needed for winner selection.
///
/// Contains the essential information about how an order was executed,
/// including limit amounts (from the original order) and executed amounts
/// (what actually happened in this solution).
#[derive(Debug, Clone)]
pub struct Order {
    /// Unique order identifier (56 bytes).
    pub uid: OrderUid,

    /// Sell token address.
    pub sell_token: Address,

    /// Buy token address.
    pub buy_token: Address,

    /// Limit amount of sell token (from original order parameters).
    ///
    /// This is the maximum amount the user is willing to sell.
    pub sell_amount: U256,

    /// Limit amount of buy token (from original order parameters).
    ///
    /// This is the minimum amount the user wants to receive.
    pub buy_amount: U256,

    /// Amount of sell token that left the user's wallet (including fees).
    ///
    /// This is the actual executed amount in this solution.
    pub executed_sell: U256,

    /// Amount of buy token the user received (after fees).
    ///
    /// This is the actual amount the user got in this solution.
    pub executed_buy: U256,

    /// Order side (Buy or Sell).
    ///
    /// Determines how surplus is calculated.
    pub side: Side,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order_uid(id: u8) -> OrderUid {
        let mut uid = [0u8; 56];
        uid[0] = id;
        OrderUid(uid)
    }

    fn token(id: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[0] = id;
        Address::from(addr)
    }

    fn solver(id: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[19] = id;
        Address::from(addr)
    }

    fn order(sell_token: Address, buy_token: Address) -> Order {
        Order {
            uid: order_uid(1),
            sell_token,
            buy_token,
            sell_amount: U256::from(1000u64),
            buy_amount: U256::from(900u64),
            executed_sell: U256::from(1000u64),
            executed_buy: U256::from(950u64),
            side: Side::Sell,
        }
    }

    #[test]
    fn determinism_same_solution_same_hash() {
        let s = solver(1);
        let sell = token(0xAA);
        let buy = token(0xBB);
        let make = || {
            Solution::new(
                42,
                s,
                vec![order(sell, buy)],
                HashMap::from([
                    (sell, U256::from(1_000_000u64)),
                    (buy, U256::from(2_000_000u64)),
                ]),
            )
        };
        assert_eq!(make().canonical_hash(), make().canonical_hash());
    }

    #[test]
    fn order_independence_orders() {
        let s = solver(1);
        let sell_a = token(0xAA);
        let buy_a = token(0xBB);
        let sell_b = token(0xCC);
        let buy_b = token(0xDD);
        let mut a = order(sell_a, buy_a);
        a.uid = order_uid(1);
        let mut b = order(sell_b, buy_b);
        b.uid = order_uid(2);

        let prices = HashMap::from([(sell_a, U256::from(100u64)), (buy_a, U256::from(200u64))]);

        let ab = Solution::new(1, s, vec![a.clone(), b.clone()], prices.clone());
        let ba = Solution::new(1, s, vec![b, a], prices);
        assert_eq!(ab.canonical_hash(), ba.canonical_hash());
    }

    #[test]
    fn order_independence_prices() {
        let s = solver(1);
        let sell = token(0xAA);
        let buy = token(0xBB);
        let third = token(0xCC);

        let prices_a = HashMap::from([
            (sell, U256::from(100u64)),
            (buy, U256::from(200u64)),
            (third, U256::from(300u64)),
        ]);
        let prices_b = HashMap::from([
            (third, U256::from(300u64)),
            (buy, U256::from(200u64)),
            (sell, U256::from(100u64)),
        ]);

        let a = Solution::new(1, s, vec![order(sell, buy)], prices_a);
        let b = Solution::new(1, s, vec![order(sell, buy)], prices_b);
        assert_eq!(a.canonical_hash(), b.canonical_hash());
    }

    #[test]
    fn uniqueness_different_solution_id() {
        let s = solver(1);
        let make = |id| Solution::new(id, s, vec![], HashMap::new());
        assert_ne!(make(1).canonical_hash(), make(2).canonical_hash());
    }

    #[test]
    fn uniqueness_different_solver() {
        let a = Solution::new(1, solver(1), vec![], HashMap::new());
        let b = Solution::new(1, solver(2), vec![], HashMap::new());
        assert_ne!(a.canonical_hash(), b.canonical_hash());
    }

    #[test]
    fn uniqueness_different_executed_amounts() {
        let s = solver(1);
        let sell = token(0xAA);
        let buy = token(0xBB);

        let mut a = order(sell, buy);
        a.executed_buy = U256::from(950u64);
        let mut b = order(sell, buy);
        b.executed_buy = U256::from(960u64);

        let prices = HashMap::from([(sell, U256::from(100u64))]);

        let sa = Solution::new(1, s, vec![a], prices.clone());
        let sb = Solution::new(1, s, vec![b], prices);
        assert_ne!(sa.canonical_hash(), sb.canonical_hash());
    }

    #[test]
    fn uniqueness_different_prices() {
        let s = solver(1);
        let sell = token(0xAA);
        let buy = token(0xBB);
        let make = |price: u64| {
            Solution::new(
                1,
                s,
                vec![order(sell, buy)],
                HashMap::from([(sell, U256::from(price))]),
            )
        };
        assert_ne!(make(100).canonical_hash(), make(200).canonical_hash());
    }

    #[test]
    fn empty_orders_and_prices_do_not_panic() {
        let h = Solution::new(1, solver(1), vec![], HashMap::new()).canonical_hash();
        assert_ne!(h, [0u8; 32]);
    }
}
