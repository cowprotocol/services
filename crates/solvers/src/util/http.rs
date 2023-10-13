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

/// Roundtrip an HTTP request. This will `TRACE` log the request and responses.
///
/// This is a thin macro wrapper around [`roundtrip_internal`] that ensures that
/// logs are attributed from the callsite and not from this module. This allows
/// log filtering to be done in a more fine-grained manner and based on where
/// the HTTP roundtripping is happening.
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
    /// An error occurred when parsing the JSON body from the HTTP response
    /// into the expected result type.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A general HTTP error when sending a request or receiving a response.
    ///
    /// This is a protocol-level error, and can indicate a networking issue or
    /// a misbehaving HTTP server.
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    /// An error indicating that the HTTP response contained a non-200 status
    /// code indicating an application-level error.
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
