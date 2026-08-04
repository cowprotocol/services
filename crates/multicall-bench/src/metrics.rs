//! Reads the RPC counters that `ethrpc`'s instrumentation layer already emits,
//! so that every timing comes with the number of calls that produced it.
//!
//! The layer sits *above* the batching layer, so these are logical JSON-RPC
//! calls as the caller made them, not HTTP round-trips — contrary to what the
//! `requests_duration_seconds` doc comment in `ethrpc` claims.
//!
//! The registry is scraped through its text encoding rather than the protobuf
//! model: the encoding is stable across `prometheus` releases, the model is
//! not.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Completed logical calls per RPC method.
    pub requests: BTreeMap<String, u64>,
    /// Total time spent in requests, summed over all of them. Requests run
    /// concurrently, so this is much larger than the wall clock.
    pub duration_sum: f64,
    pub duration_count: u64,
}

impl Snapshot {
    pub fn take() -> Self {
        let text = observe::metrics::encode(observe::metrics::get_registry());
        let mut snapshot = Self::default();

        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            let Some((labeled_name, value)) = line.rsplit_once(' ') else {
                continue;
            };
            let (name, labels) = match labeled_name.split_once('{') {
                Some((name, labels)) => (name, labels.trim_end_matches('}')),
                None => (labeled_name, ""),
            };

            if name.ends_with("requests_complete") {
                let method = label(labels, "method").unwrap_or("unknown").to_owned();
                let count = value.parse::<f64>().unwrap_or_default() as u64;
                *snapshot.requests.entry(method).or_default() += count;
            } else if name.ends_with("requests_duration_seconds_sum") {
                snapshot.duration_sum += value.parse::<f64>().unwrap_or_default();
            } else if name.ends_with("requests_duration_seconds_count") {
                snapshot.duration_count += value.parse::<f64>().unwrap_or_default() as u64;
            }
        }

        snapshot
    }

    /// The requests that happened between `self` and a later snapshot.
    pub fn delta(&self, later: &Self) -> Self {
        let mut requests = BTreeMap::new();
        for (method, count) in &later.requests {
            let delta = count.saturating_sub(self.requests.get(method).copied().unwrap_or(0));
            if delta > 0 {
                requests.insert(method.clone(), delta);
            }
        }

        Self {
            requests,
            duration_sum: later.duration_sum - self.duration_sum,
            duration_count: later.duration_count.saturating_sub(self.duration_count),
        }
    }

    pub fn total_requests(&self) -> u64 {
        self.requests.values().sum()
    }

    /// Mean time a single node round-trip took. Together with the wall clock
    /// this separates "fewer requests" from "slower requests".
    pub fn mean_request_seconds(&self) -> f64 {
        if self.duration_count == 0 {
            return 0.0;
        }
        self.duration_sum / self.duration_count as f64
    }

    pub fn methods(&self) -> String {
        self.requests
            .iter()
            .map(|(method, count)| format!("{method}={count}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn label<'a>(labels: &'a str, key: &str) -> Option<&'a str> {
    labels.split(',').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"'))
    })
}
