//! `GET /api/v1/canonical_batch_schedule`
//!
//! UNSTABLE. Exists for the CoW Swap frontend to show when orders placed with
//! `enableWaitForCow` get their next chance to trade. Its path, response shape
//! and semantics can change without notice.

use {
    crate::api::AppState,
    axum::{
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::time::now_in_epoch_seconds,
    serde::Serialize,
    shared::canonical_batch::Schedule,
    std::sync::Arc,
};

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalBatchSchedule {
    /// Seconds between two canonical batch times.
    pub interval_seconds: u32,
    /// Orders created less than this many seconds before a batch time join
    /// the batch after it.
    pub cutoff_seconds: u32,
    /// The first batch time after `server_time`, in Unix seconds.
    pub next_batch: u32,
    /// The server's current time, in Unix seconds, so a countdown isn't
    /// thrown off by the client's clock.
    pub server_time: u32,
}

impl CanonicalBatchSchedule {
    fn new(schedule: &Schedule, now: u32) -> Self {
        Self {
            interval_seconds: schedule.interval(),
            cutoff_seconds: schedule.cutoff(),
            next_batch: schedule.next_batch(now),
            server_time: now,
        }
    }
}

pub async fn get_canonical_batch_schedule_handler(State(state): State<Arc<AppState>>) -> Response {
    match state.canonical_batch_schedule.schedule().await {
        Ok(Some(schedule)) => Json(CanonicalBatchSchedule::new(
            &schedule,
            now_in_epoch_seconds(),
        ))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "Canonical batches are off."),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, "/api/v1/canonical_batch_schedule");
            crate::api::internal_error_reply()
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::num::NonZeroU32};

    #[test]
    fn reports_the_next_batch_after_now() {
        let schedule = Schedule::new(NonZeroU32::new(60).unwrap(), 5).unwrap();

        assert_eq!(
            CanonicalBatchSchedule::new(&schedule, 125),
            CanonicalBatchSchedule {
                interval_seconds: 60,
                cutoff_seconds: 5,
                next_batch: 180,
                server_time: 125,
            }
        );
        // Exactly on a batch time: that batch has started, report the next.
        assert_eq!(CanonicalBatchSchedule::new(&schedule, 120).next_batch, 180);
    }

    #[test]
    fn serializes_camel_case() {
        let schedule = Schedule::new(NonZeroU32::new(60).unwrap(), 5).unwrap();
        assert_eq!(
            serde_json::to_value(CanonicalBatchSchedule::new(&schedule, 125)).unwrap(),
            serde_json::json!({
                "intervalSeconds": 60,
                "cutoffSeconds": 5,
                "nextBatch": 180,
                "serverTime": 125,
            })
        );
    }
}
