use {
    app_data::AppDataHash,
    bigdecimal::BigDecimal,
    driver::{
        domain::eth,
        infra::solver::dto::auction::Auction as CachedAuction,
        util::serialize::Cached,
    },
    serde::Serialize,
    solvers_dto::auction::{
        self as solver,
        Auction as SolverAuction,
        BuyTokenDestination,
        Class,
        ConstantProductPool,
        ConstantProductReserve,
        FeePolicy,
        InteractionData,
        Kind,
        Liquidity,
        SellTokenSource,
        SigningScheme,
        StablePool,
        StableReserve,
        WeightedProductPool,
        WeightedProductReserve,
        WeightedProductVersion,
    },
    std::{
        collections::HashMap,
        hint::black_box,
        time::{Duration, Instant},
    },
    web3::types::{H160, H256, U256},
};

fn main() {
    let (uncached, cached) = sample_auction();
    let baseline_json = serde_json::to_vec(&uncached).expect("uncached auction serializes");
    let cached_json = serde_json::to_vec(&cached).expect("cached auction serializes");
    let baseline_value: serde_json::Value =
        serde_json::from_slice(&baseline_json).expect("baseline JSON parses");
    let cached_value: serde_json::Value =
        serde_json::from_slice(&cached_json).expect("cached JSON parses");
    if baseline_value != cached_value {
        panic!(
            "serialized payloads diverged (len {} vs {})",
            baseline_json.len(),
            cached_json.len()
        );
    }

    const WARMUP: usize = 200;
    const ITERATIONS: usize = 5_000;

    warmup(&uncached, WARMUP);
    warmup(&cached, WARMUP);

    let uncached_duration = measure(&uncached, ITERATIONS);
    let cached_duration = measure(&cached, ITERATIONS);

    let uncached_per_iter = per_iter(uncached_duration, ITERATIONS);
    let cached_per_iter = per_iter(cached_duration, ITERATIONS);
    let speedup = uncached_duration.as_secs_f64() / cached_duration.as_secs_f64();

    println!(
        "Uncached serialization: {uncached_per_iter:.3} µs/iter ({uncached_duration:?} total)",
    );
    println!(
        "Cached serialization:   {cached_per_iter:.3} µs/iter ({cached_duration:?} total)",
    );
    println!("Speedup: {speedup:.2}x faster");
}

fn warmup<T: Serialize>(value: &T, iterations: usize) {
    let _ = measure(value, iterations);
}

fn measure<T: Serialize>(value: &T, iterations: usize) -> Duration {
    let mut sink = 0usize;
    let start = Instant::now();
    for _ in 0..iterations {
        let bytes = serde_json::to_vec(black_box(value)).expect("serialization");
        sink = sink.wrapping_add(bytes.len());
    }
    let duration = start.elapsed();
    black_box(sink);
    duration
}

fn per_iter(duration: Duration, iterations: usize) -> f64 {
    duration.as_secs_f64() * 1_000_000.0 / iterations as f64
}

fn sample_auction() -> (SolverAuction, CachedAuction) {
    let addresses = sample_addresses();
    let tokens = build_tokens(&addresses);
    let orders = sample_orders(&addresses);
    let liquidity = sample_liquidity(&addresses);
    let owners = sample_owners();
    let deadline = chrono::Utc::now();
    let gas_price = U256::from(42_000_000_000u128);

    let auction = SolverAuction {
        id: Some(42),
        tokens,
        orders,
        liquidity,
        effective_gas_price: gas_price,
        deadline,
        surplus_capturing_jit_order_owners: owners.clone(),
    };

    let cached_tokens = build_tokens(&addresses);
    let cached_orders = sample_orders(&addresses);
    let cached_liquidity = sample_liquidity(&addresses);
    let cached_owners = owners.clone();

    let cached = CachedAuction {
        id: auction.id,
        tokens: cached_tokens
            .into_iter()
            .map(|(address, token)| (address.into(), Cached::new(token)))
            .collect(),
        orders: cached_orders.into_iter().map(Cached::new).collect(),
        liquidity: cached_liquidity.into_iter().map(Cached::new).collect(),
        effective_gas_price: eth::U256::from(gas_price),
        deadline,
        surplus_capturing_jit_order_owners: cached_owners.into_iter().map(Into::into).collect(),
    };

    (auction, cached)
}

fn sample_addresses() -> Vec<H160> {
    (0..32).map(|i| H160::from_low_u64_be(i + 1)).collect()
}

fn build_tokens(addresses: &[H160]) -> HashMap<H160, solver::Token> {
    addresses
        .iter()
        .enumerate()
        .map(|(i, &address)| {
            let mut reference_price = U256::from(10u128.pow(18));
            reference_price = reference_price + U256::from(i as u128 * 1_000);
            (
                address,
                solver::Token {
                    decimals: Some(18),
                    symbol: Some(format!("TKN{i}")),
                    reference_price: Some(reference_price),
                    available_balance: U256::from(10u128.pow(24)),
                    trusted: true,
                },
            )
        })
        .collect()
}

