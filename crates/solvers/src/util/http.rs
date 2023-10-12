//! Module containing utilities for round-tripping HTTP requests.
//!
//! Note that this helper is implemented as a macro. This is needed in order to
//! ensure that we preserve the module path in the log event from the original
//! callsite instead of all HTTP requests appearing as they are made from this
//! module.

use {
    crate::util,
    reqwest::{Method, RequestBuilder, StatusCode, Url},
    serde::de::DeserializeOwned,
    std::str,
};

macro_rules! roundtrip {
    (<$t:ty, $e:ty>; $request:expr) => {
        $crate::util::http::roundtrip_internal::<$t, $e>(
            $request,
            |method, url, body, message| {
                if let Some(body) = body {
                    tracing::trace!(%method, %url, %body, "{message}");
                } else {
                    tracing::trace!(%method, %url, "{message}");
                }
            },
            |status, body, message| {
                tracing::trace!(%status, %body, "{message}");
            },
        )
    };
    ($request:expr) => {
        $crate::util::http::roundtrip!(<_, _>; $request)
    };
}
pub(crate) use roundtrip;

#[doc(hidden)]
pub async fn roundtrip_internal<T, E>(
    request: RequestBuilder,
    log_request: impl FnOnce(&Method, &Url, Option<&str>, &str),
    log_response: impl FnOnce(StatusCode, &str, &str),
) -> Result<T, RoundtripError<E>>
where
    T: DeserializeOwned,
    E: DeserializeOwned,
{
    let (client, request) = request.build_split();
    let request = request.map_err(Error::from)?;

    let body = request
        .body()
        .and_then(|body| str::from_utf8(body.as_bytes()?).ok());

    log_request(
        request.method(),
        request.url(),
        body,
        "sending HTTP request",
    );
    let response = client.execute(request).await.map_err(Error::from)?;

    let status = response.status();
    let body = response.text().await.map_err(Error::from)?;
    log_response(status, &body, "received HTTP response");

    match serde_json::from_str::<T>(&body) {
        Ok(data) => Ok(data),
        // We failed to parse the body into the expected data, try to get an
        // as accurate error as possible:
        // 1. If the API returned a well-formed error that we can parse, then use that
        // 2. Otherwise, if it returned a 2xx status code, then this means that we are unable to
        //    parse a successful response, so return a JSON error
        // 3. Otherwise, return an HTTP status error with the raw body string.
        Err(err) => Err(serde_json::from_str(&body)
            .map(RoundtripError::Api)
            .unwrap_or_else(|_| {
                RoundtripError::Http(if status.is_success() {
                    Error::Json(err)
                } else {
                    Error::Status(status, body)
                })
            })),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("HTTP {0}: {1}")]
    Status(StatusCode, String),
}

impl From<RoundtripError<util::serialize::Never>> for Error {
    fn from(value: RoundtripError<util::serialize::Never>) -> Self {
        let RoundtripError::Http(err) = value else {
            unreachable!();
        };
        err
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RoundtripError<E> {
    #[error(transparent)]
    Http(#[from] Error),
    #[error("API error")]
    Api(E),
}
