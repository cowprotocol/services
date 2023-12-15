use {
    crate::orderbook::Orderbook,
    anyhow::Result,
    database::orders::Quote,
    model::order::OrderUid,
    std::{convert::Infallible, sync::Arc},
    warp::{hyper::StatusCode, reply, Filter, Rejection},
};

pub fn get_quote_request() -> impl Filter<Extract = (OrderUid,), Error = Rejection> + Clone {
    warp::path!("v1" / "orders" / OrderUid / "quote").and(warp::get())
}

pub fn get_quote_response(result: Result<Option<Quote>>) -> super::ApiReply {
    let quote = match result {
        Ok(order) => order,
        Err(err) => {
            tracing::error!(?err, "get_quote_response");
            return shared::api::internal_error_reply();
        }
    };
    match quote {
        Some(quote) => reply::with_status(reply::json(&quote), StatusCode::OK),
        None => reply::with_status(
            super::error("NotFound", "Quote was not found"),
            StatusCode::NOT_FOUND,
        ),
    }
}

pub fn get(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_quote_request().and_then(move |uid| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_quote(&uid).await;
            Result::<_, Infallible>::Ok(get_quote_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        shared::api::response_body,
        warp::{test::request, Reply},
    };

    #[tokio::test]
    async fn get_order_by_uid_request_ok() {
        let uid = OrderUid::default();
        let request = request().path(&format!("/v1/orders/{uid}")).method("GET");
        let filter = get_order_by_uid_request();
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, uid);
    }

    #[tokio::test]
    async fn get_order_by_uid_response_ok() {
        let order = Order::default();
        let response = get_order_by_uid_response(Ok(Some(order.clone()))).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_order: Order = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_order, order);
    }

    #[tokio::test]
    async fn get_order_by_uid_response_non_existent() {
        let response = get_order_by_uid_response(Ok(None)).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
