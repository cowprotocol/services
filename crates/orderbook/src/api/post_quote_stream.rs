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
    price_estimation::PriceEstimationError,
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

    let events = async_stream::stream! {
        let mut any_ok = false;
        let mut errors: Vec<String> = Vec::new();
        futures::pin_mut!(stream);
        while let Some(item) = stream.next().await {
            match item {
                Ok(response) => match Event::default().json_data(&response) {
                    Ok(event) => {
                        any_ok = true;
                        yield Ok::<_, Infallible>(event);
                    }
                    Err(err) => tracing::error!(?err, "failed to serialize streamed quote"),
                },
                Err(err) => {
                    tracing::debug!(%err, "dropping failed streamed quote");
                    errors.push(err.to_string());
                }
            }
        }
        if !any_ok {
            // Per-solver errors are dropped above, so log the aggregated reasons
            // once here for operators (the client only sees a generic event).
            tracing::warn!(?errors, "streaming quote produced no usable quote");
            // No solver produced a usable quote. Reuse the regular endpoint's
            // error mapping for NoLiquidity instead of reconstructing the body.
            let response =
                super::PriceEstimationErrorWrapper(PriceEstimationError::NoLiquidity).into_response();
            // The body is our own small JSON error, so reading it fully is safe.
            match axum::body::to_bytes(response.into_body(), usize::MAX).await {
                Ok(bytes) => {
                    yield Ok::<_, Infallible>(
                        Event::default()
                            .event("error")
                            .data(String::from_utf8_lossy(&bytes)),
                    )
                }
                Err(err) => tracing::error!(?err, "failed to read no-quote error event body"),
            }
        }
    };

    Sse::new(events)
        .keep_alive(KeepAlive::default())
        .into_response()
}
