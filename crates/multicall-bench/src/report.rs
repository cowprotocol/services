//! All of the benchmark's output. Kept apart from `bench` so that reviewing the
//! measured code does not mean reading format strings.

use {
    crate::{
        bench::Setup,
        fixture::Fixture,
        results::{Measurement, Pass},
    },
    alloy_primitives::U256,
    anyhow::Result,
    std::{path::Path, time::Duration},
};

const COLUMNS: usize = 14;
const HEADERS: [&str; COLUMNS] = [
    "mc",
    "rpc_batch",
    "conc",
    "delay",
    "min_ms",
    "med_ms",
    "max_ms",
    "calls",
    "http",
    "fill",
    "call_ms",
    "err",
    "mism",
    "moved",
];
const WIDTHS: [usize; COLUMNS] = [4, 9, 4, 5, 7, 7, 7, 6, 6, 5, 7, 5, 5, 5];

const LEGEND: &str = "\
mc      queries per Multicall3 call; 0 reads them individually and is the parity baseline
calls   logical JSON-RPC calls per pass, counted above the ethrpc batching layer
http    HTTP round-trips, counted below it: one per packet it sent, plus one per call that
        bypassed it entirely
fill    calls/http, the measured batching ratio; 1.0 means nothing was coalesced
call_ms mean duration of one logical call; for batched calls this spans the whole HTTP request
mism    results differing from the baseline config
moved   results differing between this config's own first and last pass — same code path, so
        this is on-chain movement and the noise floor for mism";

pub fn working_set(fixture: &Fixture, queries: usize) {
    let (tokens, owners) = fixture.diversity();
    println!(
        "working set: {queries} pairs, {tokens} distinct tokens, {owners} distinct owners \
         (fixture network {}, dumped {})",
        fixture.network, fixture.dumped_at,
    );
}

pub fn setup(setup: &Setup, fixture: &Fixture) {
    match setup.chain() {
        Some(chain) if chain.as_str() != fixture.network => println!(
            "WARNING: node is {} but the fixture was dumped from {} — the pairs are meaningless \
             on this chain",
            chain.as_str(),
            fixture.network,
        ),
        Some(_) => (),
        None => println!("WARNING: unknown chain ID {}", setup.chain_id),
    }

    match setup.block {
        Some(number) => println!(
            "pinned to block {number} ({} behind latest {})",
            setup.latest_block.saturating_sub(number),
            setup.latest_block,
        ),
        None => println!("not pinned: reads follow the chain, as production does"),
    }

    println!("vault relayer {}", setup.vault_relayer());
}

pub fn warmup(pass: usize, of: usize, elapsed: Duration, ok: usize, total: usize) {
    println!("warmup {pass}/{of}: {elapsed:?}, {ok}/{total} ok");
}

/// A line per config as it finishes, because a full matrix takes minutes.
pub fn progress(measurement: &Measurement) {
    let (min, med, max) = measurement.wall_ms();
    println!(
        "mc={} rpc_batch={} conc={} delay={}ms  wall {min}/{med}/{max} ms  calls={:.0}  \
         http={:.0}  fill={:.1}  call={:.1}ms",
        measurement.config.multicall_batch_size,
        measurement.config.ethrpc_batch_size,
        measurement.config.ethrpc_concurrency,
        measurement.config.ethrpc_batch_delay_ms,
        measurement.mean(|pass| pass.calls as f64),
        measurement.mean(|pass| pass.http as f64),
        measurement.mean(|pass| pass.batch_fill),
        measurement.mean(|pass| pass.mean_call_ms),
    );
}

pub fn table(measurements: &[Measurement]) {
    println!();
    println!("{}", row(&HEADERS.map(str::to_owned)));
    println!("{}", row(&WIDTHS.map(|width| "─".repeat(width))));
    for measurement in measurements {
        let (min, med, max) = measurement.wall_ms();
        println!(
            "{}",
            row(&[
                measurement.config.multicall_batch_size.to_string(),
                measurement.config.ethrpc_batch_size.to_string(),
                measurement.config.ethrpc_concurrency.to_string(),
                measurement.config.ethrpc_batch_delay_ms.to_string(),
                min.to_string(),
                med.to_string(),
                max.to_string(),
                format!("{:.0}", measurement.mean(|pass| pass.calls as f64)),
                format!("{:.0}", measurement.mean(|pass| pass.http as f64)),
                format!("{:.1}", measurement.mean(|pass| pass.batch_fill)),
                format!("{:.1}", measurement.mean(|pass| pass.mean_call_ms)),
                format!("{:.0}", measurement.mean(|pass| pass.err as f64)),
                optional(measurement.parity_mismatches),
                optional(measurement.volatile),
            ])
        );
    }
    println!("\n{LEGEND}");
    batching(measurements);
    examples(measurements);
}

