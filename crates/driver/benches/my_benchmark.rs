use {
    criterion::{Criterion, criterion_group, criterion_main},
    driver::infra::api::routes::solve::dto::SolveRequest,
    serde::Deserialize,
    serde_with::serde_as,
    std::hint::black_box,
};

// 6.3ms
// Even this should be offloaded to a blocking thread!!
fn parse_id(auction: &str) -> i64 {
    #[serde_as]
    #[derive(Deserialize)]
    struct Partial {
        #[serde_as(as = "serde_with::DisplayFromStr")]
        id: i64,
    }
    let parsed: Partial = serde_json::from_str(auction).unwrap();
    parsed.id
}

// 23.3ms
fn parse_dto(auction: &str) -> i64 {
    let parsed: SolveRequest = serde_json::from_str(auction).unwrap();
    parsed.id()
}

// 25.8ms
fn parse_json(auction: &str) -> i64 {
    let json: serde_json::Value = serde_json::from_str(auction).unwrap();
    json["id"].as_str().unwrap().parse().unwrap()
}

// 350µs
fn full_equality(auction_1: &str, auction_2: &str) -> bool {
    auction_1 == auction_2
}

// 5ns
fn partial_equality(auction_1: &str, auction_2: &str) -> bool {
    auction_1[..100] == auction_2[..100]
}

// with this info I should do this order:
// 1. full partial eq (cheap enough that we don't need partial eq)
// 2. parse_id
// 3. parse_full

fn criterion_benchmark(c: &mut Criterion) {
    let auction_bytes = std::fs::read("/Users/martin/Downloads/auction_11301102.json").unwrap();
    let auction_str = String::from_utf8(auction_bytes).unwrap();
    c.bench_function("parse id", |b| b.iter(|| parse_id(black_box(&auction_str))));
    c.bench_function("parse dto", |b| {
        b.iter(|| parse_dto(black_box(&auction_str)))
    });
    c.bench_function("parse json", |b| {
        b.iter(|| parse_json(black_box(&auction_str)))
    });
    c.bench_function("full_equality", |b| {
        b.iter(|| full_equality(black_box(&auction_str), black_box(&auction_str)))
    });
    c.bench_function("partial_equality", |b| {
        b.iter(|| partial_equality(black_box(&auction_str), black_box(&auction_str)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
