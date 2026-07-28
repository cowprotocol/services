use {
    super::AppState,
    axum::{
        Json,
        extract::State,
        response::{
            IntoResponse,
            Response,
            sse::{Event, KeepAlive, Sse},
        },
    },
    futures::StreamExt,
    model::quote::OrderQuoteRequest,
    std::{convert::Infallible, sync::Arc},
};

pub async fn post_quote_stream_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OrderQuoteRequest>,
) -> Response {
    let stream = match state.quotes.calculate_quote_stream(&request).await {
        Ok(stream) => stream,
        // Validation or prelude failure: return same HTTP error as POST /api/v1/quote.
        Err(err) => return err.into_response(),
    };

    let events = stream.filter_map(|item| async move {
        let event = match item {
            Ok(response) => Event::default().json_data(&response),
            // Terminal error from the domain: frame it as an SSE error event.
            Err(err) => axum::body::to_bytes(err.into_response().into_body(), usize::MAX)
                .await
                .map(|bytes| {
                    Event::default()
                        .event("error")
                        .data(String::from_utf8_lossy(&bytes))
                }),
        };
        event
            .inspect_err(|err| tracing::error!(?err, "failed to build SSE event"))
            .ok()
            .map(Ok::<_, Infallible>)
    });

    Sse::new(events)
        .keep_alive(KeepAlive::default())
        .into_response()
}
