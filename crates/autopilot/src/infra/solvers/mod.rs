use {
    self::dto::{reveal, settle, settle_fast_path, solve},
    crate::util,
    alloy::signers::{Signer, aws::AwsSigner},
    anyhow::{Context, Result, anyhow},
    configs::autopilot::solver::Account,
    eth_domain_types as eth,
    observe::tracing::{distributed::headers::tracing_headers, lazy::Lazy},
    reqwest::{Client, RequestBuilder, StatusCode},
    std::{borrow::Cow, time::Duration},
    thiserror::Error,
    tracing::instrument,
    url::Url,
};

pub mod dto;

const RESPONSE_SIZE_LIMIT: usize = 10_000_000;
const RESPONSE_TIME_LIMIT: Duration = Duration::from_secs(60);

pub struct Driver {
    pub name: String,
    pub url: Url,
    pub submission_address: eth::Address,
    pub capabilities: Capabilities,
    client: Client,
}

/// Optional `/solve` request features a driver opted into.
#[derive(Clone, Copy, Debug, Default)]
pub struct Capabilities {
    /// Between checkpoints, receive only the difference to the previously
    /// sent auction instead of the full auction.
    pub auction_deltas: bool,
    /// Receive brotli-compressed request bodies.
    pub brotli: bool,
}

