use crate::solution_hash::{HashableSolution, HashableTradedOrder, hash_solution};

/// A minimal test implementation of HashableTradedOrder.
#[derive(Clone, Debug)]
struct TestOrder {
    side: u8,
    sell_token: [u8; 20],
    sell_amount: [u8; 32],
    buy_token: [u8; 20],
    buy_amount: [u8; 32],
    executed_sell: [u8; 32],
    executed_buy: [u8; 32],
}

impl HashableTradedOrder for TestOrder {
    fn side_byte(&self) -> u8 {
        self.side
    }
    fn sell_token(&self) -> &[u8] {
        &self.sell_token
    }
    fn sell_amount(&self) -> [u8; 32] {
        self.sell_amount
    }
    fn buy_token(&self) -> &[u8] {
        &self.buy_token
    }
    fn buy_amount(&self) -> [u8; 32] {
        self.buy_amount
    }
    fn executed_sell(&self) -> [u8; 32] {
        self.executed_sell
    }
    fn executed_buy(&self) -> [u8; 32] {
        self.executed_buy
    }
}

/// A minimal test implementation of HashableSolution.
struct TestSolution {
    solution_id: u64,
    solver_address: [u8; 20],
    orders: Vec<([u8; 32], TestOrder)>,
    prices: Vec<([u8; 20], [u8; 32])>,
}

impl HashableSolution for TestSolution {
    type OrderId = [u8; 32];
    type Order = TestOrder;
    type TokenAddr = [u8; 20];

    fn solution_id(&self) -> u64 {
        self.solution_id
    }

    fn solver_address(&self) -> &[u8] {
        &self.solver_address
    }

    fn orders(&self) -> impl Iterator<Item = (&Self::OrderId, &Self::Order)> {
        self.orders.iter().map(|(id, order)| (id, order))
    }

    fn prices(&self) -> impl Iterator<Item = (&Self::TokenAddr, [u8; 32])> {
        self.prices.iter().map(|(token, price)| (token, *price))
    }
}

fn make_order_id(id: u8) -> [u8; 32] {
    let mut uid = [0u8; 32];
    uid[0] = id;
    uid
}

fn make_token(id: u8) -> [u8; 20] {
    let mut addr = [0u8; 20];
    addr[0] = id;
    addr
}

fn make_u256(val: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[24..32].copy_from_slice(&val.to_be_bytes());
    bytes
}

fn make_solver(id: u8) -> [u8; 20] {
    let mut addr = [0u8; 20];
    addr[19] = id;
    addr
}

fn default_order() -> TestOrder {
    TestOrder {
        side: 1, // sell
        sell_token: make_token(0xAA),
        sell_amount: make_u256(1000),
        buy_token: make_token(0xBB),
        buy_amount: make_u256(900),
        executed_sell: make_u256(1000),
        executed_buy: make_u256(950),
    }
}

fn default_solution() -> TestSolution {
    TestSolution {
        solution_id: 42,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), default_order())],
        prices: vec![
            (make_token(0xAA), make_u256(1_000_000)),
            (make_token(0xBB), make_u256(2_000_000)),
        ],
    }
}

#[test]
fn determinism_same_solution_same_hash() {
    let sol_a = default_solution();
    let sol_b = default_solution();

    let hash_a = hash_solution(&sol_a);
    let hash_b = hash_solution(&sol_b);

    assert_eq!(
        hash_a, hash_b,
        "identical solutions must produce identical hashes"
    );
}

#[test]
fn determinism_multiple_calls() {
    let sol = default_solution();

    let hash1 = hash_solution(&sol);
    let hash2 = hash_solution(&sol);
    let hash3 = hash_solution(&sol);

    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);
}