fn sample_orders(tokens: &[H160]) -> Vec<solver::Order> {
    (0..256)
        .map(|i| {
            let sell = tokens[i % tokens.len()];
            let buy = tokens[(i + 1) % tokens.len()];
            solver::Order {
                uid: [i as u8; 56],
                sell_token: sell,
                buy_token: buy,
                sell_amount: U256::from(10_000 + i as u128),
                full_sell_amount: U256::from(20_000 + i as u128),
                buy_amount: U256::from(5_000 + i as u128),
                full_buy_amount: U256::from(6_000 + i as u128),
                fee_policies: Some(vec![FeePolicy::Surplus {
                    factor: 0.1,
                    max_volume_factor: 0.5,
                }]),
                valid_to: 1_700_000_000u32 + i as u32,
                kind: if i % 2 == 0 { Kind::Sell } else { Kind::Buy },
                receiver: Some(H160::from_low_u64_be(1_000 + i as u64)),
                owner: H160::from_low_u64_be(2_000 + i as u64),
                partially_fillable: i % 3 == 0,
                pre_interactions: vec![sample_interaction(i), sample_interaction(i + 1)],
                post_interactions: vec![sample_interaction(i + 2)],
                sell_token_source: SellTokenSource::Erc20,
                buy_token_destination: BuyTokenDestination::Erc20,
                class: if i % 2 == 0 { Class::Market } else { Class::Limit },
                app_data: AppDataHash([i as u8; 32]),
                flashloan_hint: None,
                signing_scheme: SigningScheme::Eip712,
                signature: vec![0x11; 65],
            }
        })
        .collect()
}

fn sample_interaction(seed: usize) -> InteractionData {
    InteractionData {
        target: H160::from_low_u64_be(10_000 + seed as u64),
        value: U256::from(seed as u128),
        call_data: vec![seed as u8; 32],
    }
}

fn sample_liquidity(tokens: &[H160]) -> Vec<Liquidity> {
    let base_token = tokens[0];
    let quote_token = tokens[1];
    let mut constant_tokens = HashMap::new();
    constant_tokens.insert(
        base_token,
        ConstantProductReserve {
            balance: U256::from(1_000_000u128),
        },
    );
    constant_tokens.insert(
        quote_token,
        ConstantProductReserve {
            balance: U256::from(2_000_000u128),
        },
    );

    let mut weighted_tokens = HashMap::new();
    weighted_tokens.insert(
        base_token,
        WeightedProductReserve {
            balance: U256::from(3_000_000u128),
            scaling_factor: BigDecimal::new(1.into(), 0),
            weight: BigDecimal::new(5.into(), 1),
        },
    );
    weighted_tokens.insert(
        quote_token,
        WeightedProductReserve {
            balance: U256::from(4_000_000u128),
            scaling_factor: BigDecimal::new(1.into(), 0),
            weight: BigDecimal::new(5.into(), 1),
        },
    );

    let mut stable_tokens = HashMap::new();
    stable_tokens.insert(
        base_token,
        StableReserve {
            balance: U256::from(5_000_000u128),
            scaling_factor: BigDecimal::new(1.into(), 0),
        },
    );
    stable_tokens.insert(
        quote_token,
        StableReserve {
            balance: U256::from(6_000_000u128),
            scaling_factor: BigDecimal::new(1.into(), 0),
        },
    );

    vec![
        Liquidity::ConstantProduct(ConstantProductPool {
            id: "constant".into(),
            address: H160::from_low_u64_be(123),
            router: H160::from_low_u64_be(456),
            gas_estimate: U256::from(210_000u128),
            tokens: constant_tokens,
            fee: BigDecimal::new(3.into(), 3),
        }),
        Liquidity::WeightedProduct(WeightedProductPool {
            id: "weighted".into(),
            address: H160::from_low_u64_be(789),
            balancer_pool_id: H256::from_low_u64_be(1024),
            gas_estimate: U256::from(310_000u128),
            tokens: weighted_tokens,
            fee: BigDecimal::new(25.into(), 4),
            version: WeightedProductVersion::V3Plus,
        }),
        Liquidity::Stable(StablePool {
            id: "stable".into(),
            address: H160::from_low_u64_be(159),
            balancer_pool_id: H256::from_low_u64_be(753),
            gas_estimate: U256::from(410_000u128),
            tokens: stable_tokens,
            amplification_parameter: BigDecimal::new(85.into(), 2),
            fee: BigDecimal::new(15.into(), 4),
        }),
    ]
}

fn sample_owners() -> Vec<H160> {
    (0..8)
        .map(|i| H160::from_low_u64_be(50_000 + i as u64))
        .collect()
}