/// Whether the two batching mechanisms did what the config asked for.
///
/// The table's `fill` column already shows the ratio, but a ratio alone cannot
/// tell "the layer coalesced nothing" apart from "the calls never reached the
/// layer". These counters can, so the verdict is stated rather than inferred.
fn batching(measurements: &[Measurement]) {
    println!("\nbatching, measured below the ethrpc batching layer:");
    for measurement in measurements {
        let config = &measurement.config;
        let calls = measurement.mean(|pass| pass.calls as f64);
        let batched = measurement.mean(|pass| pass.batched as f64);
        let unbatched = measurement.mean(|pass| pass.unbatched as f64);
        let fill = measurement.mean(|pass| pass.batch_fill);

        // Only the last pass's breakdown: every pass runs the same code path,
        // so they all agree, and one line per config stays readable.
        let per_method = measurement
            .passes
            .last()
            .map(|pass| pass.batching.as_str())
            .unwrap_or_default();

        println!(
            "  mc={:<4} rpc_batch={:<4} {:>6.0} of {:>6.0} calls batched into {:>5.0} packets \
             (fill {fill:.1}) — {}",
            config.multicall_batch_size,
            config.ethrpc_batch_size,
            batched,
            calls,
            measurement.mean(|pass| pass.http as f64),
            verdict(config.ethrpc_batch_size, batched, unbatched, fill),
        );
        if !per_method.is_empty() {
            println!("       batched/total per method: {per_method}");
        }
    }
}

/// Reads the counters against what the config asked for. `batch_size <= 1`
/// removes the batching layer altogether, so for those configs no batching is
/// the correct outcome and any batching would mean the wrong provider was used.
fn verdict(batch_size: usize, batched: f64, unbatched: f64, fill: f64) -> String {
    if batch_size <= 1 {
        return if batched > 0.0 {
            format!("UNEXPECTED: layer is disabled but carried {batched:.0} calls")
        } else {
            "layer disabled, one HTTP request per call as configured".to_owned()
        };
    }
    if batched == 0.0 {
        return "NOT BATCHED: no call reached the batching layer".to_owned();
    }
    if unbatched > 0.0 {
        return format!("PARTIAL: {unbatched:.0} calls went around the batching layer");
    }
    if fill < 1.5 {
        return "batched, but packets held ~1 call — too few calls in flight to coalesce"
            .to_owned();
    }
    format!("batched, {fill:.1} calls per HTTP request of at most {batch_size}")
}

fn examples(measurements: &[Measurement]) {
    for measurement in measurements {
        if measurement.examples.is_empty() {
            continue;
        }
        println!(
            "\nmc={} disagrees with the baseline on:",
            measurement.config.multicall_batch_size,
        );
        for example in &measurement.examples {
            println!(
                "  owner {} token {}  baseline {}  actual {}",
                example.owner,
                example.token,
                value(example.baseline),
                value(example.actual),
            );
        }
    }
}

pub fn write_files(
    measurements: &[Measurement],
    json: Option<&Path>,
    csv_path: Option<&Path>,
) -> Result<()> {
    if let Some(path) = json {
        std::fs::write(path, serde_json::to_vec_pretty(measurements)?)?;
        println!("wrote {}", path.display());
    }
    if let Some(path) = csv_path {
        std::fs::write(path, csv(measurements))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn csv(measurements: &[Measurement]) -> String {
    let mut out = String::from(
        "multicall_batch_size,ethrpc_batch_size,ethrpc_concurrency,ethrpc_batch_delay_ms,pass,\
         wall_ms,calls,http,batched,unbatched,batch_fill,mean_call_ms,ok,err,parity_mismatches\n",
    );
    for measurement in measurements {
        for (index, pass) in measurement.passes.iter().enumerate() {
            let Pass {
                wall_ms,
                calls,
                http,
                batched,
                unbatched,
                batch_fill,
                mean_call_ms,
                ok,
                err,
                ..
            } = pass;
            out.push_str(&format!(
                "{},{},{},{},{index},{wall_ms},{calls},{http},{batched},{unbatched},{batch_fill:.\
                 3},{mean_call_ms:.3},{ok},{err},{}\n",
                measurement.config.multicall_batch_size,
                measurement.config.ethrpc_batch_size,
                measurement.config.ethrpc_concurrency,
                measurement.config.ethrpc_batch_delay_ms,
                measurement
                    .parity_mismatches
                    .map_or(String::new(), |count| count.to_string()),
            ));
        }
    }
    out
}

fn row(cells: &[String; COLUMNS]) -> String {
    let mut out = String::new();
    for (cell, width) in cells.iter().zip(WIDTHS) {
        out.push_str(&format!("{cell:>width$}  "));
    }
    out.trim_end().to_owned()
}

fn optional(value: Option<usize>) -> String {
    value.map_or("-".to_owned(), |value| value.to_string())
}

fn value(balance: Option<U256>) -> String {
    balance.map_or("failed".to_owned(), |balance| balance.to_string())
}
