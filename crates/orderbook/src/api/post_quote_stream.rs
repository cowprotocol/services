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
        match item {
            Ok(response) => match Event::default().json_data(&response) {
                Ok(event) => Some(Ok::<_, Infallible>(event)),
                Err(err) => {
                    tracing::error!(?err, "failed to serialize streamed quote");
                    None
                }
            },
            // Terminal error from the domain: frame it as an SSE error event.
            Err(err) => {
                let response = err.into_response();
                match axum::body::to_bytes(response.into_body(), usize::MAX).await {
                    Ok(bytes) => Some(Ok::<_, Infallible>(
                        Event::default()
                            .event("error")
                            .data(String::from_utf8_lossy(&bytes)),
                    )),
                    Err(err) => {
                        tracing::error!(?err, "failed to read error event body");
                        None
                    }
                }
            }
        }
    });

    Sse::new(events)
        .keep_alive(KeepAlive::default())
        .into_response()
}
