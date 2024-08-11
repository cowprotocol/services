use {
    crate::price_estimation::PriceEstimationError,
    anyhow::Result,
    axum::response::{IntoResponse, Response},
    serde::Serialize,
    std::fmt::Debug,
};

// should we just keep this shit for now and convert to an axum::Response to not
// rewrite everything at once?
// pub type ApiReply = WithStatus<Json>;
pub struct ApiReply {
    pub reply: serde_json::Value,
    pub status: axum::http::StatusCode,
}

impl IntoResponse for ApiReply {
    fn into_response(self) -> Response {
        (self.status, self.reply.to_string()).into_response()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Error<'a> {
    error_type: &'a str,
    description: &'a str,
    /// Additional arbitrary data that can be attached to an API error.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

pub fn error(error_type: &str, description: impl AsRef<str>) -> serde_json::Value {
    serde_json::to_value(&Error {
        error_type,
        description: description.as_ref(),
        data: None,
    })
    .unwrap()
}

pub fn rich_error(
    error_type: &str,
    description: impl AsRef<str>,
    data: impl Serialize,
) -> serde_json::Value {
    let data = match serde_json::to_value(&data) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(?err, "failed to serialize error data");
            None
        }
    };

    serde_json::to_value(&Error {
        error_type,
        description: description.as_ref(),
        data,
    })
    .unwrap()
}

pub fn internal_error_reply() -> ApiReply {
    ApiReply {
        status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        reply: error("InternalServerError", ""),
    }
}

pub fn convert_json_response<T, E>(result: Result<T, E>) -> ApiReply
where
    T: Serialize,
    E: IntoApiReply + Debug,
{
    match result {
        Ok(response) => ApiReply {
            status: axum::http::StatusCode::OK,
            reply: serde_json::to_value(&response).unwrap(),
        },
        Err(err) => err.into_api_reply(),
    }
}

pub trait IntoApiReply {
    fn into_api_reply(self) -> ApiReply;
}

impl IntoApiReply for PriceEstimationError {
    fn into_api_reply(self) -> ApiReply {
        match self {
            Self::UnsupportedToken { token, reason } => ApiReply {
                status: axum::http::StatusCode::BAD_REQUEST,
                reply: error(
                    "UnsupportedToken",
                    format!("Token {token:?} is unsupported: {reason:}"),
                ),
            },
            Self::UnsupportedOrderType(order_type) => ApiReply {
                status: axum::http::StatusCode::BAD_REQUEST,
                reply: error(
                    "UnsupportedOrderType",
                    format!("{order_type} not supported"),
                ),
            },
            Self::NoLiquidity | Self::RateLimited | Self::EstimatorInternal(_) => ApiReply {
                status: axum::http::StatusCode::NOT_FOUND,
                reply: error("NoLiquidity", "no route found"),
            },
            Self::ProtocolInternal(err) => {
                tracing::error!(?err, "PriceEstimationError::Other");
                internal_error_reply()
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use {super::*, serde::ser, serde_json::json};

//     #[test]
//     fn rich_errors_skip_unset_data_field() {
//         assert_eq!(
//             serde_json::to_value(&Error {
//                 error_type: "foo",
//                 description: "bar",
//                 data: None,
//             })
//             .unwrap(),
//             json!({
//                 "errorType": "foo",
//                 "description": "bar",
//             }),
//         );
//         assert_eq!(
//             serde_json::to_value(Error {
//                 error_type: "foo",
//                 description: "bar",
//                 data: Some(json!(42)),
//             })
//             .unwrap(),
//             json!({
//                 "errorType": "foo",
//                 "description": "bar",
//                 "data": 42,
//             }),
//         );
//     }

//     #[tokio::test]
//     async fn rich_errors_handle_serialization_errors() {
//         struct AlwaysErrors;
//         impl Serialize for AlwaysErrors {
//             fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
//             where
//                 S: serde::Serializer,
//             {
//                 Err(ser::Error::custom("error"))
//             }
//         }

//         let body = warp::hyper::body::to_bytes(
//             rich_error("foo", "bar", AlwaysErrors)
//                 .into_response()
//                 .into_body(),
//         )
//         .await
//         .unwrap();

//         assert_eq!(
//             serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
//             json!({
//                 "errorType": "foo",
//                 "description": "bar",
//             })
//         );
//     }
// }
