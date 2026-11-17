//! `GET /api/v1/canonical_batch_schedule`
//!
//! UNSTABLE. Exists for the CoW Swap frontend to show when orders placed with
//! `enableWaitForCow` get their chance to trade. Its path, response shape and
//! semantics can change without notice.

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
struct CanonicalBatchSchedule {
    /// Seconds between two canonical batch times.
    interval_seconds: u32,
    /// The next canonical batch time, in Unix seconds.
    next_batch: u32,
    /// Seconds until `next_batch` by the server's clock, so a countdown
    /// doesn't depend on the client's clock.
    seconds_until_next_batch: u32,
}

impl CanonicalBatchSchedule {
    fn new(schedule: &Schedule, now: u32) -> Self {
        let next_batch = schedule.next_batch(now);
        Self {
            interval_seconds: schedule.interval(),
            next_batch,
            seconds_until_next_batch: next_batch - now,
        }
    }
}

pub async fn get_canonical_batch_schedule_handler(State(state): State<Arc<AppState>>) -> Response {
    let Some(schedule) = &state.canonical_batch_schedule else {
        return (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "canonical batches are not enabled"),
        )
            .into_response();
    };
    Json(CanonicalBatchSchedule::new(
        schedule,
        now_in_epoch_seconds(),
    ))
    .into_response()
}

#[cfg(test)]
mod tests {
    use {super::*, std::num::NonZeroU32};

    #[test]
    fn reports_the_next_batch() {
        let schedule = Schedule::new(NonZeroU32::new(60).unwrap());

        assert_eq!(
            CanonicalBatchSchedule::new(&schedule, 125),
            CanonicalBatchSchedule {
                interval_seconds: 60,
                next_batch: 180,
                seconds_until_next_batch: 55,
            }
        );
        assert_eq!(
            CanonicalBatchSchedule::new(&schedule, 180),
            CanonicalBatchSchedule {
                interval_seconds: 60,
                next_batch: 180,
                seconds_until_next_batch: 0,
            }
        );
    }

    #[test]
    fn serializes_camel_case() {
        let schedule = Schedule::new(NonZeroU32::new(60).unwrap());
        assert_eq!(
            serde_json::to_value(CanonicalBatchSchedule::new(&schedule, 125)).unwrap(),
            serde_json::json!({
                "intervalSeconds": 60,
                "nextBatch": 180,
                "secondsUntilNextBatch": 55,
            })
        );
    }
}
