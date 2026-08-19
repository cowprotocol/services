//! Reads the RPC counters that `ethrpc`'s layers already emit, so that every
//! timing comes with the number of calls that produced it — and with proof of
//! what the batching layer did with them.
//!
//! Two independent layers are scraped, because they answer different
//! questions:
//!
//! * The instrumentation layer sits *above* the batching layer, so
//!   [`Snapshot::requests`] counts logical JSON-RPC calls as the caller made
//!   them. That is what `Multicall3` aggregation shrinks.
//! * The batching layer counts the packets it hands to the transport, so
//!   [`Snapshot::batches`] and [`Snapshot::batched`] count HTTP round-trips and
//!   the calls that went into them. That is what JSON-RPC batching shrinks.
//!
//! A call the batching layer never saw is missing from [`Snapshot::batched`]
//! while still being present in [`Snapshot::requests`], which is what makes
//! "did this actually get batched?" answerable rather than assumed.
//!
//! The registry is scraped through its text encoding rather than the protobuf
//! model: the encoding is stable across `prometheus` releases, the model is
//! not.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Completed logical calls per RPC method, counted above the batching
    /// layer.
    pub requests: BTreeMap<String, u64>,
    /// Total time spent in requests, summed over all of them. Requests run
    /// concurrently, so this is much larger than the wall clock.
    pub duration_sum: f64,
    pub duration_count: u64,
    /// Packets the batching layer sent, i.e. HTTP round-trips it caused.
    pub batches: u64,
    /// Calls per method that the batching layer put into those packets.
    pub batched: BTreeMap<String, u64>,
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
            let count = || value.parse::<f64>().unwrap_or_default() as u64;
            let method = || label(labels, "method").unwrap_or("unknown").to_owned();

            // Names are matched by suffix because the registry may carry a
            // configurable prefix.
            if name.ends_with("requests_complete") {
                *snapshot.requests.entry(method()).or_default() += count();
            } else if name.ends_with("requests_duration_seconds_sum") {
                snapshot.duration_sum += value.parse::<f64>().unwrap_or_default();
            } else if name.ends_with("requests_duration_seconds_count") {
                snapshot.duration_count += count();
            } else if name.ends_with("ethrpc_batching_batches") {
                snapshot.batches += count();
            } else if name.ends_with("ethrpc_batching_calls") {
                *snapshot.batched.entry(method()).or_default() += count();
            }
        }

        snapshot
    }

    /// The requests that happened between `self` and a later snapshot.
    pub fn delta(&self, later: &Self) -> Self {
        Self {
            requests: delta(&self.requests, &later.requests),
            duration_sum: later.duration_sum - self.duration_sum,
            duration_count: later.duration_count.saturating_sub(self.duration_count),
            batches: later.batches.saturating_sub(self.batches),
            batched: delta(&self.batched, &later.batched),
        }
    }

    pub fn total_requests(&self) -> u64 {
        self.requests.values().sum()
    }

    /// Logical calls that the batching layer put on the wire.
    pub fn total_batched(&self) -> u64 {
        self.batched.values().sum()
    }

    /// Calls that never reached the batching layer, so each one paid for its
    /// own HTTP round-trip. Expected to be everything when the layer is
    /// disabled, and nothing when it is not.
    pub fn unbatched(&self) -> u64 {
        self.total_requests().saturating_sub(self.total_batched())
    }

    /// Measured HTTP round-trips: one per packet the batching layer sent, plus
    /// one for every call that bypassed it.
    pub fn http_requests(&self) -> u64 {
        self.batches + self.unbatched()
    }

    /// Mean calls per HTTP round-trip. `1.0` means nothing was coalesced.
    pub fn batch_fill(&self) -> f64 {
        let http = self.http_requests();
        if http == 0 {
            return 0.0;
        }
        self.total_requests() as f64 / http as f64
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
        join(&self.requests)
    }

    /// Per method, how many of its calls the batching layer carried, as
    /// `method=batched/total`. This is the evidence that a given method — the
    /// `eth_call`s carrying `aggregate3`, above all — really was batched.
    pub fn batching(&self) -> String {
        self.requests
            .iter()
            .map(|(method, total)| {
                let batched = self.batched.get(method).copied().unwrap_or(0);
                format!("{method}={batched}/{total}")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn delta(before: &BTreeMap<String, u64>, after: &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
    after
        .iter()
        .filter_map(|(key, count)| {
            let delta = count.saturating_sub(before.get(key).copied().unwrap_or(0));
            (delta > 0).then(|| (key.clone(), delta))
        })
        .collect()
}

fn join(counts: &BTreeMap<String, u64>) -> String {
    counts
        .iter()
        .map(|(key, count)| format!("{key}={count}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn label<'a>(labels: &'a str, key: &str) -> Option<&'a str> {
    labels.split(',').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"'))
    })
}