impl Capabilities {
    /// The capabilities a configured driver opted into. Brotli additionally
    /// requires `compress_solve_request` to be enabled.
    pub fn new(driver: &configs::autopilot::solver::Solver, compress_solve_request: bool) -> Self {
        Self {
            auction_deltas: driver.supports_auction_deltas,
            brotli: compress_solve_request && driver.supports_brotli,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to load KMS account")]
    UnableToLoadKmsAccount,
    #[error("failed to build client")]
    FailedToBuildClient(#[source] reqwest::Error),
}

impl Driver {
    #[instrument(skip_all)]
    pub async fn try_new(
        url: Url,
        name: String,
        submission_account: Account,
        capabilities: Capabilities,
    ) -> Result<Self, Error> {
        let submission_address = match submission_account {
            Account::Kms(key_id) => {
                let config = alloy::signers::aws::aws_config::load_from_env().await;
                let client = alloy::signers::aws::aws_sdk_kms::Client::new(&config);
                let account = AwsSigner::new(client, key_id.0.clone(), None)
                    .await
                    .map_err(|_| {
                        tracing::error!(?name, ?key_id, "Unable to load KMS account");
                        Error::UnableToLoadKmsAccount
                    })?;
                account.address()
            }
            Account::Address(address) => address,
        };
        tracing::info!(
            ?name,
            ?url,
            ?submission_address,
            ?capabilities,
            "creating solver"
        );

        Ok(Self {
            name,
            url,
            client: Client::builder()
                .timeout(RESPONSE_TIME_LIMIT)
                .tcp_keepalive(Duration::from_secs(60))
                .build()
                .map_err(Error::FailedToBuildClient)?,
            submission_address,
            capabilities,
        })
    }

    pub async fn solve(&self, request: solve::Request) -> Result<solve::Response> {
        self.request_response("solve", request.encoded(self.capabilities.brotli))
            .await
    }

    pub async fn reveal(&self, request: reveal::Request) -> Result<reveal::Response> {
        self.request_response("reveal", request).await
    }

    pub async fn settle(
        &self,
        request: &settle::Request,
        timeout: std::time::Duration,
    ) -> Result<()> {
        self.post_settle("settle", request, request.auction_id, timeout)
            .await
    }

    pub async fn settle_fast_path(
        &self,
        request: &settle_fast_path::Request,
        timeout: std::time::Duration,
    ) -> Result<()> {
        self.post_settle("settle_fast_path", request, request.auction_id, timeout)
            .await
    }

    /// Posts a settle request and waits for the driver to acknowledge it. The
    /// endpoints return no body, so only the status is inspected.
    async fn post_settle<Request: serde::Serialize>(
        &self,
        path: &str,
        request: &Request,
        auction_id: i64,
        timeout: std::time::Duration,
    ) -> Result<()> {
        let url = util::join(&self.url, path);
        tracing::trace!(
            path=&url.path(),
            body=%serde_json::to_string_pretty(request).unwrap(),
            "solver request",
        );

        let response = self
            .client
            .post(url)
            .json(request)
            .timeout(timeout)
            .header("X-REQUEST-ID", auction_id.to_string())
            .headers(tracing_headers())
            .send()
            .await
            .context("send")?;
        let status = response.status();

        tracing::trace!(%status, "solver response");

        if status != StatusCode::OK {
            let text = response.text().await.context("read error response body")?;
            return Err(anyhow!("bad status {status}: {text}"));
        }
        Ok(())
    }

    async fn request_response<Response, Request>(
        &self,
        path: &str,
        payload: Request,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
        Request: InjectIntoHttpRequest,
    {
        let url = util::join(&self.url, path);

        tracing::trace!(
            path = &url.path(),
            body = %Lazy(|| payload.body_to_string()),
            "solver request",
        );

        let request = self.client.post(url.clone()).headers(tracing_headers());
        let mut request = payload.inject(request);

        if let Some(request_id) = observe::tracing::distributed::request_id::from_current_span() {
            request = request.header("X-REQUEST-ID", request_id);
        }

        let mut response = request.send().await.context("send")?;
        let status = response.status().as_u16();
        let body = response_body_with_size_limit(&mut response, RESPONSE_SIZE_LIMIT)
            .await
            .context("body")?;
        let text = String::from_utf8_lossy(&body);
        tracing::trace!(%status, body=%text, "solver response");
        let context = || format!("url {url}, body {text:?}");
        if status != StatusCode::OK.as_u16() {
            return Err(anyhow!("bad status {status}, {}", context()));
        }
        serde_json::from_slice(&body).with_context(|| format!("bad json {}", context()))
    }
}

/// Extracts the bytes of the response up to some size limit.
///
/// Returns an error if the byte limit was exceeded.
pub async fn response_body_with_size_limit(
    response: &mut reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let slice: &[u8] = &chunk;
        if bytes.len() + slice.len() > limit {
            return Err(anyhow!("size limit exceeded"));
        }
        bytes.extend_from_slice(slice);
    }
    Ok(bytes)
}

trait InjectIntoHttpRequest {
    fn inject(&self, request: RequestBuilder) -> RequestBuilder;
    fn body_to_string(&self) -> Cow<'_, str>;
}

impl<T> InjectIntoHttpRequest for T
where
    T: serde::ser::Serialize + Sized,
{
    fn inject(&self, request: RequestBuilder) -> RequestBuilder {
        request.json(&self)
    }

    fn body_to_string(&self) -> Cow<'_, str> {
        let serialized = serde_json::to_string(&self).expect("type should be JSON serializable");
        Cow::Owned(serialized)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain,
        alloy::primitives::Address,
        axum::{body::Bytes, http::HeaderMap},
        configs::autopilot::solver::Solver,
        std::{
            collections::HashSet,
            sync::{Arc, Mutex},
        },
    };

    #[test]
    fn brotli_needs_global_switch_and_driver_opt_in() {
        let driver = |supports_brotli| Solver {
            supports_brotli,
            ..Solver::test("solver", Address::ZERO)
        };

        for (compress_solve_request, supports_brotli, expected) in [
            (false, false, false),
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            let capabilities = Capabilities::new(&driver(supports_brotli), compress_solve_request);
            assert_eq!(
                capabilities.brotli, expected,
                "compress_solve_request={compress_solve_request} supports_brotli={supports_brotli}"
            );
        }
    }

    #[test]
    fn auction_deltas_are_taken_from_the_driver_config() {
        let driver = Solver {
            supports_auction_deltas: true,
            ..Solver::test("solver", Address::ZERO)
        };
        assert!(Capabilities::new(&driver, false).auction_deltas);
    }

    /// The `content-encoding` header and raw body of a `/solve` request.
    type Received = Arc<Mutex<Option<(Option<String>, Bytes)>>>;

    /// Serves `/solve` on a local port, recording the request it receives.
    async fn mock_driver() -> (Url, Received) {
        let received = Received::default();
        let app = axum::Router::new().route(
            "/solve",
            axum::routing::post({
                let received = Arc::clone(&received);
                move |headers: HeaderMap, body: Bytes| async move {
                    let encoding = headers
                        .get(reqwest::header::CONTENT_ENCODING)
                        .map(|value| value.to_str().unwrap().to_owned());
                    *received.lock().unwrap() = Some((encoding, body));
                    r#"{"solutions": []}"#
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, received)
    }

    async fn solve_request(with_brotli: bool) -> solve::Request {
        let auction = domain::Auction {
            id: 1,
            block: 1,
            orders: vec![],
            prices: Default::default(),
            surplus_capturing_jit_order_owners: vec![],
        };
        solve::Request::new(&auction, &HashSet::new(), chrono::Utc::now(), with_brotli).await
    }

    /// Sends a `/solve` request built with a brotli copy to a driver with
    /// the given capability and returns what the driver received.
    async fn send(brotli: bool) -> (Option<String>, Bytes) {
        let (url, received) = mock_driver().await;
        let driver = Driver::try_new(
            url,
            "solver".to_owned(),
            Account::Address(Address::ZERO),
            Capabilities {
                auction_deltas: false,
                brotli,
            },
        )
        .await
        .unwrap();

        driver.solve(solve_request(true).await).await.unwrap();
        received.lock().unwrap().take().unwrap()
    }

    #[tokio::test]
    async fn opted_in_driver_receives_brotli_body() {
        let (encoding, body) = send(true).await;

        assert_eq!(encoding.as_deref(), Some("br"));
        let mut decompressed = Vec::new();
        brotli::BrotliDecompress(&mut body.as_ref(), &mut decompressed).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&decompressed).unwrap();
        assert_eq!(json["kind"], "full");
    }

    #[tokio::test]
    async fn other_drivers_receive_plain_json() {
        let (encoding, body) = send(false).await;

        assert_eq!(encoding, None);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["kind"], "full");
    }
}
