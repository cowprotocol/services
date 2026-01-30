use {
    crate::{
        api::{error, extract_payload},
        orderbook::Orderbook,
    },
    anyhow::Result,
    model::order::{Order, OrderUid},
    std::{convert::Infallible, sync::Arc},
    warp::{Filter, Rejection, hyper::StatusCode, reply},
};

const MAX_ORDERS_LIMIT: usize = 5000;

#[derive(Debug, Eq, PartialEq)]
enum ValidationError {
    TooManyOrders(usize),
}

fn validate(uids: Vec<OrderUid>) -> Result<Vec<OrderUid>, ValidationError> {
    if uids.len() > MAX_ORDERS_LIMIT {
        return Err(ValidationError::TooManyOrders(uids.len()));
    }
    Ok(uids)
}

fn get_orders_by_uid_request()
-> impl Filter<Extract = (Result<Vec<OrderUid>, ValidationError>,), Error = Rejection> + Clone {
    warp::path!("v1" / "orders" / "lookup")
        .and(warp::post())
        .and(extract_payload())
        .map(|uids: Vec<OrderUid>| validate(uids))
}

pub fn get_orders_by_uid_response(result: Result<Vec<Order>>) -> super::ApiReply {
    let orders = match result {
        Ok(orders) => orders,
        Err(err) => {
            tracing::error!(?err, "get_orders_by_uids_response");
            return crate::api::internal_error_reply();
        }
    };
    reply::with_status(reply::json(&orders), StatusCode::OK)
}

pub fn get_orders_by_uid(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_orders_by_uid_request().and_then(
        move |request_result: Result<Vec<OrderUid>, ValidationError>| {
            let orderbook = orderbook.clone();
            async move {
                Result::<_, Infallible>::Ok(match request_result {
                    Ok(uids) => {
                        let result = orderbook.get_orders(&uids).await;
                        get_orders_by_uid_response(result)
                    }
                    Err(ValidationError::TooManyOrders(requested)) => {
                        let err = error(
                            "TooManyOrders",
                            format!(
                                "Too many order UIDs requested: {requested}. Maximum allowed: \
                                 {MAX_ORDERS_LIMIT}"
                            ),
                        );
                        reply::with_status(err, StatusCode::BAD_REQUEST)
                    }
                })
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use {super::*, warp::test::request};

    #[tokio::test]
    async fn get_orders_by_uid_request_ok() {
        let uid = OrderUid::default();
        let request = request()
            .path("/v1/orders/lookup")
            .method("POST")
            .header("content-type", "application-json")
            .json(&[uid]);

        let filter = get_orders_by_uid_request();
        let result = request.filter(&filter).await.unwrap().unwrap();
        assert_eq!(result, [uid]);
    }

    #[tokio::test]
    async fn get_orders_by_uid_request_too_many_orders() {
        let mut uids = Vec::new();
        for _ in 0..5001 {
            uids.push(OrderUid::default());
        }
        let request = request()
            .path("/v1/orders/lookup")
            .method("POST")
            .header("content-type", "application-json")
            .json(&uids);

        let filter = get_orders_by_uid_request();
        let result = request.filter(&filter).await;
        // Assert that the error is a rejection.
        assert!(result.is_err());
    }
}
