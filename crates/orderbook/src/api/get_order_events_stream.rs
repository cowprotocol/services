use {
    super::{AppState, error},
    axum::{
        extract::{Path, State},
        http::{HeaderName, StatusCode},
        response::{
            IntoResponse,
            Response,
            sse::{Event, KeepAlive, Sse},
        },
    },
    futures::StreamExt,
    model::order::OrderUid,
    rand::Rng,
    std::{convert::Infallible, ops::RangeInclusive, sync::Arc, time::Duration},
};

/// Tells proxies like nginx to forward every event at once.
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");

/// Clients wait a random time in this range before they reconnect, so that
/// they do not all come back in the same moment after a restart.
const RECONNECT_DELAY_MILLIS: RangeInclusive<u64> = 1_000..=5_000;

pub async fn get_order_events_stream_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
) -> Response {
    let Some(order_event_stream) = &state.order_event_stream else {
        let err = error("NotFound", "the order event stream is not enabled");
        return (StatusCode::NOT_FOUND, err).into_response();
    };
    let order_events = match order_event_stream.open(uid).await {
        Ok(Some(order_events)) => order_events,
        // An `EventSource` client does not reconnect after this status.
        Ok(None) => return StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            let err = error("ServiceUnavailable", err.to_string());
            return (StatusCode::SERVICE_UNAVAILABLE, err).into_response();
        }
    };

    let reconnect_delay = Duration::from_millis(rand::rng().random_range(RECONNECT_DELAY_MILLIS));
    let events = futures::stream::once(async move { Event::default().retry(reconnect_delay) })
        .chain(order_events.filter_map(|event| async move {
            Event::default()
                .json_data(&event)
                .inspect_err(|err| tracing::error!(?err, "failed to build SSE event"))
                .ok()
        }))
        .map(Ok::<_, Infallible>);

    (
        [(X_ACCEL_BUFFERING, "no")],
        Sse::new(events).keep_alive(KeepAlive::default()),
    )
        .into_response()
}
