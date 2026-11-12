//! `GET /api/v1/wait_for_cow/next_batch`
//!
//! UNSTABLE. This endpoint exists for the CoW Swap frontend to show when a
//! `waitForCow` order is expected to become solvable. Its path, response
//! shape and semantics can change or disappear without notice; external
//! projects must not rely on it.

use {
    crate::api::AppState,
    axum::{extract::State, response::Json},
    model::time::now_in_epoch_seconds,
    serde::Serialize,
    std::{sync::Arc, time::Duration},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NextBatch {
    /// Unix seconds of the next batch-auction window edge: the `validFrom`
    /// a `waitForCow` order placed now is expected to be assigned.
    pub next_valid_from: u32,
    /// Window length in seconds.
    pub batch_auction_interval: u64,
}

/// Mirrors the autopilot's assignment (`wait_for_cow_valid_from` in
/// `crates/autopilot/src/solvable_orders.rs`): the next multiple of
/// `interval` at or after `now`.
pub fn next_batch(now: u32, interval: Duration) -> NextBatch {
    let secs = u32::try_from(interval.as_secs()).unwrap_or(u32::MAX).max(1);
    NextBatch {
        next_valid_from: now.div_ceil(secs).saturating_mul(secs),
        batch_auction_interval: interval.as_secs(),
    }
}

pub async fn get_wait_for_cow_next_batch_handler(
    State(state): State<Arc<AppState>>,
) -> Json<NextBatch> {
    Json(next_batch(
        now_in_epoch_seconds(),
        state.batch_auction_interval,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_batch_rounds_up_to_the_window_edge() {
        let interval = Duration::from_secs(300);
        let at = |now| next_batch(now, interval).next_valid_from;

        // Inside a window: its end.
        assert_eq!(at(301), 600);
        assert_eq!(at(599), 600);
        // Exactly on an edge: that edge.
        assert_eq!(at(600), 600);
        // Next window, next edge.
        assert_eq!(at(601), 900);
        // Degenerate intervals do not panic.
        assert_eq!(next_batch(1234, Duration::ZERO).next_valid_from, 1234);
        assert_eq!(next_batch(u32::MAX - 1, interval).next_valid_from, u32::MAX);
    }

    #[test]
    fn next_batch_serializes_camel_case() {
        let json = serde_json::to_value(next_batch(301, Duration::from_secs(300))).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "nextValidFrom": 600, "batchAuctionInterval": 300 })
        );
    }
}
