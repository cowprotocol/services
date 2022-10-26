use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use ethcontract::U256;
use rand::seq::SliceRandom as _;
use reqwest::Client;
use shared::token_list::{TokenList, TokenListConfiguration};
use tokio::runtime::Runtime;

const TOKEN_LIST: &str = "https://gateway.ipfs.io/ipns/tokens.uniswap.org";
const BASE_URL: &str = "http://localhost:8080/api/v1";

pub fn criterion_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let token_list_configuration = TokenListConfiguration {
        url: TOKEN_LIST.to_owned(),
        chain_id: 1,
        client: Client::new(),
        update_interval: Default::default(),
    };
    let token_list = rt
        .block_on(TokenList::from_configuration(&token_list_configuration))
        .expect("Failed to fetch token list");

    let mut group = c.benchmark_group("e2e API requests");
    group
        .measurement_time(Duration::from_secs(300))
        .bench_function("Estimate Price", |b| {
            b.iter(|| estimate_fee_and_price_estimate(&token_list));
        });
    group.finish();
}

fn estimate_fee_and_price_estimate(token_list: &TokenList) {
    let mut rng = rand::thread_rng();
    let base_token = token_list
        .all()
        .choose(&mut rng)
        .expect("Empty token list")
        .clone();
    let quote_token = token_list
        .all()
        .choose(&mut rng)
        .expect("Empty token list")
        .clone();
    let order_type = &["sell", "buy"].choose(&mut rng).unwrap();
    let amount = U256::exp10(base_token.decimals as usize);
    let estimate_amount = format!(
        "{}/markets/{:#x}-{:#x}/{}/{}",
        BASE_URL, base_token.address, quote_token.address, order_type, amount
    );
    let estimate_fee = format!(
        "{}/fee?sellToken={:#x}&buyToken={:#x}&kind={}&amount={}",
        BASE_URL, base_token.address, quote_token.address, order_type, amount
    );

    for request_url in &[estimate_amount, estimate_fee] {
        let result = reqwest::blocking::get(request_url).expect("Query failed");
        if !result.status().is_success() {
            println!(
                "Request: {}, Status: {}, Response: {}",
                request_url,
                result.status(),
                result.text().expect("No text")
            );
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
