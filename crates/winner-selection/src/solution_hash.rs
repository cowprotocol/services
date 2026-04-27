use alloy_primitives::keccak256;

/// Concrete view of a traded order's data needed for deterministic hashing.
pub struct HashableOrder<'a> {
    /// `0` for Buy, `1` for Sell.
    pub side: u8,
    pub sell_token: &'a [u8],
    pub sell_amount: [u8; 32],
    pub buy_token: &'a [u8],
    pub buy_amount: [u8; 32],
    pub executed_sell: [u8; 32],
    pub executed_buy: [u8; 32],
}

/// Concrete view of a solution's data needed for deterministic hashing.
///
/// `orders` and `prices` may be passed in any order — `hash_solution` sorts
/// them by key before encoding so independent observers always produce the
/// same hash for the same logical solution.
pub struct HashableSolution<'a> {
    pub solution_id: u64,
    pub solver_address: &'a [u8],
    pub orders: Vec<(&'a [u8], HashableOrder<'a>)>,
    pub prices: Vec<(&'a [u8], [u8; 32])>,
}

pub fn hash_solution(mut sol: HashableSolution<'_>) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&sol.solution_id.to_be_bytes());
    buf.extend_from_slice(sol.solver_address);

    sol.orders.sort_by_key(|(uid, _)| *uid);
    buf.extend_from_slice(&(sol.orders.len() as u64).to_be_bytes());
    for (uid, order) in sol.orders {
        buf.extend_from_slice(uid);
        buf.push(order.side);
        buf.extend_from_slice(order.sell_token);
        buf.extend_from_slice(&order.sell_amount);
        buf.extend_from_slice(order.buy_token);
        buf.extend_from_slice(&order.buy_amount);
        buf.extend_from_slice(&order.executed_sell);
        buf.extend_from_slice(&order.executed_buy);
    }

    sol.prices.sort_by_key(|(token, _)| *token);
    buf.extend_from_slice(&(sol.prices.len() as u64).to_be_bytes());
    for (token, price) in sol.prices {
        buf.extend_from_slice(token);
        buf.extend_from_slice(&price);
    }

    keccak256(&buf).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order_id(id: u8) -> [u8; 32] {
        let mut uid = [0u8; 32];
        uid[0] = id;
        uid
    }

    fn token(id: u8) -> [u8; 20] {
        let mut addr = [0u8; 20];
        addr[0] = id;
        addr
    }

    fn u256(val: u64) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[24..32].copy_from_slice(&val.to_be_bytes());
        bytes
    }

    fn solver(id: u8) -> [u8; 20] {
        let mut addr = [0u8; 20];
        addr[19] = id;
        addr
    }

    fn order<'a>(sell_token: &'a [u8; 20], buy_token: &'a [u8; 20]) -> HashableOrder<'a> {
        HashableOrder {
            side: 1,
            sell_token,
            sell_amount: u256(1000),
            buy_token,
            buy_amount: u256(900),
            executed_sell: u256(1000),
            executed_buy: u256(950),
        }
    }

    #[test]
    fn determinism_same_solution_same_hash() {
        let solver_addr = solver(1);
        let uid = order_id(1);
        let sell = token(0xAA);
        let buy = token(0xBB);

        let make = || HashableSolution {
            solution_id: 42,
            solver_address: &solver_addr,
            orders: vec![(uid.as_slice(), order(&sell, &buy))],
            prices: vec![
                (sell.as_slice(), u256(1_000_000)),
                (buy.as_slice(), u256(2_000_000)),
            ],
        };
        assert_eq!(hash_solution(make()), hash_solution(make()));
    }

    #[test]
    fn order_independence_orders() {
        let solver_addr = solver(1);
        let uid_a = order_id(1);
        let uid_b = order_id(2);
        let sell_a = token(0xAA);
        let buy_a = token(0xBB);
        let sell_b = token(0xCC);
        let buy_b = token(0xDD);
        let prices = vec![
            (sell_a.as_slice(), u256(100)),
            (buy_a.as_slice(), u256(200)),
        ];

        let ab = HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![
                (uid_a.as_slice(), order(&sell_a, &buy_a)),
                (uid_b.as_slice(), order(&sell_b, &buy_b)),
            ],
            prices: prices.clone(),
        };
        let ba = HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![
                (uid_b.as_slice(), order(&sell_b, &buy_b)),
                (uid_a.as_slice(), order(&sell_a, &buy_a)),
            ],
            prices,
        };
        assert_eq!(hash_solution(ab), hash_solution(ba));
    }

    #[test]
    fn order_independence_prices() {
        let solver_addr = solver(1);
        let uid = order_id(1);
        let sell = token(0xAA);
        let buy = token(0xBB);
        let third = token(0xCC);

        let prices_a = vec![
            (sell.as_slice(), u256(100)),
            (buy.as_slice(), u256(200)),
            (third.as_slice(), u256(300)),
        ];
        let prices_b = vec![
            (third.as_slice(), u256(300)),
            (buy.as_slice(), u256(200)),
            (sell.as_slice(), u256(100)),
        ];

        let a = HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![(uid.as_slice(), order(&sell, &buy))],
            prices: prices_a,
        };
        let b = HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![(uid.as_slice(), order(&sell, &buy))],
            prices: prices_b,
        };
        assert_eq!(hash_solution(a), hash_solution(b));
    }

    #[test]
    fn uniqueness_different_solution_id() {
        let solver_addr = solver(1);
        let make = |id| HashableSolution {
            solution_id: id,
            solver_address: &solver_addr,
            orders: vec![],
            prices: vec![],
        };
        assert_ne!(hash_solution(make(1)), hash_solution(make(2)));
    }

    #[test]
    fn uniqueness_different_solver() {
        let s1 = solver(1);
        let s2 = solver(2);
        let a = HashableSolution {
            solution_id: 1,
            solver_address: &s1,
            orders: vec![],
            prices: vec![],
        };
        let b = HashableSolution {
            solution_id: 1,
            solver_address: &s2,
            orders: vec![],
            prices: vec![],
        };
        assert_ne!(hash_solution(a), hash_solution(b));
    }

    #[test]
    fn uniqueness_different_executed_amounts() {
        let solver_addr = solver(1);
        let uid = order_id(1);
        let sell = token(0xAA);
        let buy = token(0xBB);

        let mut a = order(&sell, &buy);
        a.executed_buy = u256(950);
        let mut b = order(&sell, &buy);
        b.executed_buy = u256(960);

        let sa = HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![(uid.as_slice(), a)],
            prices: vec![(sell.as_slice(), u256(100))],
        };
        let sb = HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![(uid.as_slice(), b)],
            prices: vec![(sell.as_slice(), u256(100))],
        };
        assert_ne!(hash_solution(sa), hash_solution(sb));
    }

    #[test]
    fn uniqueness_different_prices() {
        let solver_addr = solver(1);
        let uid = order_id(1);
        let sell = token(0xAA);
        let buy = token(0xBB);
        let make = |price| HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![(uid.as_slice(), order(&sell, &buy))],
            prices: vec![(sell.as_slice(), u256(price))],
        };
        assert_ne!(hash_solution(make(100)), hash_solution(make(200)));
    }

    #[test]
    fn empty_orders_and_prices_do_not_panic() {
        let solver_addr = solver(1);
        let h = hash_solution(HashableSolution {
            solution_id: 1,
            solver_address: &solver_addr,
            orders: vec![],
            prices: vec![],
        });
        assert_ne!(h, [0u8; 32]);
    }
}