#[test]
fn order_independence_orders() {
    // Create a solution with two orders in order A, B
    let order_a = TestOrder {
        side: 1,
        sell_token: make_token(0xAA),
        sell_amount: make_u256(100),
        buy_token: make_token(0xBB),
        buy_amount: make_u256(90),
        executed_sell: make_u256(100),
        executed_buy: make_u256(95),
    };
    let order_b = TestOrder {
        side: 0,
        sell_token: make_token(0xCC),
        sell_amount: make_u256(200),
        buy_token: make_token(0xDD),
        buy_amount: make_u256(180),
        executed_sell: make_u256(190),
        executed_buy: make_u256(180),
    };

    let sol_ab = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![
            (make_order_id(1), order_a.clone()),
            (make_order_id(2), order_b.clone()),
        ],
        prices: vec![
            (make_token(0xAA), make_u256(100)),
            (make_token(0xBB), make_u256(200)),
        ],
    };

    // Same solution but orders in reverse insertion order
    let sol_ba = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(2), order_b), (make_order_id(1), order_a)],
        prices: vec![
            (make_token(0xAA), make_u256(100)),
            (make_token(0xBB), make_u256(200)),
        ],
    };

    assert_eq!(
        hash_solution(&sol_ab),
        hash_solution(&sol_ba),
        "order of orders must not affect hash (sorted by uid)"
    );
}

#[test]
fn order_independence_prices() {
    let order = default_order();

    let sol_ab = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), order.clone())],
        prices: vec![
            (make_token(0xAA), make_u256(100)),
            (make_token(0xBB), make_u256(200)),
            (make_token(0xCC), make_u256(300)),
        ],
    };

    // Same prices but in reverse insertion order
    let sol_ba = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), order)],
        prices: vec![
            (make_token(0xCC), make_u256(300)),
            (make_token(0xBB), make_u256(200)),
            (make_token(0xAA), make_u256(100)),
        ],
    };

    assert_eq!(
        hash_solution(&sol_ab),
        hash_solution(&sol_ba),
        "order of prices must not affect hash (sorted by token)"
    );
}

#[test]
fn uniqueness_different_solution_id() {
    let mut sol_a = default_solution();
    let mut sol_b = default_solution();
    sol_a.solution_id = 1;
    sol_b.solution_id = 2;

    assert_ne!(
        hash_solution(&sol_a),
        hash_solution(&sol_b),
        "different solution IDs must produce different hashes"
    );
}

#[test]
fn uniqueness_different_solver() {
    let mut sol_a = default_solution();
    let mut sol_b = default_solution();
    sol_a.solver_address = make_solver(1);
    sol_b.solver_address = make_solver(2);

    assert_ne!(
        hash_solution(&sol_a),
        hash_solution(&sol_b),
        "different solver addresses must produce different hashes"
    );
}

#[test]
fn uniqueness_different_order_amounts() {
    let mut order_a = default_order();
    let mut order_b = default_order();
    order_a.executed_buy = make_u256(950);
    order_b.executed_buy = make_u256(960);

    let sol_a = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), order_a)],
        prices: vec![(make_token(0xAA), make_u256(100))],
    };

    let sol_b = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), order_b)],
        prices: vec![(make_token(0xAA), make_u256(100))],
    };

    assert_ne!(
        hash_solution(&sol_a),
        hash_solution(&sol_b),
        "different order amounts must produce different hashes"
    );
}

#[test]
fn uniqueness_different_prices() {
    let order = default_order();

    let sol_a = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), order.clone())],
        prices: vec![(make_token(0xAA), make_u256(100))],
    };

    let sol_b = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), order)],
        prices: vec![(make_token(0xAA), make_u256(200))],
    };

    assert_ne!(
        hash_solution(&sol_a),
        hash_solution(&sol_b),
        "different clearing prices must produce different hashes"
    );
}

#[test]
fn edge_case_empty_orders() {
    let sol = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![],
        prices: vec![(make_token(0xAA), make_u256(100))],
    };

    // Should not panic and should produce a valid hash
    let hash = hash_solution(&sol);
    assert_ne!(
        hash, [0u8; 32],
        "hash of empty-orders solution should not be all zeros"
    );
}

#[test]
fn edge_case_empty_prices() {
    let sol = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), default_order())],
        prices: vec![],
    };

    let hash = hash_solution(&sol);
    assert_ne!(
        hash, [0u8; 32],
        "hash of empty-prices solution should not be all zeros"
    );
}

#[test]
fn edge_case_single_order() {
    let sol = TestSolution {
        solution_id: 1,
        solver_address: make_solver(1),
        orders: vec![(make_order_id(1), default_order())],
        prices: vec![(make_token(0xAA), make_u256(1_000_000))],
    };

    let hash = hash_solution(&sol);
    // Just verify it doesn't panic and gives a non-trivial result
    assert_ne!(hash, [0u8; 32]);
}
