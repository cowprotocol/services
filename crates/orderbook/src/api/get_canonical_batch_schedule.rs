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
    configs::canonical_batch::CanonicalBatchInterval,
    model::time::now_in_epoch_seconds,
    serde::Serialize,
    std::{sync::Arc, time::Duration},
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
    /// The smallest `validTo`, in Unix seconds, that a wait-for-CoW order
    /// should use.
    next_batch_valid_to: u32,
}

impl CanonicalBatchSchedule {
    fn new(
        interval: CanonicalBatchInterval,
        min_order_validity_period: Duration,
        now: u32,
    ) -> Self {
        let next_batch = interval.next_batch(now);
        let min_validity = u32::try_from(min_order_validity_period.as_secs()).unwrap_or(u32::MAX);
        Self {
            interval_seconds: interval.seconds(),
            next_batch,
            seconds_until_next_batch: next_batch - now,
            next_batch_valid_to: next_batch
                .saturating_add(interval.seconds())
                .saturating_add(min_validity),
        }
    }
}

pub async fn get_canonical_batch_schedule_handler(State(state): State<Arc<AppState>>) -> Response {
    let Some(interval) = state.canonical_batch_interval else {
        return (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "canonical batches are not enabled"),
        )
            .into_response();
    };
    Json(CanonicalBatchSchedule::new(
        interval,
        state.min_order_validity_period,
        now_in_epoch_seconds(),
    ))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_VALIDITY: Duration = Duration::from_secs(90);

    fn interval() -> CanonicalBatchInterval {
        Duration::from_secs(60).try_into().unwrap()
    }

    #[test]
    fn reports_the_next_batch() {
        assert_eq!(
            CanonicalBatchSchedule::new(interval(), MIN_VALIDITY, 125),
            CanonicalBatchSchedule {
                interval_seconds: 60,
                next_batch: 180,
                seconds_until_next_batch: 55,
                next_batch_valid_to: 330,
            }
        );
        assert_eq!(
            CanonicalBatchSchedule::new(interval(), MIN_VALIDITY, 180),
            CanonicalBatchSchedule {
                interval_seconds: 60,
                next_batch: 180,
                seconds_until_next_batch: 0,
                next_batch_valid_to: 330,
            }
        );
    }

    #[test]
    fn saturates_next_batch_valid_to() {
        assert_eq!(
            CanonicalBatchSchedule::new(interval(), Duration::MAX, 125).next_batch_valid_to,
            u32::MAX
        );
    }

    #[test]
    fn serializes_camel_case() {
        assert_eq!(
            serde_json::to_value(CanonicalBatchSchedule::new(interval(), MIN_VALIDITY, 125))
                .unwrap(),
            serde_json::json!({
                "intervalSeconds": 60,
                "nextBatch": 180,
                "secondsUntilNextBatch": 55,
                "nextBatchValidTo": 330,
            })
        );
    }
}
