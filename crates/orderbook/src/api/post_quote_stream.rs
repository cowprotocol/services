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
                Err(err) => tracing::debug!(%err, "dropping failed streamed quote"),
            }
        }
        if !any_ok {
            // No solver produced a usable quote. Signal the same "no route found"
            // body the regular endpoint returns, routed through the shared mapping.
            let (_, Json(body)) = super::price_estimation_error_response(PriceEstimationError::NoLiquidity);
            let event = Event::default()
                .event("error")
                .json_data(&body)
                .expect("static error body serializes");
            yield Ok::<_, Infallible>(event);
        }
    };

    Sse::new(events)
        .keep_alive(KeepAlive::default())
        .into_response()
}
